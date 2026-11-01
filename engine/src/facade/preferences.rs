//! Folder-scoped preference access serialized with synchronization.
use super::*;

pub(super) fn store(
    folder: &FolderConfiguration,
    config: &auth::OAuthConfig,
) -> Result<crate::state::Preferences, EngineError> {
    crate::state::Preferences::open(
        &configuration::identity::metadata_path(
            folder,
            &config.client_id,
            "conflict-preferences.sqlite3",
        )?,
        &config.client_id,
        folder,
    )
}
impl DriveSyncEngine {
    /// Reads this exact folder pair's rule without contacting Drive.
    /// Missing consent means Ask; unreadable or invalid data is an error.
    pub fn conflict_preference(
        &self,
        id: &AccountId,
    ) -> Result<crate::ConflictPreference, EngineError> {
        self.with_root(id, |folder, config, _| store(folder, config)?.load())
    }
    /// Saves an explicit rule for the reviewed folder pair, never another pair.
    /// It authorizes only proven same-path binary concurrent edits during upload-
    /// enabled cycles. Ask stops new decisions, not previously accepted recovery.
    ///
    /// # Errors
    /// Changed configuration, unavailable identity/locks and storage failures
    /// reject consent without changing files or contacting Google.
    pub fn set_conflict_preference(
        &self,
        folder: &FolderConfiguration,
        rule: crate::ConflictPreference,
    ) -> Result<(), EngineError> {
        self.with_root(&folder.account_id, |current, config, _| {
            if current != folder {
                return Err(EngineError::InvalidPlan);
            }
            store(current, config)?.save(rule)
        })
    }
}
