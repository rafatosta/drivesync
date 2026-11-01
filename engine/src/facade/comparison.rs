//! Account-scoped read-only comparison, independent of upload consent.
use super::*;
use crate::resolution::provider::Provider;

impl DriveSyncEngine {
    /// Inspects the exact pending versions on a worker without accepting a choice.
    /// Files up to 1 MiB per side are verified in memory; larger files return
    /// metadata only. Reading requires access, not upload authorization.
    ///
    /// # Errors
    /// Busy/unavailable roots, stale history, changed versions, transport failures
    /// and integrity failures leave the conflict and original files untouched.
    pub fn compare_conflict(
        &self,
        id: &AccountId,
        record: &crate::ConflictRecord,
        cancel: &Cancellation,
    ) -> Result<crate::VersionComparison, EngineError> {
        self.with_root(id, |folder, config, root| {
            let history = super::resolution::history(folder, config)?;
            let scope = crate::state::CheckpointScope {
                client: &config.client_id,
                account: id,
                remote: &folder.remote.id,
                local: &folder.local,
            };
            if !history.records(&scope)?.iter().any(|r| r == record) {
                return Err(EngineError::RemoteChanged);
            }
            let inventory = self.inventory(id, cancel)?;
            self.auth.with_access_config(config, id, |token| {
                let mut provider =
                    Provider::new(self, folder, config, root, cancel, &inventory, token)?;
                crate::comparison::inspect(root, &mut provider, record, cancel)
            })
        })
    }
}
