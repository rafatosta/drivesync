//! Public boundary for authorization, accounts and folder configuration.
use crate::{
    accounts, auth, configuration, remote, remote_detector, AccountId, AccountSummary, AuthError,
    Cancellation, DetectionCheck, DetectionCorpus, DetectionError, EngineError,
    FolderConfiguration, FolderPage, RemoteDelivery, RestoredAccount,
};

/// Coordinates account and folder operations without exposing credentials.
///
/// Blocking operations must run outside the GTK main thread.
#[derive(Default)]
pub struct DriveSyncEngine {
    auth: auth::AuthProvider,
    accounts: accounts::Accounts,
}

impl DriveSyncEngine {
    /// Constructs the engine using the platform's default adapters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts OAuth authorization and records the verified account.
    ///
    /// The browser callback receives no credentials and must report launch
    /// failure. The operation blocks until authorization completes or fails.
    ///
    /// # Errors
    ///
    /// Returns authorization, credential-vault or account-storage errors.
    pub fn add_account(
        &self,
        cancel: &Cancellation,
        open_browser: impl FnOnce(&str) -> Result<(), AuthError>,
    ) -> Result<AccountSummary, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let account = self
            .auth
            .authorize(&config, cancel, open_browser)
            .map_err(EngineError::Authentication)?;
        self.accounts
            .record(&config.client_id, account.clone())
            .map_err(EngineError::Authentication)?;
        Ok(account)
    }

    /// Reads saved profiles without asserting that credentials remain valid.
    ///
    /// # Errors
    ///
    /// Returns OAuth configuration or account-storage errors.
    pub fn list_accounts(&self) -> Result<Vec<AccountSummary>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        self.accounts
            .list(&config.client_id)
            .map_err(EngineError::Authentication)
    }

    /// Reads the local account description without contacting Google or the vault.
    ///
    /// # Errors
    ///
    /// Returns configuration, storage or missing-account errors.
    pub fn account_description(&self, id: &AccountId) -> Result<Option<String>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        self.accounts
            .description(&config.client_id, id)
            .map_err(EngineError::Authentication)
    }

    /// Saves a local description without changing identity, credentials or folders.
    /// Blank input clears the description.
    ///
    /// # Errors
    ///
    /// Returns invalid-description, configuration, storage or missing-account errors.
    pub fn set_account_description(
        &self,
        id: &AccountId,
        value: &str,
    ) -> Result<Option<String>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        self.accounts.set_description(&config.client_id, id, value)
    }

    /// Verifies persisted credentials and refreshes the saved account profile.
    ///
    /// # Errors
    ///
    /// Returns authorization, credential-vault or account-storage errors.
    pub fn verify_account(&self, id: &AccountId) -> Result<AccountSummary, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let account = self
            .auth
            .verify_with_config(&config, id)
            .map_err(EngineError::Authentication)?;
        self.accounts
            .record(&config.client_id, account.clone())
            .map_err(EngineError::Authentication)?;
        Ok(account)
    }

    /// Restores saved profiles, reporting verification failures per account.
    ///
    /// Cached identities do not imply authentication. Only a confirmed missing
    /// vault entry permits pruning a profile; transient failures retain it.
    ///
    /// # Errors
    ///
    /// Returns OAuth configuration, credential-vault or account-storage errors.
    pub fn restore_accounts(&self) -> Result<Vec<RestoredAccount>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        for profile in self
            .accounts
            .list(&config.client_id)
            .map_err(EngineError::Authentication)?
        {
            self.auth
                .prune_if_missing(&config, &profile.id, || {
                    self.accounts.forget(&config.client_id, &profile.id)
                })
                .map_err(EngineError::Authentication)?;
        }
        self.restore_with(&config.client_id, |id| {
            self.auth.verify_with_config(&config, id)
        })
    }

    fn restore_with(
        &self,
        client: &str,
        mut verify: impl FnMut(&AccountId) -> Result<AccountSummary, AuthError>,
    ) -> Result<Vec<RestoredAccount>, EngineError> {
        let profiles = self
            .accounts
            .list(client)
            .map_err(EngineError::Authentication)?;
        let folders = self
            .accounts
            .folders(client)
            .map_err(EngineError::Authentication)?;
        Ok(profiles
            .into_iter()
            .map(|cached| match verify(&cached.id) {
                Ok(account) if account.id == cached.id => {
                    let error = self.accounts.record(client, account.clone()).err();
                    RestoredAccount {
                        folder: folders.iter().find(|f| f.account_id == account.id).cloned(),
                        account,
                        error,
                    }
                }
                Ok(_) => RestoredAccount {
                    folder: folders.iter().find(|f| f.account_id == cached.id).cloned(),
                    account: cached,
                    error: Some(AuthError::IdentityMismatch),
                },
                Err(error) => RestoredAccount {
                    folder: folders.iter().find(|f| f.account_id == cached.id).cloned(),
                    account: cached,
                    error: Some(error),
                },
            })
            .collect())
    }

    /// Lists a page of Drive folders using the selected account's credentials.
    ///
    /// # Errors
    ///
    /// Returns account, provider, authorization or folder validation errors.
    pub fn drive_folders(
        &self,
        id: &AccountId,
        parent: &str,
        page: Option<&str>,
    ) -> Result<FolderPage, EngineError> {
        self.auth
            .with_access(id, |token| remote::list(token, parent, page))
    }

    /// Checks a saved local directory without creating files or contacting Google.
    ///
    /// # Errors
    ///
    /// Returns invalid-folder for absent, inaccessible, relative or non-directory paths.
    pub fn check_local_folder(&self, path: &std::path::Path) -> Result<(), EngineError> {
        if !path.is_absolute() || !std::fs::metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
            return Err(EngineError::InvalidFolder);
        }
        Ok(())
    }

    /// Returns a saved folder pair without asserting current remote access.
    ///
    /// # Errors
    ///
    /// Returns OAuth configuration or account-storage errors.
    pub fn saved_folder(&self, id: &AccountId) -> Result<Option<FolderConfiguration>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        Ok(self
            .accounts
            .folders(&config.client_id)
            .map_err(EngineError::Authentication)?
            .into_iter()
            .find(|f| &f.account_id == id))
    }

    /// Resolves the configured local observation scope without network or vault access.
    ///
    /// This reads local configuration and verifies any portal origin off the GUI
    /// thread. It uses direct read access when already granted, without changing
    /// saved settings or establishing notification coverage.
    /// # Errors
    /// Returns OAuth configuration, account-storage or invalid-scope errors.
    pub fn local_observation_scope(
        &self,
        id: &AccountId,
    ) -> Result<Option<crate::LocalScope>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let folder = self
            .accounts
            .folders(&config.client_id)
            .map_err(EngineError::Authentication)?
            .into_iter()
            .find(|f| &f.account_id == id);
        folder
            .map(|folder| {
                let root = configuration::observation_root(&folder.local, &folder.local_display)?;
                crate::LocalScope::new(&config.client_id, &id.0, &root)
                    .map_err(|_| EngineError::InvalidFolder)
            })
            .transpose()
    }

    /// Reads recorded activity for the selected accounts under this OAuth client.
    pub fn activity_history(
        &self,
        accounts: &[AccountId],
        before: Option<i64>,
    ) -> Result<crate::ActivityPage, EngineError> {
        if accounts.is_empty() {
            return Ok(crate::ActivityPage::default());
        }
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        crate::ActivityHistory::application()
            .and_then(|history| history.page(&config.client_id, accounts, before))
            .map_err(EngineError::ActivityHistory)
    }

    /// Opens portable state for the saved folder pair.
    pub fn folder_state(
        &self,
        id: &AccountId,
        scope: &crate::LocalScope,
    ) -> Result<crate::LocalStateStore, crate::LocalStateError> {
        let folder = self
            .saved_folder(id)
            .map_err(|_| crate::LocalStateError::Storage)?
            .ok_or(crate::LocalStateError::Missing)?;
        let root = configuration::observation_root(&folder.local, &folder.local_display)
            .map_err(|_| crate::LocalStateError::Storage)?;
        if scope.root() != root || scope.account() != id.0 {
            return Err(crate::LocalStateError::Stale);
        }
        crate::LocalStateStore::portable(&folder.local, scope, &folder.remote.id)
    }

    /// Resumes the saved Drive tree, installing and recording new remote items.
    ///
    /// This blocking, explicit operation requires write access to the configured
    /// local directory. Run it while no other program edits or moves that tree.
    ///
    /// # Errors
    /// Returns provider, local I/O, identity or conflict errors. Items already
    /// installed remain on disk, even if their identity could not be saved.
    pub fn initial_download(&self, id: &AccountId) -> Result<crate::InitialDownload, EngineError> {
        self.initial_download_with_progress(id, |_, _| {})
    }

    /// Resumes downloads and reports actual file transfers on the calling thread.
    pub fn initial_download_with_progress(
        &self,
        id: &AccountId,
        notify: impl Fn(&std::path::Path, crate::DownloadProgress),
    ) -> Result<crate::InitialDownload, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let folder = self
            .accounts
            .folders(&config.client_id)
            .map_err(EngineError::Authentication)?
            .into_iter()
            .find(|folder| &folder.account_id == id)
            .ok_or(EngineError::InvalidFolder)?;
        let root = configuration::observation_root(&folder.local, &folder.local_display)?;
        let scope = crate::LocalScope::new(&config.client_id, &id.0, &root)
            .map_err(|_| EngineError::InvalidFolder)?;
        self.auth.with_access_config(&config, id, |token| {
            crate::download::run(token, &folder, &scope, &notify)
        })
    }

    /// Checks the remote cursor, installs new items and applies verified file
    /// edits, renames or trash events. Unverified changes remain pending.
    pub fn poll_remote_downloads(
        &self,
        id: &AccountId,
        notify: impl Fn(&std::path::Path, crate::DownloadProgress),
    ) -> Result<bool, EngineError> {
        use crate::{DetectionCheck, RemoteEventKind, RemoteMembership};
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let folder = self
            .accounts
            .folders(&config.client_id)
            .map_err(EngineError::Authentication)?
            .into_iter()
            .find(|folder| &folder.account_id == id)
            .ok_or(EngineError::InvalidFolder)?;
        self.auth.with_access_config(&config, id, |token| {
            let provider = remote_detector::GoogleDrive::new(token);
            let corpus = remote_detector::configured_corpus(
                &provider,
                &folder.remote.id,
                &config.client_id,
                id,
            )
            .map_err(map_detection_error)?;
            let events = match remote_detector::check(
                &provider,
                &config.client_id,
                id,
                &folder.remote.id,
                &corpus,
                true,
            )
            .map_err(map_detection_error)?
            {
                DetectionCheck::NoChanges => return Ok(false),
                DetectionCheck::Initialized(events) | DetectionCheck::Changes(events) => events,
            };
            let latest: std::collections::HashMap<_, _> = events
                .iter()
                .filter_map(|delivery| delivery.event.file_id.as_ref().map(|id| (id, delivery.id)))
                .collect();
            let arrivals: std::collections::HashSet<_> = events
                .iter()
                .filter_map(|delivery| {
                    let event = &delivery.event;
                    (event.after == RemoteMembership::Inside
                        && (event.kind == RemoteEventKind::Initial
                            || event.before == RemoteMembership::Outside))
                        .then_some(event.file_id.as_ref())
                        .flatten()
                })
                .collect();
            let root = configuration::observation_root(&folder.local, &folder.local_display)?;
            let scope = crate::LocalScope::new(&config.client_id, &id.0, &root)
                .map_err(|_| EngineError::InvalidFolder)?;
            let mut targets = std::collections::HashSet::new();
            let mut acknowledge = Vec::new();
            let mut unresolved = false;
            for delivery in &events {
                let event = &delivery.event;
                let is_latest = event
                    .file_id
                    .as_ref()
                    .is_none_or(|id| latest.get(id) == Some(&delivery.id));
                if !is_latest || event.kind == RemoteEventKind::InitialComplete {
                    acknowledge.push(delivery.id);
                } else {
                    if event.after == RemoteMembership::Inside {
                        if let Some(target) =
                            event.file_id.as_ref().filter(|id| arrivals.contains(id))
                        {
                            if target != &folder.remote.id {
                                targets.insert(target.clone());
                            }
                        }
                    }
                    if event.after == RemoteMembership::Inside
                        && (event.kind == RemoteEventKind::Initial
                            || event.before == RemoteMembership::Outside)
                    {
                        acknowledge.push(delivery.id);
                    } else {
                        match crate::download::reconcile_file(
                            token, &folder, &scope, event, &notify,
                        ) {
                            Ok(true) => acknowledge.push(delivery.id),
                            Ok(false)
                            | Err(
                                EngineError::DownloadConflict | EngineError::DownloadConflictAt(_),
                            ) => unresolved = true,
                            Err(error) => return Err(error),
                        }
                    }
                }
            }
            if !targets.is_empty() {
                crate::download::run_selected(token, &folder, &scope, &notify, Some(&targets))?;
            }
            remote_detector::acknowledge_many(
                &config.client_id,
                id,
                &folder.remote.id,
                &corpus,
                &acknowledge,
            )
            .map_err(map_detection_error)?;
            Ok(unresolved)
        })
    }

    /// Performs a bounded, read-only remote check for the configured folder.
    ///
    /// The caller selects the change-log corpus explicitly. This blocks during
    /// authentication and network I/O and must not run on the GUI thread.
    /// Pending events are retained until individually acknowledged.
    ///
    /// # Errors
    ///
    /// Returns incomplete, retryable, intervention or sanitized account/provider
    /// errors. If a storage commit fails ambiguously, reload pending state
    /// before retrying.
    pub fn check_remote_changes(
        &self,
        id: &AccountId,
        corpus: &DetectionCorpus,
    ) -> Result<DetectionCheck, DetectionError> {
        let config = auth::OAuthConfig::load()
            .map_err(|error| DetectionError::Provider(EngineError::Authentication(error)))?;
        let folder = self
            .accounts
            .folders(&config.client_id)
            .map_err(|error| DetectionError::Provider(EngineError::Authentication(error)))?
            .into_iter()
            .find(|f| &f.account_id == id)
            .ok_or(DetectionError::Intervention)?;
        self.auth
            .with_access_config(&config, id, |token| {
                Ok(remote_detector::check(
                    &remote_detector::GoogleDrive::new(token),
                    &config.client_id,
                    id,
                    &folder.remote.id,
                    corpus,
                    false,
                ))
            })
            .map_err(DetectionError::Provider)?
    }

    /// Reads pending events for a specific account, root and corpus without
    /// requiring network access or acknowledging the events. An old root can
    /// still be queried after a folder reconfiguration.
    ///
    /// # Errors
    ///
    /// Returns configuration, invalid scope or journal errors.
    pub fn remote_events(
        &self,
        id: &AccountId,
        root: &str,
        corpus: &DetectionCorpus,
    ) -> Result<Vec<RemoteDelivery>, DetectionError> {
        let config = auth::OAuthConfig::load()
            .map_err(|error| DetectionError::Provider(EngineError::Authentication(error)))?;
        remote_detector::pending(&config.client_id, id, root, corpus)
    }

    /// Idempotently acknowledges one event in its exact account/root/corpus.
    /// Returns whether a pending event was actually removed.
    ///
    /// # Errors
    ///
    /// Returns configuration, invalid scope or journal errors.
    pub fn acknowledge_remote_event(
        &self,
        id: &AccountId,
        root: &str,
        corpus: &DetectionCorpus,
        event_id: i64,
    ) -> Result<bool, DetectionError> {
        let config = auth::OAuthConfig::load()
            .map_err(|error| DetectionError::Provider(EngineError::Authentication(error)))?;
        remote_detector::acknowledge(&config.client_id, id, root, corpus, event_id)
    }

    /// Validates and saves a remote folder and writable local directory.
    ///
    /// Remote metadata, portal origins and overlap with other
    /// accounts are checked before the association is persisted.
    ///
    /// # Errors
    ///
    /// Returns provider, local access, identity, overlap or persistence errors.
    pub fn configure_folder(
        &self,
        id: &AccountId,
        remote_id: &str,
        local: &std::path::Path,
    ) -> Result<FolderConfiguration, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let remote = self
            .auth
            .with_access_config(&config, id, |token| remote::get(token, remote_id))?;
        let local = configuration::local_folder(local)?;
        let local_display = configuration::local_origin(&local)?;
        let folder = FolderConfiguration {
            account_id: id.clone(),
            remote,
            local,
            local_display,
        };
        self.accounts.configure(&config.client_id, folder.clone())?;
        Ok(folder)
    }

    /// Removes account credentials and app metadata without deleting user files.
    ///
    /// # Errors
    ///
    /// Returns configuration, credential-vault, locking or account-storage errors.
    pub fn sign_out(&self, id: &AccountId) -> Result<(), EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        self.auth
            .sign_out_then(&config, id, || self.accounts.forget(&config.client_id, id))
            .map_err(EngineError::Authentication)
    }
}

#[cfg(test)]
mod tests;

fn map_detection_error(error: DetectionError) -> EngineError {
    match error {
        DetectionError::Provider(error) => error,
        DetectionError::Retry => EngineError::DownloadBusy,
        _ => EngineError::RemoteCheckIncomplete,
    }
}
