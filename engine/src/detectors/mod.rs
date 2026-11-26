//! Internal observation and context services for synchronization.

pub(crate) mod activity;
pub(crate) mod context;
pub(crate) mod observation;

use activity::{ActivityGuard, ActivityPublisher, Details, Origin, Phase, State, Wait};
use context::{Context, ContextConfig, ContextError, File};
use observation::Observation;
/// Consolidation facade consumed by the synchronization lifecycle, without analysis or execution.
pub(crate) struct Detectors {
    context: Context,
}
impl Detectors {
    pub(crate) fn open(config: ContextConfig) -> Result<Self, ContextError> {
        Ok(Self {
            context: Context::open(config)?,
        })
    }
    /// Context consumes the same Observation envelope for complete rounds and individual reads.
    pub(crate) async fn contextualize_batch(
        self: std::sync::Arc<Self>,
        observation: Observation,
        delivery: &ActivityGuard,
    ) -> Result<Vec<File>, ContextError> {
        let activity = delivery.child_from(
            Origin::Shared,
            Phase::Contextualize,
            State::Waiting(Wait::Worker),
            Details::default(),
        );
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
        publisher: &ActivityPublisher,
        epoch: u64,
        expected: &crate::models::LocalIdentity,
    ) -> Result<crate::synchronization::SynchronizationAudit, ContextError> {
        let activity = publisher.begin(
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
