//! Internal observation and context services for synchronization.

pub(crate) mod context;
pub(crate) mod observation;

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
    ) -> Result<Vec<File>, ContextError> {
        tokio::task::spawn_blocking(move || self.context.consolidate(observation))
            .await
            .map_err(|_| ContextError::InventoryUnavailable)?
    }
    /// Persisted remote IDs are probe candidates, not evidence of current presence.
    pub(crate) fn known_remote_ids(&self) -> Result<Vec<crate::models::RemoteId>, ContextError> {
        Ok(self
            .context
            .snapshots()?
            .into_iter()
            .filter_map(|s| s.identity.remote_id)
            .collect())
    }
}

pub(crate) mod sources;
