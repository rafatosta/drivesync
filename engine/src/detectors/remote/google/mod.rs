//! Google evidence collection, initial traversal and crash-safe page replay through Provider.
use super::*;
mod api;
pub(super) mod metadata;
use crate::{
    detectors::record::{ObservationRecord, RawRemoteObservation},
    models::*,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub(super) enum Request {
    Start,
    Get(String),
    List {
        parent: String,
        token: Option<String>,
    },
    Changes(String),
}
pub(super) enum Response {
    Start(String),
    File(Box<Value>),
    List(OriginalFileList),
    Changes(OriginalChangeList),
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OriginalFileList {
    #[serde(default)]
    files: Vec<Value>,
    next_page_token: Option<String>,
    incomplete_search: Option<bool>,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OriginalChangeList {
    #[serde(default)]
    changes: Vec<Value>,
    next_page_token: Option<String>,
    new_start_page_token: Option<String>,
}
#[cfg(test)]
impl Response {
    pub(super) fn file(file: impl Serialize) -> Self {
        Self::File(Box::new(serde_json::to_value(file).unwrap()))
    }
    pub(super) fn list(page: impl Serialize) -> Self {
        Self::List(serde_json::from_value(serde_json::to_value(page).unwrap()).unwrap())
    }
    pub(super) fn changes(page: impl Serialize) -> Self {
        Self::Changes(serde_json::from_value(serde_json::to_value(page).unwrap()).unwrap())
    }
}
/// Provider seam: typed requests only; no HTTP endpoints are built by the detector.
pub(super) trait Api: Send + Sync {
    fn call(&self, request: Request, cancellation: &Cancellation) -> RemoteResult<Response>;
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    files: Vec<ObservationRecord>,
    cursor: Option<String>,
    traversal: RemoteScanUpdate,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    version: u32,
    epoch: String,
    sequence: u64,
    cursor: Option<String>,
    pending: Option<Pending>,
    warning: Option<RemoteError>,
}
impl State {
    fn fresh() -> RemoteResult<Self> {
        use std::io::Read;
        let mut seed = [0u8; 24];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut seed))
            .map_err(|_| RemoteFailure::new(RemoteError::Internal))?;
        Ok(Self {
            version: 4,
            epoch: format!("{:x}", Sha256::digest(seed)),
            sequence: 0,
            cursor: None,
            pending: None,
            warning: None,
        })
    }
}
pub(super) struct GoogleRemoteDetector {
    pub(super) scope: RemoteDetectorScope,
    pub(super) api: Arc<dyn Api>,
    pub(super) backend: Arc<dyn RemoteBackend>,
}
fn incomplete() -> RemoteFailure {
    RemoteFailure::new(RemoteError::PageIncomplete)
}
fn valid(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control)
}
fn fingerprint(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))[..12].to_owned()
}
impl GoogleRemoteDetector {
    fn save(
        &self,
        old: &mut Option<Vec<u8>>,
        state: &State,
        traversal: &RemoteScanUpdate,
    ) -> RemoteResult<()> {
        let value = serde_json::to_vec(state).map_err(|_| incomplete())?;
        self.backend.save(old.as_deref(), &value, traversal)?;
        *old = Some(value);
        Ok(())
    }
    fn flush(
        &self,
        old: &mut Option<Vec<u8>>,
        state: &mut State,
        cancel: &Cancellation,
    ) -> RemoteResult<u64> {
        let Some(pending) = &state.pending else {
            return Ok(0);
        };
        cancel.check()?;
        let mut count = 0;
        for file in &pending.files {
            cancel.check()?;
            file.validate().map_err(|_| incomplete())?;
            if file.sync_id() != &self.scope.sync_id {
                return Err(incomplete());
            }
            let inserted = match file {
                ObservationRecord::Common(file) => self.backend.emit(file)?,
                ObservationRecord::Remote(raw) => self.backend.emit_original(raw)?,
            };
            if inserted {
                count += 1;
            }
        }
        cancel.check()?;
        let pending = state.pending.take().unwrap();
        state.cursor = pending.cursor;
        self.save(old, state, &pending.traversal)?;
        Ok(count)
    }
    fn stage(
        &self,
        old: &mut Option<Vec<u8>>,
        state: &mut State,
        files: Vec<ObservationRecord>,
        cursor: Option<String>,
        traversal: RemoteScanUpdate,
        cancel: &Cancellation,
    ) -> RemoteResult<u64> {
        state.pending = Some(Pending {
            files,
            cursor,
            traversal,
        });
        self.save(old, state, &RemoteScanUpdate::default())?;
        self.flush(old, state, cancel)
    }
    /// Reports collection failures, independently of resource change interpretation.
    fn warning_event(
        &self,
        state: &State,
        kind: FileEventKind,
        reason: Option<ObservationUncertainty>,
    ) -> RemoteResult<File> {
        let event = File {
            id: crate::models::FileId(format!("remote:{}:{}:0", state.epoch, state.sequence)),
            sync_id: self.scope.sync_id.clone(),
            event: FileEvent {
                local: None,
                remote: None,
                kind,
                uncertainty: reason,
                previous: None,
                current: None,
            },
            observed_at: now(),
            coverage: if kind == FileEventKind::ScopeUnavailable {
                ObservationCoverage::Unavailable
            } else {
                ObservationCoverage::Partial
            },
            scope: FileScope::Remote(RemoteFileRef {
                account_id: self.scope.account_id.clone(),
                file_id: RemoteFileId(self.scope.remote_root_id.clone()),
            }),
            data: None,
        };
        event.validate().map_err(|_| incomplete())?;
        Ok(event)
    }
    /// Preserves an original provider item; normalization belongs to delivery extraction.
    fn observation(
        &self,
        state: &State,
        index: usize,
        id: &str,
        original: Value,
        source: RemoteObservationSource,
    ) -> RemoteResult<ObservationRecord> {
        let raw = RawRemoteObservation {
            id: crate::models::FileId(format!("remote:{}:{}:{index}", state.epoch, state.sequence)),
            sync_id: self.scope.sync_id.clone(),
            observed_at: now(),
            account_id: self.scope.account_id.clone(),
            provider: ProviderId("google".into()),
            resource_id: RemoteFileId(id.to_owned()),
            source,
            original,
        };
        raw.validate().map_err(|_| incomplete())?;
        Ok(ObservationRecord::Remote(Box::new(raw)))
    }
    fn validate_checkpoint(&self) -> RemoteResult<()> {
        if let Some(bytes) = self.backend.load()? {
            let state: State = serde_json::from_slice(&bytes)
                .map_err(|_| RemoteFailure::new(RemoteError::CheckpointUnavailable))?;
            if state.version != 4
                || state.epoch.len() != 64
                || !state.epoch.bytes().all(|byte| byte.is_ascii_hexdigit())
                || state.cursor.as_deref().is_some_and(|token| !valid(token))
            {
                return Err(RemoteFailure::new(RemoteError::CheckpointUnavailable));
            }
            if let Some(pending) = state.pending {
                if pending.files.len() > 200
                    || pending.cursor.as_deref().is_some_and(|token| !valid(token))
                    || pending.traversal.validate().is_err()
                {
                    return Err(RemoteFailure::new(RemoteError::CheckpointUnavailable));
                }
                for file in pending.files {
                    file.validate()
                        .map_err(|_| RemoteFailure::new(RemoteError::CheckpointUnavailable))?;
                    if file.sync_id() != &self.scope.sync_id {
                        return Err(RemoteFailure::new(RemoteError::CheckpointUnavailable));
                    }
                }
            }
        }
        Ok(())
    }
    #[cfg(test)]
    pub fn poll(
        &self,
        mode: RemotePollMode,
        budget: u32,
        cancel: &Cancellation,
    ) -> RemoteResult<RemotePollResult> {
        self.observe_round(mode, budget, cancel, &|_, _| {})
    }
    fn observe_round(
        &self,
        mode: RemotePollMode,
        budget: u32,
        cancel: &Cancellation,
        progress: &dyn Fn(u64, u64),
    ) -> RemoteResult<RemotePollResult> {
        let counters = std::cell::Cell::new((0, 0));
        let report = |observed, emitted| {
            counters.set((observed, emitted));
            progress(observed, emitted);
        };
        let result = self.poll_round(mode, budget, cancel, &report);
        if let Err(error) = &result {
            if matches!(
                error.kind,
                RemoteError::PermissionDenied
                    | RemoteError::AuthorizationRequired
                    | RemoteError::CursorExpired
                    | RemoteError::PageIncomplete
            ) {
                let warnings = self.record_failure(error.kind, cancel)?;
                let (observed, emitted) = counters.get();
                progress(observed, emitted + warnings);
            }
        }
        result
    }
    fn record_failure(&self, error: RemoteError, cancel: &Cancellation) -> RemoteResult<u64> {
        cancel.check()?;
        let mut old = self.backend.load()?;
        let mut state = match &old {
            Some(bytes) => serde_json::from_slice::<State>(bytes)
                .map_err(|_| RemoteFailure::new(RemoteError::CheckpointUnavailable))?,
            None => State::fresh()?,
        };
        if state.pending.is_some() || state.warning == Some(error) {
            return Ok(0);
        }
        state.sequence = state
            .sequence
            .checked_add(1)
            .ok_or_else(|| RemoteFailure::new(RemoteError::CheckpointUnavailable))?;
        state.warning = Some(error);
        let inaccessible = matches!(
            error,
            RemoteError::PermissionDenied | RemoteError::AuthorizationRequired
        );
        let event = self.warning_event(
            &state,
            if inaccessible {
                FileEventKind::ScopeUnavailable
            } else {
                FileEventKind::RecheckRequired
            },
            if inaccessible {
                None
            } else {
                Some(ObservationUncertainty::CoverageIncomplete)
            },
        )?;
        let cursor = state.cursor.clone();
        self.stage(
            &mut old,
            &mut state,
            vec![ObservationRecord::Common(Box::new(event))],
            cursor,
            RemoteScanUpdate::default(),
            cancel,
        )
    }
    /// Page preparation is durable before first emission; replay cannot regenerate IDs/times.
    fn poll_round(
        &self,
        mode: RemotePollMode,
        budget: u32,
        cancel: &Cancellation,
        progress: &dyn Fn(u64, u64),
    ) -> RemoteResult<RemotePollResult> {
        cancel.check()?;
        let mut old = self.backend.load()?;
        let mut state = match &old {
            Some(bytes) => serde_json::from_slice::<State>(bytes)
                .map_err(|_| RemoteFailure::new(RemoteError::CheckpointUnavailable))?,
            None => State::fresh()?,
        };
        if state.version != 4 {
            return Err(RemoteFailure::new(RemoteError::CheckpointUnavailable));
        }
        let mut emitted = self.flush(&mut old, &mut state, cancel)?;
        let mut observed = 0;
        progress(observed, emitted);
        if mode == RemotePollMode::Recovery && state.cursor.is_some() {
            state.sequence = state
                .sequence
                .checked_add(1)
                .ok_or_else(|| RemoteFailure::new(RemoteError::CheckpointUnavailable))?;
            let event = self.warning_event(
                &state,
                FileEventKind::RecheckRequired,
                Some(ObservationUncertainty::CoverageIncomplete),
            )?;
            // Recovery never assumes that unseen entries were deleted.
            emitted += self.stage(
                &mut old,
                &mut state,
                vec![ObservationRecord::Common(Box::new(event))],
                None,
                RemoteScanUpdate {
                    reset: true,
                    ..Default::default()
                },
                cancel,
            )?;
        }
        let mut pages = 0;
        let mut completed = false;
        while pages < budget {
            cancel.check()?;
            let mut cursor = state.cursor.clone();
            let mut traversal = RemoteScanUpdate::default();
            let mut files = Vec::new();
            state.sequence = state
                .sequence
                .checked_add(1)
                .ok_or_else(|| RemoteFailure::new(RemoteError::CheckpointUnavailable))?;
            if cursor.is_none() {
                let Response::Start(start_cursor) = self.api.call(Request::Start, cancel)? else {
                    return Err(incomplete());
                };
                if !valid(&start_cursor) {
                    return Err(incomplete());
                }
                let Response::File(root) = self
                    .api
                    .call(Request::Get(self.scope.remote_root_id.clone()), cancel)?
                else {
                    return Err(incomplete());
                };
                if root.get("id").and_then(Value::as_str) != Some(&self.scope.remote_root_id)
                    || root.get("mimeType").and_then(Value::as_str)
                        != Some("application/vnd.google-apps.folder")
                    || root.get("trashed").and_then(Value::as_bool) != Some(false)
                {
                    return Err(RemoteFailure::new(RemoteError::PermissionDenied));
                }
                files.push(self.observation(
                    &state,
                    0,
                    &self.scope.remote_root_id,
                    *root,
                    RemoteObservationSource::Capture,
                )?);
                // Initial capture starts an empty traversal generation.
                traversal.reset = true;
                traversal.discovered.push(self.scope.remote_root_id.clone());
                cursor = Some(start_cursor);
                observed += 1;
                progress(observed, emitted);
            } else if let Some(scan) = self.backend.next_scan()? {
                let Response::List(page) = self.api.call(
                    Request::List {
                        parent: scan.folder.clone(),
                        token: scan.token.clone(),
                    },
                    cancel,
                )?
                else {
                    return Err(incomplete());
                };
                if page.incomplete_search == Some(true)
                    || page.files.len() > 200
                    || page
                        .next_page_token
                        .as_ref()
                        .is_some_and(|token| !valid(token) || Some(token) == scan.token.as_ref())
                {
                    return Err(incomplete());
                }
                traversal.finished = Some(scan);
                traversal.continuation = page.next_page_token;
                for raw in page.files {
                    observed += 1;
                    progress(observed, emitted);
                    let id = raw
                        .get("id")
                        .and_then(Value::as_str)
                        .filter(|id| valid(id))
                        .map(str::to_owned)
                        .ok_or_else(incomplete)?;
                    let directory = raw.get("mimeType").and_then(Value::as_str)
                        == Some("application/vnd.google-apps.folder");
                    files.push(self.observation(
                        &state,
                        files.len(),
                        &id,
                        raw,
                        RemoteObservationSource::Capture,
                    )?);
                    if directory {
                        traversal.discovered.push(id);
                    }
                }
            } else {
                let current_cursor = cursor.as_ref().unwrap();
                let Response::Changes(page) = self
                    .api
                    .call(Request::Changes(current_cursor.clone()), cancel)?
                else {
                    return Err(incomplete());
                };
                if page.changes.len() > 200
                    || page.next_page_token.is_some() == page.new_start_page_token.is_some()
                {
                    return Err(incomplete());
                }
                completed = page.next_page_token.is_none();
                let token = page
                    .next_page_token
                    .or(page.new_start_page_token)
                    .ok_or_else(incomplete)?;
                if !valid(&token) || (!completed && token == *current_cursor) {
                    return Err(incomplete());
                }
                for change in page.changes {
                    cancel.check()?;
                    observed += 1;
                    progress(observed, emitted);
                    let id = change
                        .get("fileId")
                        .and_then(Value::as_str)
                        .filter(|id| valid(id))
                        .map(str::to_owned)
                        .ok_or_else(incomplete)?;
                    files.push(self.observation(
                        &state,
                        files.len(),
                        &id,
                        change,
                        RemoteObservationSource::Changes,
                    )?);
                }
                cursor = Some(token);
            }
            state.warning = None;
            emitted += self.stage(&mut old, &mut state, files, cursor, traversal, cancel)?;
            progress(observed, emitted);
            pages += 1;
            if completed {
                break;
            }
        }
        let has_more = !completed || self.backend.next_scan()?.is_some();
        Ok(RemotePollResult {
            coverage: if has_more {
                RemoteCoverage::Partial
            } else {
                RemoteCoverage::Complete
            },
            observed,
            emitted,
            has_more,
            checkpoint_confirmed: state.cursor.as_deref().map(fingerprint),
        })
    }
}
#[cfg(test)]
mod tests;

/// Creates Google's implementation after the common facade has selected this provider.
pub(super) fn create(
    connector: crate::Engine,
    session: RemoteSessionContext,
    scope: RemoteDetectorScope,
    backend: Arc<dyn RemoteBackend>,
) -> RemoteResult<Box<dyn RemoteObserver>> {
    if session.provider.0 != "google" || session.account_id != scope.account_id {
        return Err(RemoteFailure::new(RemoteError::InvalidScope));
    }
    Ok(Box::new(GoogleRemoteDetector {
        scope,
        api: Arc::new(api::Access::new(connector, session)),
        backend,
    }))
}
impl RemoteObserver for GoogleRemoteDetector {
    fn validate_progress(&self) -> RemoteResult<()> {
        self.validate_checkpoint()
    }
    fn poll_once(
        &self,
        request: RemotePollRequest,
        cancel: &Cancellation,
        progress: &dyn Fn(u64, u64),
    ) -> RemoteResult<RemotePollResult> {
        let budget = request
            .page_budget
            .ok_or_else(|| RemoteFailure::new(RemoteError::InvalidScope))?;
        self.observe_round(request.mode, budget, cancel, progress)
    }
}
