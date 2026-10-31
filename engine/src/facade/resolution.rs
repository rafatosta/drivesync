//! Public decision boundary: displayed evidence is revalidated under root locks.
use super::*;
use crate::resolution::{
    execution::{self, Context, Remote},
    provider::Provider,
    Intent, Plan, RemoteVersion, ResolutionChoice, ResolutionRequest, Stage,
};
use sha2::{Digest, Sha256};
use std::path::Path;

pub(super) fn operation_id() -> String {
    format!(
        "{:x}",
        Sha256::digest(oauth2::CsrfToken::new_random().secret().as_bytes())
    )
}

pub(super) fn scope(folder: &FolderConfiguration) -> crate::DownloadPlan {
    crate::DownloadPlan {
        account_id: folder.account_id.clone(),
        remote_root: folder.remote.id.clone(),
        local_root: folder.local.clone(),
        entries: vec![],
    }
}

/// Reconcile previously accepted effects before a new download plan can see an
/// absent origin or target as permission to create a misleading replacement.
pub(super) fn recover(
    engine: &DriveSyncEngine,
    folder: &FolderConfiguration,
    config: &auth::OAuthConfig,
    root: &crate::download::filesystem::Root,
    store: &crate::state::Store,
    cancel: &Cancellation,
) -> Result<Vec<crate::ResolutionReport>, EngineError> {
    let scope = scope(folder);
    let ctx = Context {
        client: &config.client_id,
        scope: &scope,
        root,
        store,
        cancel,
    };
    crate::resolution::guard::check(&ctx)?;
    let intents = store.resolutions(&config.client_id, &scope)?;
    for intent in intents.iter().filter(|v| v.stage == Stage::Confirmed) {
        complete(folder, config, intent)?;
    }
    let pending: Vec<_> = intents
        .into_iter()
        .filter(|v| !matches!(v.stage, Stage::Confirmed | Stage::Aborted))
        .collect();
    if pending.is_empty() {
        return Ok(vec![]);
    }
    let _ = history(folder, config)?;
    let inventory = engine.inventory(&folder.account_id, cancel)?;
    engine.auth_upload_access(config, &folder.account_id, |token| {
        let mut provider = Provider::new(engine, folder, config, root, cancel, &inventory, token)?;
        let mut reports = Vec::new();
        for intent in pending {
            let result = execution::run(&ctx, &mut provider, intent)?;
            let confirmed = store
                .resolutions(&config.client_id, &scope)?
                .into_iter()
                .find(|v| v.id == result.operation_id)
                .ok_or(EngineError::StateStorage)?;
            complete(folder, config, &confirmed)?;
            reports.push(result);
        }
        Ok(reports)
    })
}

pub(super) fn same_request(intent: &Intent, request: &ResolutionRequest) -> bool {
    intent.plan.conflict.as_ref() == Some(&request.record.conflict_id)
        && intent.plan.choice == request.choice
        && request.record.remote_id.as_ref() == Some(&intent.plan.remote.item.id)
        && request.record.relative_path.as_ref() == Some(&intent.plan.origin)
        && request.record.local_fingerprint.as_ref() == Some(&intent.plan.local_fingerprint)
        && request.record.remote_version == intent.plan.remote.item.version
        && request.record.remote_name.as_ref() == Some(&intent.plan.remote.item.name)
        && request.record.remote_parent_id.as_ref() == Some(&intent.plan.remote.item.parent_id)
}
pub(super) fn history(
    folder: &FolderConfiguration,
    config: &auth::OAuthConfig,
) -> Result<crate::state::ConflictHistory, EngineError> {
    crate::state::ConflictHistory::open(&configuration::identity::metadata_path(
        folder,
        &config.client_id,
        "conflict-history.sqlite3",
    )?)
}
pub(super) fn complete(
    folder: &FolderConfiguration,
    config: &auth::OAuthConfig,
    intent: &Intent,
) -> Result<(), EngineError> {
    history(folder, config)?.complete_resolution(
        &crate::state::CheckpointScope {
            client: &config.client_id,
            account: &folder.account_id,
            remote: &folder.remote.id,
            local: &folder.local,
        },
        intent,
    )
}

/// Revalidates continuity, aliases, a stable parent mapping and same-parent scope.
/// Move/ancestor reconciliation remains the responsibility of SYNC-017.
pub(super) fn continuity(
    root: &crate::download::filesystem::Root,
    inventory: &crate::RemoteInventory,
    origin: &Path,
    fingerprint: &str,
    remote: &RemoteVersion,
    base: &crate::ConfirmedBase,
    cancel: &Cancellation,
) -> Result<(), EngineError> {
    let location = base
        .location
        .as_ref()
        .ok_or(EngineError::ObservationUnknown)?;
    let (old_hash, dev, ino) = crate::planner::fingerprint_parts(&base.local_fingerprint)
        .ok_or(EngineError::ObservationUnknown)?;
    let (hash, current_dev, current_ino) =
        crate::planner::fingerprint_parts(fingerprint).ok_or(EngineError::ObservationUnknown)?;
    if (dev, ino) != (current_dev, current_ino)
        || remote.item.version.is_none_or(|v| v < base.remote_version)
        || remote.item.parent_id != location.remote_parent
    {
        return Err(EngineError::ObservationUnknown);
    }
    let local = crate::local::scan(root, cancel)?;
    if local.observe(origin) != crate::LocalObservation::Verified(fingerprint.into()) {
        return Err(EngineError::DownloadConflict);
    }
    let aliases=local.entries.values().filter(|entry|matches!(entry,crate::local::Evidence::Stable {stamp,..} if (stamp.device,stamp.inode)==(dev,ino))).count();
    if aliases != 1
        || (origin != location.relative_path
            && (hash != old_hash
                || local.observe(&location.relative_path) != crate::LocalObservation::Absent))
    {
        return Err(EngineError::ObservationUnknown);
    }
    let projected =
        crate::planner::plan(inventory, root.path(), |_| crate::LocalObservation::Unknown)?;
    let target = projected
        .entries
        .iter()
        .find(|v| v.remote_id == remote.item.id)
        .and_then(|v| v.relative_path.as_ref())
        .ok_or(EngineError::ObservationUnknown)?;
    if target.parent() != origin.parent() || origin.parent() != location.relative_path.parent() {
        return Err(EngineError::ObservationUnknown);
    }
    Ok(())
}

pub(super) fn copy_name(origin: &Path, id: &str) -> Result<String, EngineError> {
    let stem = origin
        .file_stem()
        .and_then(|v| v.to_str())
        .ok_or(EngineError::InvalidPlan)?;
    let extension = origin
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| format!(".{v}"))
        .unwrap_or_default();
    let name = format!("{stem} (DriveSync remote conflict {id}){extension}");
    if !crate::planner::safe(&name) {
        return Err(EngineError::InvalidPlan);
    }
    Ok(name)
}

impl DriveSyncEngine {
    /// Applies a decision bound to the saved versions displayed by the UI.
    ///
    /// Call on a worker. Both originals are snapshotted before conditional remote
    /// mutation; recovery reconciles a persisted same-ID intention before replay.
    /// A successful return confirms the effects at that time, not future access.
    ///
    /// # Errors
    /// Stale/unknown evidence, unsupported moves/types, permissions, collisions,
    /// root/configuration changes and persistence/transport failures keep data
    /// preserved. An interrupted accepted decision may need a bounded retry.
    pub fn resolve_conflict(
        &self,
        id: &AccountId,
        request: &ResolutionRequest,
        cancel: &Cancellation,
    ) -> Result<crate::ResolutionReport, EngineError> {
        self.with_root(id, |folder, config, root| {
            if crate::state::recovery::uploads_blocked(&folder.local.join(".drivesync"))? {
                return Err(EngineError::StateRecoveryPending);
            }
            let store = crate::state::Store::open(&configuration::identity::state_path(
                folder,
                &config.client_id,
            )?)?;
            let guard_scope = scope(folder);
            crate::resolution::guard::check(&Context {
                client: &config.client_id,
                scope: &guard_scope,
                root,
                store: &store,
                cancel,
            })?;
            let scope = store
                .load(&config.client_id, id, &folder.remote.id, &folder.local)?
                .ok_or(EngineError::InvalidPlan)?;
            let inventory = self.inventory(id, cancel)?;
            self.auth_upload_access(config, id, |token| {
                let mut provider =
                    Provider::new(self, folder, config, root, cancel, &inventory, token)?;
                let ctx = Context {
                    client: &config.client_id,
                    scope: &scope,
                    root,
                    store: &store,
                    cancel,
                };
                if let Some(previous) = store
                    .resolutions(&config.client_id, &scope)?
                    .into_iter()
                    .find(|intent| same_request(intent, request) && intent.stage != Stage::Aborted)
                {
                    let report = execution::run(&ctx, &mut provider, previous)?;
                    let completed = store
                        .resolutions(&config.client_id, &scope)?
                        .into_iter()
                        .find(|intent| intent.id == report.operation_id)
                        .ok_or(EngineError::StateStorage)?;
                    complete(folder, config, &completed)?;
                    return Ok(report);
                }
                let mut journal = history(folder, config)?;
                let history_scope = crate::state::CheckpointScope {
                    client: &config.client_id,
                    account: id,
                    remote: &folder.remote.id,
                    local: &folder.local,
                };
                if !journal
                    .records(&history_scope)?
                    .iter()
                    .any(|record| record == &request.record)
                {
                    return Err(EngineError::RemoteChanged);
                }
                let remote_id = request
                    .record
                    .remote_id
                    .as_deref()
                    .ok_or(EngineError::InvalidPlan)?;
                let origin = request
                    .record
                    .relative_path
                    .as_ref()
                    .ok_or(EngineError::InvalidPlan)?;
                let remote = provider.observe(remote_id)?;
                let fingerprint =
                    crate::download::filesystem::fingerprint_root(root, origin, cancel)?;
                let current = crate::ResolutionObservation {
                    relative_path: origin.clone(),
                    remote: crate::RemoteState::Present {
                        id: remote.item.id.clone(),
                        parent_id: remote.item.parent_id.clone(),
                        name: remote.item.name.clone(),
                        version: remote.item.version.ok_or(EngineError::ObservationUnknown)?,
                    },
                    local: fingerprint
                        .clone()
                        .map_or(crate::LocalState::Missing, |fingerprint| {
                            crate::LocalState::Present { fingerprint }
                        }),
                };
                if journal
                    .revalidate_resolution(&history_scope, &request.record.conflict_id, &current)?
                    .is_err()
                {
                    return Err(EngineError::RemoteChanged);
                }
                let fingerprint = fingerprint.ok_or(EngineError::DownloadConflict)?;
                let base = store
                    .confirmed_bases(&config.client_id, id, &folder.remote.id, &folder.local)?
                    .remove(remote_id)
                    .ok_or(EngineError::ObservationUnknown)?;
                continuity(
                    root,
                    &inventory,
                    origin,
                    &fingerprint,
                    &remote,
                    &base,
                    cancel,
                )?;
                let op_id = operation_id();
                let desired_name = if request.choice == ResolutionChoice::KeepRemote {
                    remote.item.name.clone()
                } else {
                    origin
                        .file_name()
                        .and_then(|v| v.to_str())
                        .ok_or(EngineError::InvalidPlan)?
                        .into()
                };
                let target = origin.with_file_name(&desired_name);
                let (copy_name, copy_id) = if request.choice == ResolutionChoice::KeepBoth {
                    let name = copy_name(origin, &op_id)?;
                    let local = crate::local::scan(root, cancel)?;
                    if local.entries.keys().any(|path| {
                        path.parent() == origin.parent()
                            && path.file_name().and_then(|v| v.to_str()).is_some_and(|v| {
                                crate::planner::path_key(v) == crate::planner::path_key(&name)
                            })
                    }) {
                        return Err(EngineError::DownloadConflict);
                    }
                    (Some(name), Some(provider.generate()?))
                } else {
                    (None, None)
                };
                let plan = Plan {
                    conflict: Some(request.record.conflict_id.clone()),
                    origin: origin.clone(),
                    target,
                    local_fingerprint: fingerprint,
                    remote,
                    choice: request.choice,
                    base: Some(base),
                    desired_name,
                    copy_name,
                    copy_id,
                };
                let intent = execution::begin(&ctx, plan, op_id)?;
                let report = execution::run(&ctx, &mut provider, intent)?;
                let completed = store
                    .resolutions(&config.client_id, &scope)?
                    .into_iter()
                    .find(|intent| intent.id == report.operation_id)
                    .ok_or(EngineError::StateStorage)?;
                complete(folder, config, &completed)?;
                Ok(report)
            })
        })
    }
}
