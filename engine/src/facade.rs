//! DriveSync engine facade implementation.
//!
//! Coordinates independent account, authentication, Drive and local folder
//! modules without exposing credentials or platform storage to the GUI.
use crate::{
    accounts, auth, configuration, remote, AccountId, AccountSummary, AuthError, Cancellation,
    EngineError, EngineStatus, FolderConfiguration, FolderPage, RestoredAccount, SyncRootId,
};

/// Public boundary for account authorization, restoration and folder setup.
///
/// Blocking calls must be dispatched off the GTK main thread. Internal
/// authentication and storage adapters remain hidden behind this facade.
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
    /// Reads durable suspension without accessing the selected volume or Drive.
    /// Missing external identity is unverified, never implicit permission to bind.
    /// Call off the GTK main thread.
    ///
    /// # Errors
    /// Returns configuration or storage errors without resetting existing state.
    pub fn root_suspension(
        &self,
        id: &AccountId,
    ) -> Result<Option<crate::RootSuspension>, EngineError> {
        let folder = self.saved_folder(id)?.ok_or(EngineError::InvalidFolder)?;
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let roots = self.accounts.roots()?;
        if roots.marker(&config.client_id, &folder)?.is_none() {
            return Ok(Some(crate::RootSuspension::IdentityUnverified));
        }
        roots.reason(&config.client_id, &folder)
    }

    /// Runs one association under a continuous local lock, recording root failures.
    fn with_root<T>(
        &self,
        id: &AccountId,
        f: impl FnOnce(
            &FolderConfiguration,
            &auth::OAuthConfig,
            &crate::download::filesystem::Root,
        ) -> Result<T, EngineError>,
    ) -> Result<T, EngineError> {
        let folder = self.saved_folder(id)?.ok_or(EngineError::InvalidFolder)?;
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let roots = self.accounts.roots()?;
        let _association_lock = roots.lock(&config.client_id, &folder)?;
        let expected = roots.marker(&config.client_id, &folder)?;
        let opened = crate::coordinator::open(&config.client_id, &folder, expected.as_deref());
        let result = match &opened {
            Ok(root) => {
                let result = f(&folder, &config, root).map_err(crate::coordinator::connection);
                crate::coordinator::verify(root, &config.client_id, &folder, expected.as_deref())
                    .and(result)
            }
            Err(error) => Err(error.clone()),
        };
        match &result {
            Err(EngineError::StateStorage) if expected.is_some() => roots.suspend(
                &config.client_id,
                &folder,
                Some(crate::RootSuspension::StateUnavailable),
            )?,
            Err(EngineError::RootSuspended(reason)) if expected.is_some() => {
                roots.suspend(&config.client_id, &folder, Some(*reason))?
            }
            Ok(_) => roots.suspend(&config.client_id, &folder, None)?,
            _ => {}
        }
        result
    }

    /// Rebuilds invalid local state without network access or changing user files.
    /// Original databases and sidecars remain in place. Healthy stores stay active.
    /// Missing installation evidence never grants ownership of existing files.
    /// Call off the UI thread after the user chooses recovery.
    ///
    /// # Errors
    /// Returns identity, lock, cancellation or storage failures. Uploads with
    /// lost intent evidence remain blocked for explicit reconciliation.
    pub fn recover_state(
        &self,
        id: &AccountId,
        cancel: &Cancellation,
    ) -> Result<crate::StateRecovery, EngineError> {
        self.with_root(id, |folder, config, root| {
            root.validate()?;
            let result = crate::state::recovery::rebuild(folder, &config.client_id, cancel)?;
            root.validate()?;
            Ok(result)
        })
    }

    /// Revalidates identity and configuration during transfers, without creating metadata.
    pub(crate) fn verify_root(
        &self,
        folder: &FolderConfiguration,
        config: &auth::OAuthConfig,
    ) -> Result<(), EngineError> {
        if self.saved_folder(&folder.account_id)?.as_ref() != Some(folder) {
            return Err(EngineError::InvalidPlan);
        }
        let expected = self.accounts.roots()?.marker(&config.client_id, folder)?;
        let actual = configuration::identity::verify(folder, &config.client_id)?;
        if expected.as_deref() != Some(actual.as_str()) {
            return Err(EngineError::RootSuspended(
                crate::RootSuspension::IdentityUnverified,
            ));
        }
        Ok(())
    }

    /// Returns the reserved coarse configuration placeholder.
    ///
    /// This does not describe download jobs. Callers must use cycle results and
    /// their application-owned scheduler to present actual synchronization state.
    pub fn status(&self) -> EngineStatus {
        EngineStatus::NotConfigured
    }
    /// Blocking operation: call from a worker, never the GTK main thread.
    /// The browser port must report launch failure; it receives no credentials.
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
    /// Blocking disk read of saved profiles, not a verification of authentication.
    pub fn list_accounts(&self) -> Result<Vec<AccountSummary>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        self.accounts
            .list(&config.client_id)
            .map_err(EngineError::Authentication)
    }
    /// Reads the local account description without contacting Google or the vault.
    ///
    /// # Errors
    /// Returns configuration, storage or missing-account errors.
    pub fn account_description(&self, id: &AccountId) -> Result<Option<String>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        self.accounts
            .description(&config.client_id, id)
            .map_err(EngineError::Authentication)
    }

    /// Saves a local description in the account index using the existing atomic writer.
    ///
    /// Blank input clears the description. This never changes Google identity,
    /// credentials or folder settings and may be used while the account is offline.
    ///
    /// # Errors
    /// Returns invalid-description, configuration, storage or missing-account errors.
    pub fn set_account_description(
        &self,
        id: &AccountId,
        value: &str,
    ) -> Result<Option<String>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        self.accounts.set_description(&config.client_id, id, value)
    }

    /// Reuses persisted credentials, renewing if necessary, without exposing them.
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
    /// Blocking startup operation. Cached profiles never imply authentication.
    pub fn restore_accounts(&self) -> Result<Vec<RestoredAccount>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        // Migrate profiles left by older local logout: prune only a confirmed
        // missing vault entry. Offline, revoked access and locked vault are not absence.
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
    /// Blocking metadata lookup; tokens stay inside auth.
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
    /// Uses the stored access path (including portal paths), not the host display
    /// path. Availability is a point-in-time check and does not prove write access.
    ///
    /// # Errors
    /// Returns invalid-folder for absent, inaccessible, relative or non-directory paths.
    pub fn check_local_folder(&self, path: &std::path::Path) -> Result<(), EngineError> {
        if !path.is_absolute() || !std::fs::metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
            return Err(EngineError::InvalidFolder);
        }
        Ok(())
    }

    /// Inventories the configured remote tree without transferring or modifying files.
    ///
    /// Resolves the root and scopes each request to this account's credentials.
    /// The result is observational; it is not a consistent remote snapshot or
    /// proof of synchronization. Call off the GTK thread. Cancellation is checked
    /// between bounded HTTP requests, not during a blocking request.
    ///
    /// # Errors
    /// Returns invalid-folder for absent/unavailable configuration, inventory-limit,
    /// cancellation, invalid remote metadata, authorization or transport failures.
    /// No partial result is published on failure and no state cursor is advanced.
    pub fn inventory(
        &self,
        id: &AccountId,
        cancel: &crate::Cancellation,
    ) -> Result<crate::RemoteInventory, EngineError> {
        if cancel.is_cancelled() {
            return Err(EngineError::Authentication(AuthError::Cancelled));
        }
        let folder = self.saved_folder(id)?.ok_or(EngineError::InvalidFolder)?;
        self.check_local_folder(&folder.local)?;
        let root = self
            .auth
            .with_access(id, |token| remote::inventory_root(token, &folder.remote.id))?;
        remote::inventory_scan(id, &root, cancel, |parent, page| {
            self.auth
                .with_access(id, |token| remote::inventory_page(token, parent, page))
        })
    }

    /// Inventories and prepares download candidates, then persists observations and
    /// decisions atomically. Existing local files without a confirmed installation
    /// remain conflicts; observations alone never become a base. No user files are
    /// created, replaced or deleted.
    ///
    /// # Errors
    /// Propagates inventory/local errors; rejects changed configurations and corrupt
    /// state. Cancelled or failed preparations never commit a new generation.
    pub fn prepare_downloads(
        &self,
        id: &AccountId,
        cancel: &crate::Cancellation,
    ) -> Result<crate::DownloadPlan, EngineError> {
        self.with_root(id, |folder, config, local_root| {
            let inventory = self.inventory(id, cancel)?;
            if std::fs::symlink_metadata(&folder.local)
                .map_err(|_| EngineError::InvalidFolder)?
                .file_type()
                .is_symlink()
                || std::fs::canonicalize(&folder.local).map_err(|_| EngineError::InvalidFolder)?
                    != folder.local
            {
                return Err(EngineError::InvalidFolder);
            }
            if cancel.is_cancelled() {
                return Err(EngineError::Authentication(AuthError::Cancelled));
            }
            if self.saved_folder(id)?.as_ref() != Some(folder) {
                return Err(EngineError::InvalidPlan);
            }
            let state_path = crate::configuration::identity::state_path(folder, &config.client_id)?;
            let mut store = crate::state::Store::open(&state_path)?;
            let bases =
                store.confirmed_bases(&config.client_id, id, &folder.remote.id, &folder.local)?;
            let local = crate::local::scan(local_root, cancel)?;
            let mut source = DownloadSource {
                engine: self,
                folder,
                config,
                cancel,
                unchanged: std::collections::HashSet::new(),
                preparation: &|_| {},
            };
            let plan = crate::changes::prepare(
                &inventory,
                &folder.local,
                &bases,
                &local,
                &mut source,
                cancel,
            )?;
            let scope = crate::state::CheckpointScope {
                client: &config.client_id,
                account: id,
                remote: &folder.remote.id,
                local: &folder.local,
            };
            let mut reconciliations = crate::state::Reconciliations::open(
                &crate::configuration::identity::metadata_path(
                    folder,
                    &config.client_id,
                    "reconciliation.sqlite3",
                )?,
            )?;
            let previous = reconciliations.load(&scope)?;
            let history = store
                .operations(&config.client_id, &plan)?
                .into_iter()
                .map(|op| crate::changes::Notice {
                    id: op.item.id,
                    path: Some(op.relative),
                })
                .collect();
            let record = crate::state::Reconciliation::new(
                local,
                inventory.clone(),
                plan.clone(),
                bases,
                history,
                previous.as_ref().map(|(_, record)| record),
            );
            if cancel.is_cancelled() {
                return Err(EngineError::Authentication(AuthError::Cancelled));
            }
            if self.saved_folder(id)?.as_ref() != Some(folder) {
                return Err(EngineError::InvalidPlan);
            }
            self.verify_root(folder, config)?;
            crate::download::Source::configured(&mut source)?;
            local_root.validate()?;
            store.save(&config.client_id, &inventory, &plan, cancel)?;
            local_root.validate()?;
            reconciliations.save(
                &scope,
                previous.as_ref().map(|(revision, _)| revision.as_str()),
                &record,
                cancel,
            )?;
            Ok(plan)
        })
    }

    /// Runs one download-only cycle using durable incremental remote observations.
    /// The first cycle reconciles an inventory with the change feed; subsequent
    /// cycles reuse the ID graph and consume new events. Local files are rechecked.
    /// Call off the UI thread.
    /// Binary files are staged and verified. The per-root operation journal is
    /// stored beside the marker in the local root, so it survives app-data resets.
    /// Managed intact copies can be updated after preserving their prior version;
    /// concurrent destinations are never overwritten.
    /// Existing divergent data is reported per item. Partial staging is preserved.
    ///
    /// # Errors
    /// Returns configuration, authorization, cancellation, root lock or state errors.
    /// A failure can follow completed downloads; durable evidence permits replay.
    pub fn download_once(
        &self,
        id: &AccountId,
        cancel: &crate::Cancellation,
        progress: impl FnMut(crate::DownloadProgress),
    ) -> Result<crate::DownloadReport, EngineError> {
        self.with_root(id, |folder, config, root| {
            self.download_locked(id, cancel, progress, folder, config, root)
        })
    }

    fn download_locked(
        &self,
        id: &AccountId,
        cancel: &crate::Cancellation,
        progress: impl FnMut(crate::DownloadProgress),
        folder: &FolderConfiguration,
        config: &auth::OAuthConfig,
        local_root: &crate::download::filesystem::Root,
    ) -> Result<crate::DownloadReport, EngineError> {
        let progress = std::cell::RefCell::new(progress);
        let preparation =
            |stage| (progress.borrow_mut())(crate::DownloadProgress::preparing(stage));
        preparation(crate::PreparationProgress::Connecting);
        let journal_path = crate::configuration::identity::state_path(folder, &config.client_id)?;
        let checkpoint_path =
            crate::configuration::identity::changes_path(folder, &config.client_id)?;
        // Validate both stores before collecting observations or changing user files.
        let mut store = crate::state::Store::open(&journal_path)?;
        let mut checkpoints = crate::state::Checkpoints::open(&checkpoint_path)?;
        let root = self
            .auth
            .with_access_config(config, id, |token| {
                remote::inventory_root(token, &folder.remote.id)
            })
            .map_err(crate::coordinator::remote)?;
        let scope = crate::state::CheckpointScope {
            client: &config.client_id,
            account: id,
            remote: &root,
            local: &folder.local,
        };
        let previous = checkpoints.load(&scope)?;
        let mut source = DownloadSource {
            engine: self,
            folder,
            config,
            cancel,
            unchanged: std::collections::HashSet::new(),
            preparation: &preparation,
        };
        let mut checkpoint = crate::changes::collect(
            id,
            &root,
            previous.as_ref().map(|(_, checkpoint)| checkpoint),
            cancel,
            &mut source,
        )?;
        let inventory = &checkpoint.inventory;
        let bases = store.confirmed_bases(&config.client_id, id, &root, &folder.local)?;
        if let Some((_, old)) = &previous {
            let old: std::collections::HashMap<_, _> = old
                .inventory
                .entries
                .iter()
                .map(|item| (&item.id, item))
                .collect();
            source.unchanged = inventory
                .entries
                .iter()
                .filter(|item| old.get(&item.id).is_some_and(|prior| *prior == *item))
                .map(|item| item.id.clone())
                .collect();
        }
        let local = crate::local::scan(local_root, cancel)?;
        preparation(crate::PreparationProgress::CheckingLocal {
            checked: local.entries.len(),
        });
        let plan = crate::changes::prepare(
            inventory,
            &folder.local,
            &bases,
            &local,
            &mut source,
            cancel,
        )?;
        let mut history = checkpoint.notices.clone();
        for operation in store.operations(&config.client_id, &plan)? {
            history.push(crate::changes::Notice {
                id: operation.item.id,
                path: Some(operation.relative),
            });
        }
        let mut reconciliations =
            crate::state::Reconciliations::open(&crate::configuration::identity::metadata_path(
                folder,
                &config.client_id,
                "reconciliation.sqlite3",
            )?)?;
        let previous_reconciliation = reconciliations.load(&scope)?;
        let reconciliation = crate::state::Reconciliation::new(
            local,
            inventory.clone(),
            plan.clone(),
            bases,
            history,
            previous_reconciliation.as_ref().map(|(_, record)| record),
        );
        preparation(crate::PreparationProgress::SavingState);
        if self.saved_folder(id)?.as_ref() != Some(folder) {
            return Err(EngineError::InvalidPlan);
        }
        self.verify_root(folder, config)?;
        crate::download::Source::configured(&mut source)?;
        local_root.validate()?;
        checkpoints.save(
            &scope,
            previous.as_ref().map(|(revision, _)| revision.as_str()),
            &checkpoint,
            cancel,
        )?;
        local_root.validate()?;
        store.save(&config.client_id, inventory, &plan, cancel)?;
        local_root.validate()?;
        reconciliations.save(
            &scope,
            previous_reconciliation
                .as_ref()
                .map(|(revision, _)| revision.as_str()),
            &reconciliation,
            cancel,
        )?;
        let mut report = crate::download::run_with_root(
            crate::download::Context {
                client: &config.client_id,
                scope: &plan,
                store: &store,
                cancel,
            },
            inventory,
            &mut source,
            |event| (progress.borrow_mut())(event),
            local_root,
        )?;
        crate::coordinator::report(&report)?;
        if report
            .issues
            .iter()
            .any(|issue| issue.error == EngineError::RemoteChanged)
        {
            // A feed observation can lag behind current metadata. Repeating the cached
            // observation cannot repair that mismatch, especially for deleted local
            // files which must pass exact provider validation before downloading.
            let (revision, latest) = checkpoints.load(&scope)?.ok_or(EngineError::StateStorage)?;
            let refreshed = crate::changes::reconcile(id, &root, &latest, cancel, &mut source)?;
            if self.saved_folder(id)?.as_ref() != Some(folder) {
                return Err(EngineError::InvalidPlan);
            }
            checkpoints.save(&scope, Some(&revision), &refreshed, cancel)?;
            checkpoint = refreshed;
        }
        crate::download::Source::configured(&mut source)?;
        local_root.validate()?;
        report.reconciliation = reconciliation.entries;
        report.previous_paths = crate::changes::previous_paths(&checkpoint)?;
        Ok(report)
    }

    /// Returns the exact folder pair's opt-in. Missing settings are download-only.
    pub fn upload_enabled(&self, id: &AccountId) -> Result<bool, EngineError> {
        let Some(folder) = self.saved_folder(id)? else {
            return Ok(false);
        };
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        if crate::state::recovery::uploads_blocked(&folder.local.join(".drivesync"))? {
            return Ok(false);
        }
        crate::state::Uploads::open(
            &crate::configuration::identity::uploads_path(&folder, &config.client_id)?,
            &config.client_id,
            &folder,
        )?
        .enabled()
    }
    /// Enables create-only upload after verifying write consent for this account.
    /// This never changes another account or an old folder pair.
    pub fn set_upload_enabled(
        &self,
        folder: &FolderConfiguration,
        enabled: bool,
    ) -> Result<(), EngineError> {
        if self.saved_folder(&folder.account_id)?.as_ref() != Some(folder) {
            return Err(EngineError::InvalidPlan);
        }
        self.with_root(&folder.account_id, |current, config, root| {
            if current != folder {
                return Err(EngineError::InvalidPlan);
            }
            if enabled && crate::state::recovery::uploads_blocked(&folder.local.join(".drivesync"))?
            {
                return Err(EngineError::StateRecoveryPending);
            }
            if enabled
                && !self
                    .auth
                    .can_upload(config, &folder.account_id)
                    .map_err(EngineError::Authentication)?
            {
                return Err(EngineError::Authentication(AuthError::MissingPermissions));
            }
            crate::state::Uploads::open(
                &crate::configuration::identity::uploads_path(folder, &config.client_id)?,
                &config.client_id,
                folder,
            )?
            .enable_locked(enabled, root)
        })
    }

    pub(crate) fn auth_upload_access<T>(
        &self,
        config: &auth::OAuthConfig,
        id: &AccountId,
        f: impl FnOnce(&str) -> Result<T, EngineError>,
    ) -> Result<T, EngineError> {
        if !self
            .auth
            .can_upload(config, id)
            .map_err(EngineError::Authentication)?
        {
            return Err(EngineError::Authentication(AuthError::MissingPermissions));
        }
        self.auth.with_access_config(config, id, f)
    }
    /// Runs download reconciliation and optional create-only upload, sequentially.
    /// Updates/deletions are never sent. Call off the UI thread.
    pub fn synchronize_once(
        &self,
        id: &AccountId,
        cancel: &Cancellation,
        mut progress: impl FnMut(crate::DownloadProgress),
    ) -> Result<crate::DownloadReport, EngineError> {
        self.with_root(id, |folder, config, root| {
            let mut report =
                self.download_locked(id, cancel, &mut progress, folder, config, root)?;
            crate::coordinator::report(&report)?;
            if !self.upload_enabled(id)? {
                if crate::state::recovery::uploads_blocked(&folder.local.join(".drivesync"))? {
                    report.issues.push(crate::DownloadIssue {
                        relative_path: None,
                        error: EngineError::StateRecoveryPending,
                    });
                }
                return Ok(report);
            }
            let checkpoint_path =
                crate::configuration::identity::changes_path(folder, &config.client_id)?;
            let checkpoints = crate::state::Checkpoints::open(&checkpoint_path)?;
            let scope = crate::state::CheckpointScope {
                client: &config.client_id,
                account: id,
                remote: &folder.remote.id,
                local: &folder.local,
            };
            let (_, checkpoint) = checkpoints.load(&scope)?.ok_or(EngineError::StateStorage)?;
            let uploads = crate::state::Uploads::open(
                &crate::configuration::identity::uploads_path(folder, &config.client_id)?,
                &config.client_id,
                folder,
            )?;
            let mut excluded: std::collections::HashSet<_> = checkpoint
                .notices
                .iter()
                .filter_map(|n| n.path.clone())
                .collect();
            let journal_path =
                crate::configuration::identity::state_path(folder, &config.client_id)?;
            let journal = crate::state::Store::open(&journal_path)?;
            let plan = journal
                .load(&config.client_id, id, &folder.remote.id, &folder.local)?
                .ok_or(EngineError::StateStorage)?;
            let current: std::collections::HashSet<_> = plan
                .entries
                .iter()
                .filter_map(|e| e.relative_path.clone())
                .collect();
            for op in journal.operations(&config.client_id, &plan)? {
                if !current.contains(&op.relative) {
                    excluded.insert(op.relative);
                }
            }
            let journal_path =
                crate::configuration::identity::state_path(folder, &config.client_id)?;
            let mut remote = crate::upload::provider::Provider {
                uploads: &uploads,
                engine: self,
                folder,
                config,
                cancel,
                parents: checkpoint
                    .inventory
                    .entries
                    .iter()
                    .filter(|i| i.kind == crate::InventoryKind::Folder)
                    .map(|i| (i.id.clone(), i.parent_id.clone()))
                    .collect(),
            };
            crate::upload::execution::run_with_root(
                crate::upload::execution::Context {
                    client: &config.client_id,
                    root_path: &folder.local,
                    state_path: &journal_path,
                    inventory: &checkpoint.inventory,
                    uploads: &uploads,
                    cancel,
                    excluded,
                },
                &mut remote,
                &mut report,
                &mut progress,
                root,
            )?;
            crate::coordinator::report(&report)?;
            Ok(report)
        })
    }

    /// Reads the last complete reconciliation without contacting Google.
    /// These historical diagnoses do not authorize writes; every cycle observes anew.
    ///
    /// # Errors
    /// Returns configuration, identity or storage errors without resetting data.
    pub fn saved_reconciliation(
        &self,
        id: &AccountId,
    ) -> Result<Vec<crate::ReconciliationEntry>, EngineError> {
        let folder = self.saved_folder(id)?.ok_or(EngineError::InvalidFolder)?;
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let path = crate::configuration::identity::metadata_path(
            &folder,
            &config.client_id,
            "reconciliation.sqlite3",
        )?;
        let scope = crate::state::CheckpointScope {
            client: &config.client_id,
            account: id,
            remote: &folder.remote.id,
            local: &folder.local,
        };
        Ok(crate::state::Reconciliations::open(&path)?
            .load(&scope)?
            .map(|(_, record)| record.entries)
            .unwrap_or_default())
    }

    /// Loads the current persisted plan for this account's configured root.
    /// It is historical observation only: executors must revalidate all preconditions.
    ///
    /// # Errors
    /// Returns configuration/storage errors without recreating damaged databases.
    pub fn saved_download_plan(
        &self,
        id: &AccountId,
    ) -> Result<Option<crate::DownloadPlan>, EngineError> {
        let folder = self.saved_folder(id)?.ok_or(EngineError::InvalidFolder)?;
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let state_path = crate::configuration::identity::state_path(&folder, &config.client_id)?;
        crate::state::Store::open(&state_path)?.load(
            &config.client_id,
            id,
            &folder.remote.id,
            &folder.local,
        )
    }

    /// Returns the saved folder pair for an account, without remote validation.
    ///
    /// # Errors
    ///
    /// Returns an [`EngineError`] if OAuth configuration or metadata storage
    /// cannot be read.
    pub fn saved_folder(&self, id: &AccountId) -> Result<Option<FolderConfiguration>, EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        Ok(self
            .accounts
            .folders(&config.client_id)
            .map_err(EngineError::Authentication)?
            .into_iter()
            .find(|f| &f.account_id == id))
    }
    /// Configures a remote Drive folder with a writable local directory.
    ///
    /// Verifies remote folder metadata, resolves local/portal paths and rejects
    /// overlap with other accounts' configured roots.
    ///
    /// # Errors
    ///
    /// Returns an [`EngineError`] for provider access, an invalid directory,
    /// unidentified portal origin, root overlap or persistence failure.
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
    /// Blocking local logout: forgets credentials and app settings, never user files.
    pub fn sign_out(&self, id: &AccountId) -> Result<(), EngineError> {
        let config = auth::OAuthConfig::load().map_err(EngineError::Authentication)?;
        let folder = self.saved_folder(id)?;
        self.auth
            .sign_out_then(&config, id, || {
                if let Some(folder) = &folder {
                    crate::state::Uploads::open(
                        &crate::configuration::identity::uploads_path(folder, &config.client_id)
                            .map_err(|_| AuthError::AccountStorage)?,
                        &config.client_id,
                        folder,
                    )
                    .and_then(|store| store.enable(false))
                    .map_err(|_| AuthError::AccountStorage)?;
                }
                self.accounts.forget(&config.client_id, id)
            })
            .map_err(EngineError::Authentication)
    }
    /// Reserved sync entrypoint. File synchronization is not implemented.
    ///
    /// # Errors
    ///
    /// Always returns [`EngineError::NotImplemented`] in the current MVP.
    pub fn start_sync(&self, _root: SyncRootId) -> Result<(), EngineError> {
        Err(EngineError::NotImplemented("start_sync"))
    }
}
#[cfg(test)]
mod live_google_tests;
#[cfg(test)]
mod tests;

/// Holds one account/configuration scope throughout one transfer cycle.
struct DownloadSource<'a> {
    engine: &'a DriveSyncEngine,
    folder: &'a FolderConfiguration,
    config: &'a auth::OAuthConfig,
    cancel: &'a Cancellation,
    unchanged: std::collections::HashSet<String>,
    preparation: &'a dyn Fn(crate::PreparationProgress),
}
impl crate::download::Source for DownloadSource<'_> {
    fn cached_unchanged(&self, item: &crate::InventoryEntry) -> bool {
        self.unchanged.contains(&item.id)
    }
    fn check_configuration(&mut self) -> Result<(), EngineError> {
        self.engine.verify_root(self.folder, self.config)?;
        if self.engine.saved_folder(&self.folder.account_id)?.as_ref() != Some(self.folder) {
            return Err(EngineError::InvalidPlan);
        }
        Ok(())
    }
    fn metadata(
        &mut self,
        item: &crate::InventoryEntry,
    ) -> Result<crate::InventoryEntry, EngineError> {
        self.engine
            .auth
            .with_access_config(self.config, &self.folder.account_id, |token| {
                remote::file_metadata(token, item)
            })
            .map_err(crate::coordinator::connection)
    }
    fn media(&mut self, id: &str) -> Result<Box<dyn std::io::Read>, EngineError> {
        self.engine
            .auth
            .with_access_config(self.config, &self.folder.account_id, |token| {
                Ok(Box::new(remote::file_media(token, id)?) as Box<dyn std::io::Read>)
            })
            .map_err(crate::coordinator::connection)
    }
    fn configured(&mut self) -> Result<(), EngineError> {
        self.engine.verify_root(self.folder, self.config)?;
        if self.engine.saved_folder(&self.folder.account_id)?.as_ref() != Some(self.folder) {
            return Err(EngineError::InvalidPlan);
        }
        let root = self
            .engine
            .auth
            .with_access_config(self.config, &self.folder.account_id, |token| {
                remote::inventory_root(token, &self.folder.remote.id)
            })
            .map_err(crate::coordinator::remote)?;
        if root != self.folder.remote.id {
            return Err(EngineError::RemoteChanged);
        }
        Ok(())
    }
}

/// Metadata collection and transfer use the same credential/configuration scope.
impl crate::changes::Provider for DownloadSource<'_> {
    fn start(&mut self) -> Result<String, EngineError> {
        (self.preparation)(crate::PreparationProgress::ReadingChanges);
        self.engine.auth.with_access_config(
            self.config,
            &self.folder.account_id,
            remote::change_start,
        )
    }
    fn scan(&mut self) -> Result<crate::RemoteInventory, EngineError> {
        (self.preparation)(crate::PreparationProgress::Scanning {
            files: 0,
            folders: 0,
            current_path: std::path::PathBuf::new(),
        });
        let root = self
            .engine
            .auth
            .with_access_config(self.config, &self.folder.account_id, |token| {
                remote::inventory_root(token, &self.folder.remote.id)
            })
            .map_err(crate::coordinator::remote)?;
        remote::inventory_scan_with_progress(
            &self.folder.account_id,
            &root,
            self.cancel,
            |parent, page| {
                self.engine
                    .auth
                    .with_access_config(self.config, &self.folder.account_id, |token| {
                        remote::inventory_page(token, parent, page)
                    })
            },
            |files, folders, path| {
                (self.preparation)(crate::PreparationProgress::Scanning {
                    files,
                    folders,
                    current_path: path.to_path_buf(),
                })
            },
        )
    }
    fn page(&mut self, page: &str) -> Result<remote::ChangePage, EngineError> {
        (self.preparation)(crate::PreparationProgress::ReadingChanges);
        self.engine
            .auth
            .with_access_config(self.config, &self.folder.account_id, |token| {
                remote::change_page(token, page)
            })
    }
}
