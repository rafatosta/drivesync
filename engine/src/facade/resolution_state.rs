//! Offline recovery summaries and explicit resumption of already accepted work.
use super::*;
use crate::resolution::{execution::Context, Stage};

impl DriveSyncEngine {
    /// Reads accepted recovery intentions without contacting Drive or checking
    /// file contents. Saved paths do not promise current copy availability.
    ///
    /// # Errors
    /// Locks, unavailable roots, incompatible/corrupt state and missing guarded
    /// intentions are errors, never an empty successful list or an implicit reset.
    pub fn saved_resolutions(
        &self,
        id: &AccountId,
    ) -> Result<Vec<crate::ResolutionProgress>, EngineError> {
        let folder = self.saved_folder(id)?.ok_or(EngineError::InvalidFolder)?;
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let roots = self.accounts.roots()?;
        let _association = roots.lock(&config.client_id, &folder)?;
        let expected = roots.marker(&config.client_id, &folder)?;
        let root = crate::coordinator::open(&config.client_id, &folder, expected.as_deref())?;
        let store = crate::state::Store::open(&configuration::identity::state_path(
            &folder,
            &config.client_id,
        )?)?;
        let scope = super::resolution::scope(&folder);
        let cancel = Cancellation::new();
        crate::resolution::guard::validate(&Context {
            client: &config.client_id,
            scope: &scope,
            root: &root,
            store: &store,
            cancel: &cancel,
        })?;
        let history = super::resolution::history(&folder, &config)?;
        let pending = history.records(&crate::state::CheckpointScope {
            client: &config.client_id,
            account: id,
            remote: &folder.remote.id,
            local: &folder.local,
        })?;
        Ok(store
            .resolutions(&config.client_id, &scope)?
            .iter()
            .filter(|intent| {
                intent.stage != Stage::Confirmed
                    || pending.iter().any(|record| {
                        record.remote_id.as_ref() == Some(&intent.plan.remote.item.id)
                            && record.relative_path.as_ref() == Some(&intent.plan.origin)
                            && record.local_fingerprint.as_ref()
                                == Some(&intent.plan.local_fingerprint)
                            && record.remote_version == intent.plan.remote.item.version
                    })
            })
            .filter_map(crate::resolution::progress::saved)
            .collect())
    }

    /// Reconciles accepted exact intentions without choosing a new version.
    /// Call on a worker; file effects and final confirmation follow the same
    /// recovery gates and durable retry deadlines as the initial decision.
    ///
    /// # Errors
    /// Returns configuration, identity, access, collision, stale evidence and
    /// storage/transport errors while retaining snapshots and pending authority.
    pub fn resume_resolutions(
        &self,
        id: &AccountId,
        cancel: &Cancellation,
    ) -> Result<Vec<crate::ResolutionReport>, EngineError> {
        self.with_root(id, |folder, config, root| {
            if crate::state::recovery::uploads_blocked(&folder.local.join(".drivesync"))? {
                return Err(EngineError::StateRecoveryPending);
            }
            let store = crate::state::Store::open(&configuration::identity::state_path(
                folder,
                &config.client_id,
            )?)?;
            super::resolution::recover(self, folder, config, root, &store, cancel)
        })
    }
}
