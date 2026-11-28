//! Operations owns task-scoped reporting and a bounded read side for the Orchestrator.

mod models;
#[cfg(test)]
mod tests;

use crate::{
    activity::{
        ActivityContext, ActivityModule, ActivityReference, ActivityReporter, DiagnosticWriter,
        Recent, Sample, HASH_PROGRESS_MIN,
    },
    inventory::models::{InventoryWriteResult, WriteStatus},
    models::{Id, ItemId},
};
pub(in crate::orchestrator) use models::*;
use std::{
    future::Future,
    io::Write,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    task::Poll,
    time::{Duration, Instant},
};
use tokio::sync::Notify;

const ACTIVE_LIMIT: usize = 128;
const UPDATE_LIMIT: usize = 256;

struct Latest {
    closed: bool,
    next: u64,
    active: Vec<Activity>,
    updates: Recent<Activity>,
    overwritten: u64,
    omitted: u64,
}

struct Channel {
    scope: Arc<Scope>,
    label: u64,
    trace: bool,
    latest: Mutex<Latest>,
    changed: Notify,
}

impl Channel {
    fn update(&self, id: ActivityId, sample: Sample<Update>) {
        let mut latest = self.latest.lock().unwrap_or_else(|e| e.into_inner());
        let Some(index) = latest.index(id) else {
            return;
        };
        let record = &mut latest.active[index];
        if sample.sequence <= record.sequence {
            return;
        }
        record.trace = true;
        match sample.value {
            Update::Phase(phase, state) => {
                if record.phase != phase {
                    record.progress = None;
                    record.receipt = None;
                    record.last_wait = None;
                }
                record.phase = phase;
                record.state = state;
            }
            Update::Progress(progress) => record.progress = Some(progress),
            Update::Effect(effect, persisted) => {
                record.effect = effect;
                record.effect_persisted = persisted;
            }
            Update::Receipt(status) => record.receipt = Some(status),
            Update::Response(phase, state, elapsed) => {
                debug_assert_eq!(record.phase, phase);
                record.state = state;
                if state == State::Running {
                    record.last_wait = Some((Wait::Response, elapsed));
                }
                record.trace = elapsed >= Duration::from_millis(200);
            }
        }
        record.sequence = sample.sequence;
        record.updated = sample.at;
        let record = record.clone();
        latest.publish(record);
        drop(latest);
        self.changed.notify_one();
    }

    fn finish(&self, id: ActivityId, outcome: Outcome, progress: Option<Progress>) {
        let mut latest = self.latest.lock().unwrap_or_else(|e| e.into_inner());
        if latest.retire(id, outcome, progress) {
            drop(latest);
            self.changed.notify_one();
        }
    }
}

impl Latest {
    fn index(&self, id: ActivityId) -> Option<usize> {
        self.active
            .binary_search_by_key(&id, |record| record.id)
            .ok()
    }

    fn publish(&mut self, record: Activity) {
        self.overwritten += u64::from(self.updates.push(record));
    }

    fn retire(&mut self, id: ActivityId, outcome: Outcome, progress: Option<Progress>) -> bool {
        let Some(index) = self.index(id) else {
            return false;
        };
        let mut record = self.active.remove(index);
        while let Some(child) = self.active.iter().find(|record| record.parent == Some(id)) {
            self.retire(child.id, Outcome::Cancelled, None);
        }
        if progress.is_some() {
            record.progress = progress;
        }
        record.state = State::Finished(outcome);
        record.trace = true;
        record.updated = Instant::now();
        record.sequence += 1;
        self.publish(record);
        true
    }
}

/// Per-account/root lifecycle. Recording is independent of the functional queue and public watch.
#[derive(Clone)]
pub(in crate::orchestrator) struct ActivityInbox(Option<Arc<Channel>>);

impl ActivityInbox {
    pub(in crate::orchestrator) fn new(scope: Scope) -> Self {
        Self::with_trace(
            scope,
            std::env::var_os("DRIVESYNC_TRACE_OPERATIONS").is_some_and(|value| value == "1"),
        )
    }

    #[cfg(test)]
    pub(in crate::orchestrator) fn disabled() -> Self {
        Self(None)
    }

    fn with_trace(scope: Scope, trace: bool) -> Self {
        static NEXT_SCOPE: AtomicU64 = AtomicU64::new(1);
        Self(Some(Arc::new(Channel {
            scope: Arc::new(scope),
            label: NEXT_SCOPE.fetch_add(1, Ordering::Relaxed),
            trace,
            latest: Mutex::new(Latest {
                closed: false,
                next: 0,
                active: Vec::new(),
                updates: Recent::new(UPDATE_LIMIT),
                overwritten: 0,
                omitted: 0,
            }),
            changed: Notify::new(),
        })))
    }

    /// Install the Operations sink for one functional future without spawning a task.
    pub(in crate::orchestrator) async fn observe<F: Future>(&self, work: F) -> F::Output {
        let context = self.0.as_ref().map(|channel| Context {
            channel: channel.clone(),
            parent: None,
        });
        CONTEXT.scope(context, work).await
    }

    pub(in crate::orchestrator) async fn changed(&self) {
        match &self.0 {
            Some(channel) => channel.changed.notified().await,
            None => std::future::pending().await,
        }
    }

    pub(in crate::orchestrator) fn close(&self) {
        let Some(channel) = &self.0 else { return };
        let mut latest = channel.latest.lock().unwrap_or_else(|e| e.into_inner());
        latest.closed = true;
        while let Some(record) = latest.active.first() {
            let id = record.id;
            latest.retire(id, Outcome::Cancelled, None);
        }
        drop(latest);
        channel.changed.notify_one();
    }

    /// Read a consistent active view without draining ordered updates or consulting work status.
    pub(in crate::orchestrator) fn current(&self) -> Vec<Activity> {
        self.0.as_ref().map_or_else(Vec::new, |channel| {
            channel
                .latest
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .active
                .clone()
        })
    }

    #[cfg(test)]
    pub(in crate::orchestrator) fn take_updates(&self) -> Vec<Activity> {
        self.0.as_ref().map_or_else(Vec::new, |channel| {
            channel.latest.lock().unwrap().updates.take()
        })
    }

    #[cfg(test)]
    pub(super) fn publication_count(&self) -> u64 {
        self.0.as_ref().map_or(0, |channel| {
            let mut latest = channel.latest.lock().unwrap();
            latest.overwritten + latest.updates.take().len() as u64
        })
    }

    #[cfg(test)]
    pub(in crate::orchestrator) fn for_config(config: &super::models::OperatorConfig) -> Self {
        Self::with_trace(
            Scope {
                account: config.account_id.clone(),
                root: config.sync_root_id.clone(),
            },
            false,
        )
    }

    #[cfg(test)]
    pub(in crate::orchestrator) async fn test_execute<T, E>(
        &self,
        config: &super::models::OperatorConfig,
        request: &super::models::ExecuteOperationRequest,
        work: impl Future<Output = Result<T, E>>,
    ) -> Result<T, E> {
        self.observe(async {
            let mut span = Span::execution(
                config,
                Kind::Execute(request.operation.action),
                Some(&request.execution_id),
                Some(&request.operation.item_ref),
                Phase::Dispatch,
            );
            let result = span.within(work).await;
            span.result(&result);
            result
        })
        .await
    }

    /// Consume records in publication order. Formatting and stderr never run on a producer.
    pub(in crate::orchestrator) fn consume(&self) {
        let Some(channel) = &self.0 else { return };
        if !channel.trace {
            channel
                .latest
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .updates
                .clear();
            return;
        }
        channel.flush();
    }
}

impl Channel {
    fn flush(self: &Arc<Self>) {
        static WRITER: DiagnosticWriter = DiagnosticWriter::new();
        if !self.pending_trace() {
            return;
        }
        let channel = self.clone();
        let wake = self.clone();
        WRITER.spawn(
            move || {
                let _ = channel.write_pending(std::io::stderr().lock());
            },
            move || {
                let closed = wake.latest.lock().unwrap_or_else(|e| e.into_inner()).closed;
                if closed {
                    // The supervisor no longer receives notifications after closing the inbox.
                    wake.flush();
                } else {
                    wake.changed.notify_one();
                }
            },
        );
    }

    fn pending_trace(&self) -> bool {
        !self
            .latest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .updates
            .is_empty()
    }

    fn write_pending(&self, mut output: impl Write) -> std::io::Result<()> {
        self.write_to(&mut output)?;
        let closing_backlog = {
            let latest = self.latest.lock().unwrap_or_else(|e| e.into_inner());
            latest.closed && !latest.updates.is_empty()
        };
        if closing_backlog {
            self.write_to(&mut output)?;
        }
        Ok(())
    }

    fn write_to(&self, mut output: impl Write) -> std::io::Result<()> {
        let (records, overwritten, omitted) = {
            let mut latest = self.latest.lock().unwrap_or_else(|e| e.into_inner());
            (
                latest.updates.take(),
                std::mem::take(&mut latest.overwritten),
                std::mem::take(&mut latest.omitted),
            )
        };
        for record in records {
            let elapsed = record.updated.saturating_duration_since(record.started);
            if !record.trace
                || (record.phase == Phase::CheckpointTransfer
                    && record.state != State::Finished(Outcome::Failed)
                    && elapsed < Duration::from_millis(200))
            {
                continue;
            }
            debug_assert_eq!(record.execution.scope, self.scope);
            debug_assert_eq!(
                record.execution.item.is_some(),
                record.execution.display_item.is_some()
            );
            writeln!(
                output,
                "drivesync-operations scope={} activity={} parent={:?} kind={:?} direction={:?} label={:?} phase={:?} state={:?} sequence={} elapsed_ms={} progress={:?} bytes_per_sec={:?} effect={:?} persisted={} receipt={:?} has_execution={} last_wait={:?}",
                self.label, record.id.0, record.parent, record.execution.kind, record.execution.direction,
                record.execution.display_item, record.phase, record.state, record.sequence,
                elapsed.as_millis(), record.progress, record.bytes_per_second(), record.effect, record.effect_persisted, record.receipt,
                record.execution.id.is_some(), record.last_wait,
            )?;
        }
        if overwritten != 0 || omitted != 0 {
            writeln!(
                output,
                "drivesync-operations scope={} overwritten={overwritten} omitted={omitted}",
                self.label,
            )?;
        }
        Ok(())
    }
}

#[derive(Clone)]
struct Context {
    channel: Arc<Channel>,
    parent: Option<ActivityId>,
}

tokio::task_local! {
    static CONTEXT: Option<Context>;
}

enum Update {
    Phase(Phase, State),
    Progress(Progress),
    Effect(Effect, bool),
    Receipt(WriteStatus),
    Response(Phase, State, Duration),
}

struct Command<'a> {
    kind: Kind,
    execution: Option<&'a Id>,
    item: Option<&'a ItemId>,
}

/// Private reporting contract for RemoteExecutor, Filesystem, Journal and recovery. The active
/// parent travels with the polled future, including synchronous work performed inside that future.
#[derive(Default)]
pub(super) struct Span {
    context: Option<Context>,
    reporter: ActivityReporter,
}

impl Span {
    pub(super) fn activity_context(&self) -> Option<ActivityContext> {
        let context = self.context.as_ref()?;
        let id = context.parent?;
        Some(ActivityContext {
            parent: Some(ActivityReference {
                module: ActivityModule::Operations,
                producer: context.channel.label,
                session: context.channel.label,
                account: context.channel.scope.account.clone(),
                root: Some(context.channel.scope.root.clone()),
                id,
            }),
            correlation: None,
        })
    }

    pub(super) fn execution(
        scope: &super::models::OperatorConfig,
        kind: Kind,
        execution: Option<&Id>,
        item: Option<&ItemId>,
        phase: Phase,
    ) -> Self {
        CONTEXT
            .try_with(|context| {
                let Some(context) = context else {
                    return Self::default();
                };
                if context.channel.scope.account != scope.account_id
                    || context.channel.scope.root != scope.sync_root_id
                {
                    return Self::default();
                }
                Self::begin(
                    context,
                    phase,
                    State::Running,
                    Some(Command {
                        kind,
                        execution,
                        item,
                    }),
                )
            })
            .unwrap_or_default()
    }

    pub(super) fn start(phase: Phase, state: State) -> Self {
        CONTEXT
            .try_with(|context| {
                context.as_ref().map_or_else(Self::default, |context| {
                    Self::begin(context, phase, state, None)
                })
            })
            .unwrap_or_default()
    }

    fn begin(context: &Context, phase: Phase, state: State, command: Option<Command<'_>>) -> Self {
        let channel = &context.channel;
        let mut latest = channel.latest.lock().unwrap_or_else(|e| e.into_inner());
        if latest.closed {
            return Self::default();
        }
        let parent = match context.parent {
            Some(parent) => match latest.index(parent) {
                Some(index) => Some(&latest.active[index]),
                None => return Self::default(),
            },
            None => None,
        };
        if latest.active.len() >= ACTIVE_LIMIT {
            latest.omitted += 1;
            return Self::default();
        }
        let id = ActivityId(latest.next + 1);
        let execution = match command {
            Some(command) => Arc::new(Execution {
                scope: channel.scope.clone(),
                id: command.execution.cloned(),
                item: command.item.cloned(),
                display_item: command.item.map(|_| id),
                kind: command.kind,
                direction: command.kind.direction(),
            }),
            None => match parent {
                Some(parent) => parent.execution.clone(),
                None => return Self::default(),
            },
        };
        let now = Instant::now();
        let record = Activity {
            id,
            parent: context.parent,
            execution,
            phase,
            state,
            progress: None,
            effect: Effect::NotApplied,
            effect_persisted: false,
            receipt: None,
            sequence: 0,
            started: now,
            updated: now,
            last_wait: None,
            trace: true,
        };
        latest.next = id.0;
        latest.active.push(record.clone());
        latest.publish(record);
        drop(latest);
        channel.changed.notify_one();
        Self {
            context: Some(Context {
                channel: channel.clone(),
                parent: Some(id),
            }),
            reporter: ActivityReporter::default(),
        }
    }

    pub(super) fn enabled(&self) -> bool {
        self.context.is_some()
    }

    pub(super) async fn within<F: Future>(&self, work: F) -> F::Output {
        CONTEXT.scope(self.context.clone(), work).await
    }

    pub(super) fn within_sync<T>(&self, work: impl FnOnce() -> T) -> T {
        CONTEXT.sync_scope(self.context.clone(), work)
    }

    /// Track actual body/acknowledgement waits only when the underlying future suspends.
    pub(super) async fn response<F: Future>(&self, phase: Phase, work: F) -> F::Output {
        let mut work = std::pin::pin!(work);
        let mut waiting_since: Option<Instant> = None;
        std::future::poll_fn(|cx| {
            let result = work.as_mut().poll(cx);
            let Some(context) = &self.context else {
                return result;
            };
            let update = match (&result, waiting_since) {
                (Poll::Pending, None) => {
                    waiting_since = Some(Instant::now());
                    Some(Update::Response(
                        phase,
                        State::Waiting(Wait::Response),
                        Duration::ZERO,
                    ))
                }
                (Poll::Ready(_), Some(since)) => {
                    Some(Update::Response(phase, State::Running, since.elapsed()))
                }
                _ => None,
            };
            if let Some(update) = update {
                context
                    .channel
                    .update(context.parent.unwrap(), self.reporter.transition(update));
            }
            result
        })
        .await
    }

    pub(super) fn phase(&self, phase: Phase, state: State) {
        self.update(true, Update::Phase(phase, state));
    }

    pub(super) fn progress(&self, progress: Progress, final_sample: bool) {
        self.update(final_sample, Update::Progress(progress));
    }

    pub(super) fn effect(&self, effect: Effect, persisted: bool) {
        self.update(true, Update::Effect(effect, persisted));
    }

    pub(super) fn receipt(&self, status: WriteStatus) {
        self.update(true, Update::Receipt(status));
    }

    fn update(&self, immediate: bool, update: Update) {
        let Some(context) = &self.context else { return };
        if let Some(sample) = self.reporter.sample(immediate, || update) {
            context.channel.update(context.parent.unwrap(), sample);
        }
    }

    pub(super) fn finish(&mut self, outcome: Outcome) {
        self.finish_with_progress(outcome, None);
    }

    fn finish_with_progress(&mut self, outcome: Outcome, progress: Option<Progress>) {
        if let Some(context) = self.context.take() {
            context
                .channel
                .finish(context.parent.unwrap(), outcome, progress);
        }
    }

    pub(super) fn result<T, E>(&mut self, result: &Result<T, E>) {
        self.finish(if result.is_ok() {
            Outcome::Complete
        } else {
            Outcome::Failed
        });
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        self.finish(if std::thread::panicking() {
            Outcome::Failed
        } else {
            Outcome::Cancelled
        });
    }
}

pub(super) async fn waiting<T, E>(
    phase: Phase,
    reason: Wait,
    work: impl Future<Output = Result<T, E>>,
) -> Result<T, E> {
    let mut span = Span::start(phase, State::Waiting(reason));
    let result = span.within(work).await;
    span.result(&result);
    result
}

pub(super) fn persist(
    phase: Phase,
    write: impl FnOnce(Option<&ActivityContext>) -> InventoryWriteResult,
) -> InventoryWriteResult {
    let mut span = Span::start(phase, State::Waiting(Wait::Inventory));
    let context = span.activity_context();
    let receipt = write(context.as_ref());
    span.receipt(receipt.status);
    span.finish(if receipt.status == WriteStatus::Written {
        Outcome::Complete
    } else {
        Outcome::Failed
    });
    receipt
}

pub(super) fn measured<T, E>(
    phase: Phase,
    state: State,
    work: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    let mut span = Span::start(phase, state);
    let result = span.within_sync(work);
    span.result(&result);
    result
}

pub(super) fn measured_context<T, E>(
    phase: Phase,
    state: State,
    work: impl FnOnce(Option<&ActivityContext>) -> Result<T, E>,
) -> Result<T, E> {
    let mut span = Span::start(phase, state);
    let context = span.activity_context();
    let result = span.within_sync(|| work(context.as_ref()));
    span.result(&result);
    result
}

pub(super) fn applied<T>(
    phase: Phase,
    work: impl FnOnce() -> Result<T, super::models::OperatorError>,
) -> Result<T, super::models::OperatorError> {
    let mut span = Span::start(phase, State::Waiting(Wait::LocalIo));
    let result = span.within_sync(work);
    let (effect, outcome) = match &result {
        Ok(_) => (Effect::Applied, Outcome::Complete),
        Err(error) if error.effect != crate::models::ExecutionEffect::NotApplied => {
            (Effect::Uncertain, Outcome::Unconfirmed)
        }
        Err(_) => (Effect::NotApplied, Outcome::Failed),
    };
    if effect != Effect::NotApplied {
        span.effect(effect, false);
    }
    span.finish(outcome);
    result
}

/// Reports the hashes already computed by a single content loop.
pub(super) struct HashProgress {
    span: Span,
    bytes: Bytes,
}

impl HashProgress {
    pub(super) fn new(phase: Phase, total: Option<u64>, algorithm: Algorithm) -> Self {
        Self {
            span: Span::start(phase, State::Running),
            bytes: Bytes {
                total,
                hashed: Some(0),
                algorithm: Some(algorithm),
                ..Bytes::default()
            },
        }
    }

    pub(super) fn processed(&mut self, bytes: usize) {
        if !self.span.enabled() {
            return;
        }
        self.bytes.hashed = Some(self.bytes.hashed.unwrap().saturating_add(bytes as u64));
        if self.bytes.total.or(self.bytes.hashed).unwrap_or_default() >= HASH_PROGRESS_MIN {
            self.span.progress(Progress::Bytes(self.bytes), false);
        }
    }

    pub(super) fn finish(&mut self, outcome: Outcome) {
        self.span
            .finish_with_progress(outcome, Some(Progress::Bytes(self.bytes)));
    }

    pub(super) fn result<T, E>(&mut self, result: &Result<T, E>) {
        self.finish(if result.is_ok() {
            Outcome::Complete
        } else {
            Outcome::Failed
        });
    }
}

pub(super) async fn request<T, E>(
    phase: Phase,
    work: impl Future<Output = Result<T, E>>,
) -> Result<T, E> {
    provider_request(phase, work, false).await
}

pub(super) async fn mutation<T, E>(
    phase: Phase,
    work: impl Future<Output = Result<T, E>>,
) -> Result<T, E> {
    provider_request(phase, work, true).await
}

async fn provider_request<T, E>(
    phase: Phase,
    work: impl Future<Output = Result<T, E>>,
    mutation: bool,
) -> Result<T, E> {
    let mut span = Span::start(phase, State::Waiting(Wait::Provider));
    let result = work.await;
    if mutation && result.is_ok() {
        span.effect(Effect::Applied, false);
    }
    span.finish_with_progress(
        if result.is_ok() {
            Outcome::Complete
        } else {
            Outcome::Failed
        },
        Some(Progress::Count {
            unit: Unit::Requests,
            completed: 1,
            total: Some(1),
        }),
    );
    result
}

pub(super) async fn page<T, E>(
    work: impl Future<Output = Result<T, E>>,
    items: impl FnOnce(&T) -> usize,
) -> Result<T, E> {
    let mut span = Span::start(Phase::CheckDestination, State::Waiting(Wait::Provider));
    let result = work.await;
    span.progress(
        Progress::Page {
            requests: 1,
            pages: u64::from(result.is_ok()),
            items: result.as_ref().ok().map_or(0, |page| items(page) as u64),
        },
        true,
    );
    span.result(&result);
    result
}

#[cfg(test)]
pub(super) async fn test_response(
    request: &super::ExecuteOperationRequest,
    work: impl Future<Output = ()>,
) {
    let mut root = CONTEXT
        .try_with(|context| {
            let Some(context) = context.as_ref().filter(|context| {
                context.channel.scope.account == request.identity.account_id
                    && context.channel.scope.root == request.identity.sync_root_id
            }) else {
                return Span::default();
            };
            Span::begin(
                context,
                Phase::Dispatch,
                State::Running,
                Some(Command {
                    kind: Kind::Execute(request.operation.action),
                    execution: Some(&request.execution_id),
                    item: Some(&request.operation.item_ref),
                }),
            )
        })
        .unwrap_or_default();
    root.within(async {
        let mut span = Span::start(Phase::Upload, State::Running);
        span.response(Phase::Upload, work).await;
        span.finish(Outcome::Complete);
    })
    .await;
    root.finish(Outcome::Complete);
}
