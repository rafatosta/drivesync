//! Internal observation and context services for synchronization.

pub(crate) mod activity;
mod context;
mod observation;
mod progress;

use activity::{ActivityChannel, ActivityGuard, Details, Origin, Phase, Scope, State, Wait};
#[cfg(test)]
pub(crate) use context::{AssociationCandidate, Context as TestContext, Environment, Observations};
use context::{Context, ContextError};
pub(crate) use context::{ContextConfig, File, PathRoot};
#[cfg(test)]
pub(crate) use observation::{
    models::{CaptureEntries, ScopeCapture},
    remote::interfaces::CancellationToken as TestCancellation,
    scanner::Scanner as TestScanner,
    RemoteObservation,
};
pub(crate) use observation::{Observation, ObservationCoverage};
use std::sync::Arc;
/// Consolidation facade consumed by the synchronization lifecycle, without analysis or execution.
pub(crate) struct Detectors {
    context: Context,
    activities: Arc<ActivityChannel>,
}
impl Detectors {
    pub(crate) fn open(config: ContextConfig) -> Result<Self, ContextError> {
        let activities = ActivityChannel::new(Scope {
            account: config.account_id.clone(),
            root: config.sync_root_id.clone(),
        });
        Ok(Self {
            context: Context::open(config)?,
            activities,
        })
    }
    pub(crate) fn activity_inbox(&self) -> activity::ActivityInbox {
        self.activities.inbox()
    }

    /// Context consumes the same Observation envelope for complete rounds and individual reads.
    pub(crate) async fn contextualize_batch(
        self: std::sync::Arc<Self>,
        observation: Observation,
        delivery: Option<&ActivityGuard>,
    ) -> Result<Vec<File>, ContextError> {
        let state = State::Waiting(Wait::Worker);
        let activity = match delivery {
            Some(delivery) => delivery.child_from(
                Origin::Shared,
                Phase::Contextualize,
                state,
                Details::default(),
            ),
            None => self.activities.shared().begin(
                Origin::Shared,
                None,
                Phase::Contextualize,
                state,
                Details::default(),
            ),
        };
        tokio::task::spawn_blocking(move || {
            activity.set_state(State::Running);
            let result = self.context.consolidate_observed(observation, &activity);
            activity.finish(if result.is_ok() {
                State::Complete
            } else {
                State::Failed
            });
            result
        })
        .await
        .map_err(|_| ContextError::InventoryUnavailable)?
    }
    pub(crate) fn audit(
        &self,
        epoch: u64,
        expected: &crate::models::LocalIdentity,
    ) -> Result<crate::synchronization::SynchronizationAudit, ContextError> {
        let activity = self.activities.shared().begin(
            Origin::Shared,
            None,
            Phase::Audit,
            State::Running,
            Details::default(),
        );
        let result = self.context.audit_observed(epoch, expected, &activity);
        activity.finish(if result.is_ok() {
            State::Complete
        } else {
            State::Failed
        });
        result
    }
    /// Persisted remote IDs are probe candidates, not evidence of current presence.
    pub(crate) fn known_remote_ids(
        &self,
        activity: ActivityGuard,
    ) -> Result<Vec<crate::models::RemoteId>, ContextError> {
        activity.set_state(State::Running);
        let result = self.context.known_remote_ids(&activity);
        activity.finish(if result.is_ok() {
            State::Complete
        } else {
            State::Failed
        });
        result
    }
}

pub(crate) mod sources;

/// Policy for the caller's fresh read-only local verification.
pub(crate) enum LocalVerification {
    Once,
    ExpandLimit,
}

/// Returns a complete pinned-root read for an execution precondition.
/// This never submits observations or alters the source capture horizon.
pub(crate) fn verify_local_scope(
    root: crate::models::Path,
    scope: crate::models::SyncRootId,
    verification: LocalVerification,
) -> Result<std::collections::BTreeMap<crate::models::Path, observation::LocalObservation>, ()> {
    use observation::{
        remote::interfaces::CancellationToken,
        scanner::{ScanIssueKind, Scanner},
    };
    let mut scanner = Scanner::open(root, scope).map_err(|_| ())?;
    let cancellation = CancellationToken::new();
    loop {
        let scan = scanner.scan(&cancellation).map_err(|_| ())?;
        if scan.coverage == ObservationCoverage::Complete {
            return Ok(scan.entries);
        }
        if !matches!(verification, LocalVerification::ExpandLimit)
            || !scan
                .issues
                .iter()
                .any(|issue| issue.kind == ScanIssueKind::LimitReached)
            || !scanner.expand_limit()
        {
            return Err(());
        }
    }
}
