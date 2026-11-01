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

impl DriveSyncEngine {
    /// Builds a review-only plain-text merge from the exact saved B/L/R versions.
    /// Missing/corrupt ancestors, unsupported formats and changed snapshots fail
    /// without accepting any decision. Call on a worker; upload consent is not used.
    pub fn preview_text_merge(
        &self,
        id: &AccountId,
        record: &crate::ConflictRecord,
        cancel: &Cancellation,
    ) -> Result<crate::TextMergeCandidate, EngineError> {
        self.with_root(id, |folder, config, root| {
            let history = super::resolution::history(folder, config)?;
            let history_scope = crate::state::CheckpointScope {
                client: &config.client_id,
                account: id,
                remote: &folder.remote.id,
                local: &folder.local,
            };
            if !history.records(&history_scope)?.iter().any(|r| r == record) {
                return Err(EngineError::RemoteChanged);
            }
            let store = crate::state::Store::open(&configuration::identity::state_path(
                folder,
                &config.client_id,
            )?)?;
            let scope = super::resolution::scope(folder);
            let remote_id = record
                .remote_id
                .as_deref()
                .ok_or(EngineError::InvalidPlan)?;
            let base = store
                .confirmed_bases(&config.client_id, id, &folder.remote.id, &folder.local)?
                .remove(remote_id)
                .ok_or(EngineError::ObservationUnknown)?;
            let bytes = store
                .common_base(&config.client_id, &scope, remote_id, &base)?
                .ok_or(EngineError::ObservationUnknown)?;
            let inventory = self.inventory(id, cancel)?;
            self.auth.with_access_config(config, id, |token| {
                use crate::resolution::execution::Remote;
                let mut provider =
                    Provider::new(self, folder, config, root, cancel, &inventory, token)?;
                let version = provider.observe(remote_id)?;
                let comparison = crate::comparison::inspect(root, &mut provider, record, cancel)?;
                if provider.observe(remote_id)? != version {
                    return Err(EngineError::RemoteChanged);
                }
                super::resolution::continuity(
                    root,
                    &inventory,
                    record
                        .relative_path
                        .as_ref()
                        .ok_or(EngineError::InvalidPlan)?,
                    record
                        .local_fingerprint
                        .as_deref()
                        .ok_or(EngineError::InvalidPlan)?,
                    &version,
                    &base,
                    cancel,
                )?;
                crate::resolution::text::candidate(
                    &config.client_id,
                    &scope,
                    record,
                    base.clone(),
                    version,
                    bytes.clone(),
                    comparison,
                )
            })
        })
    }
}
