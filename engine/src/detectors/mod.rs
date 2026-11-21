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
    /// Runs consolidation off async executor threads while preserving the File-only facade.
    pub(crate) async fn contextualize_async(
        self: std::sync::Arc<Self>,
        observation: Observation,
    ) -> Result<File, ContextError> {
        tokio::task::spawn_blocking(move || self.contextualize(observation))
            .await
            .map_err(|_| ContextError::InventoryUnavailable)?
    }
    pub(crate) fn contextualize(&self, observation: Observation) -> Result<File, ContextError> {
        self.context.build(observation)
    }
}

pub(crate) mod sources;
