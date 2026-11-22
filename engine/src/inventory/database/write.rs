//! Atomic Inventory state transitions and optimistic revision checks.

use crate::models::{Evidence, FileIdentity};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};

use super::{
    models::{
        BaselineState, InventorySnapshot, InventoryWrite, InventoryWriteResult, OperationState,
        PersistedEffect, WriteStatus,
    },
    Database,
};

pub(super) fn write(db: &mut Database, request: InventoryWrite) -> InventoryWriteResult {
    let unavailable =
        || InventoryWriteResult::new(WriteStatus::Unavailable, None, request.recorded_at);
    if request.identity.account_id != db.account_id
        || request.identity.account_id.0.trim().is_empty()
        || request.identity.sync_root_id.0.trim().is_empty()
        || request.item_id.0.trim().is_empty()
        || request.identity.item_id.as_ref() != Some(&request.item_id)
        || request.operation_id.0.trim().is_empty()
    {
        return InventoryWriteResult::new(WriteStatus::Rejected, None, request.recorded_at);
    }

    let transaction = match db
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
    {
        Ok(transaction) => transaction,
        Err(_) => return unavailable(),
    };
    let stored = match current_snapshot(&transaction, &db.account_id.0, &request) {
        Ok(stored) => stored,
        Err(()) => return unavailable(),
    };
    let (record_id, current_revision, mut snapshot) = match stored {
        Some((record_id, revision, snapshot)) => (Some(record_id), revision, snapshot),
        None => {
            if request.expected_revision != Some(0) {
                return InventoryWriteResult::new(WriteStatus::Rejected, None, request.recorded_at);
            }
            (
                None,
                0,
                InventorySnapshot {
                    identity: request.identity.clone(),
                    baseline: super::models::Baseline::never_synced(),
                    local_path: None,
                    pending_operations: Vec::new(),
                    prior_conflict: None,
                    revision: 0,
                },
            )
        }
    };

    if let Some(expected) = request.expected_revision {
        if expected != current_revision {
            return InventoryWriteResult::new(
                WriteStatus::RevisionConflict,
                Some(current_revision),
                request.recorded_at,
            );
        }
    }

    if !identity_matches_scope(&snapshot.identity, &request.identity, &request.item_id) {
        return InventoryWriteResult::new(
            WriteStatus::Rejected,
            Some(current_revision),
            request.recorded_at,
        );
    }
    if let PersistedEffect::ConvergenceConfirmed {
        parent: Some(parent),
        ..
    }
    | PersistedEffect::BaselineConfirmed {
        parent: Some(parent),
        ..
    } = &request.effect
    {
        let current = transaction.query_row(
            "SELECT snapshot_json, revision FROM inventory_items WHERE account_id = ?1 AND sync_root_id = ?2 AND item_id = ?3",
            params![request.identity.account_id.0,request.identity.sync_root_id.0,parent.item_id.0],
            |row| Ok((row.get::<_,String>(0)?,row.get::<_,i64>(1)?)),
        ).ok().and_then(|(json,revision)| serde_json::from_str::<InventorySnapshot>(&json).ok().map(|snapshot|(snapshot,revision)));
        if !current.is_some_and(|(snapshot, revision)| {
            revision > 0
                && snapshot.identity.account_id == request.identity.account_id
                && snapshot.identity.sync_root_id == request.identity.sync_root_id
                && snapshot.identity.item_id.as_ref() == Some(&parent.item_id)
                && revision as u64 == parent.revision
                && snapshot.identity.remote_id.as_ref() == Some(&parent.remote_id)
                && snapshot.local_path.as_ref() == Some(&parent.local_path)
        }) {
            return InventoryWriteResult::new(
                WriteStatus::RevisionConflict,
                Some(current_revision),
                request.recorded_at,
            );
        }
    }
    snapshot.revision = current_revision;
    merge_identity(&mut snapshot.identity, &request.identity);

    if !apply_effect(&mut snapshot, &request) {
        return InventoryWriteResult::new(
            WriteStatus::Rejected,
            Some(current_revision),
            request.recorded_at,
        );
    }

    let revision = match current_revision.checked_add(1) {
        Some(revision) => revision,
        None => return unavailable(),
    };
    let revision_sql = match i64::try_from(revision) {
        Ok(revision) => revision,
        Err(_) => return unavailable(),
    };
    snapshot.revision = revision;
    let payload = match serde_json::to_string(&snapshot) {
        Ok(payload) => payload,
        Err(_) => {
            return InventoryWriteResult::new(
                WriteStatus::Rejected,
                Some(current_revision),
                request.recorded_at,
            )
        }
    };
    let indexes = match IndexValues::from_snapshot(&snapshot) {
        Ok(indexes) => indexes,
        Err(()) => {
            return InventoryWriteResult::new(
                WriteStatus::Rejected,
                Some(current_revision),
                request.recorded_at,
            )
        }
    };

    let collision: rusqlite::Result<bool> = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM inventory_items WHERE account_id = ?1 AND sync_root_id = ?2 AND item_id <> ?3 AND ((?4 IS NOT NULL AND remote_id = ?4) OR (?5 IS NOT NULL AND local_path = ?5)))",
        params![request.identity.account_id.0,request.identity.sync_root_id.0,request.item_id.0,indexes.remote_id,indexes.local_path], |row| row.get(0));
    if !matches!(collision, Ok(false)) {
        return InventoryWriteResult::new(
            WriteStatus::RevisionConflict,
            Some(current_revision),
            request.recorded_at,
        );
    }
    let persisted = if let Some(record_id) = record_id {
        transaction.execute(
            "UPDATE inventory_items SET remote_id = ?1, local_device_id = ?2,
                local_inode = ?3, local_mount_id = ?4, local_path = ?5,
                snapshot_json = ?6, revision = ?7
             WHERE record_id = ?8 AND account_id = ?9 AND sync_root_id = ?10",
            params![
                indexes.remote_id,
                indexes.local_device_id,
                indexes.local_inode,
                indexes.local_mount_id,
                indexes.local_path,
                payload,
                revision_sql,
                record_id,
                request.identity.account_id.0,
                request.identity.sync_root_id.0
            ],
        )
    } else {
        transaction.execute(
            "INSERT INTO inventory_items (
                account_id, sync_root_id, item_id, remote_id, local_device_id,
                local_inode, local_mount_id, local_path, snapshot_json, revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                request.identity.account_id.0,
                request.identity.sync_root_id.0,
                request.item_id.0,
                indexes.remote_id,
                indexes.local_device_id,
                indexes.local_inode,
                indexes.local_mount_id,
                indexes.local_path,
                payload,
                revision_sql
            ],
        )
    };
    if persisted.is_err() {
        return unavailable();
    }
    if transaction.commit().is_err() {
        return unavailable();
    }
    InventoryWriteResult::new(WriteStatus::Written, Some(revision), request.recorded_at)
}

fn current_snapshot(
    transaction: &Transaction<'_>,
    account_id: &str,
    request: &InventoryWrite,
) -> Result<Option<(i64, u64, InventorySnapshot)>, ()> {
    let row = transaction
        .query_row(
            "SELECT record_id, revision, snapshot_json FROM inventory_items
             WHERE account_id = ?1 AND sync_root_id = ?2 AND item_id = ?3",
            params![
                account_id,
                request.identity.sync_root_id.0,
                request.item_id.0
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|_| ())?;
    row.map(|(id, revision, json)| {
        let snapshot = serde_json::from_str(&json).map_err(|_| ())?;
        let revision = u64::try_from(revision).map_err(|_| ())?;
        Ok((id, revision, snapshot))
    })
    .transpose()
}

fn identity_matches_scope(
    stored: &FileIdentity,
    incoming: &FileIdentity,
    item_id: &crate::models::ItemId,
) -> bool {
    stored.account_id == incoming.account_id
        && stored.sync_root_id == incoming.sync_root_id
        && stored.item_id.as_ref() == Some(item_id)
}

fn merge_identity(stored: &mut FileIdentity, incoming: &FileIdentity) {
    if incoming.remote_id.is_some() {
        stored.remote_id.clone_from(&incoming.remote_id);
    }
    if matches!(incoming.local_identity, Evidence::Known(_)) {
        stored.local_identity.clone_from(&incoming.local_identity);
    }
}

fn apply_effect(snapshot: &mut InventorySnapshot, request: &InventoryWrite) -> bool {
    match &request.effect {
        PersistedEffect::ItemRegistered { local_path } => {
            if snapshot.revision != 0 {
                return false;
            }
            snapshot.local_path = local_path.clone();
            true
        }
        PersistedEffect::ConvergenceConfirmed {
            baseline, effect, ..
        } => {
            if !effect.verified
                || baseline.state != BaselineState::Synced
                || baseline.confirmed_at.is_none()
                || baseline.local.is_none()
                || baseline.remote.is_none()
                || baseline.local != effect.local
                || baseline.remote != effect.remote
                || snapshot.pending_operations.iter().any(|op| {
                    matches!(
                        op.state,
                        OperationState::Pending
                            | OperationState::Running
                            | OperationState::AppliedUnconfirmed
                    )
                })
            {
                return false;
            }
            snapshot.local_path = baseline.local.as_ref().and_then(|local| local.path.clone());
            snapshot.baseline = baseline.clone();
            true
        }
        PersistedEffect::Pending(operation) => {
            if operation.id != request.operation_id
                || operation.item_id != request.item_id
                || operation.applied_effect.is_some()
            {
                return false;
            }
            if let Some(existing) = snapshot
                .pending_operations
                .iter_mut()
                .find(|existing| existing.id == operation.id)
            {
                if existing.item_id != operation.item_id
                    || existing.action != operation.action
                    || existing.state != OperationState::Pending
                    || operation.state != OperationState::Running
                {
                    return false;
                }
                existing.state = OperationState::Running;
                existing.updated_at = request.recorded_at;
            } else {
                if operation.state != OperationState::Pending
                    || snapshot.pending_operations.iter().any(|existing| {
                        existing.idempotency_key.is_some()
                            && existing.idempotency_key == operation.idempotency_key
                    })
                {
                    return false;
                }
                let mut operation = operation.clone();
                operation.created_at = request.recorded_at;
                operation.updated_at = request.recorded_at;
                snapshot.pending_operations.push(operation);
            }
            true
        }
        PersistedEffect::OperationApplied(effect) => {
            let Some(operation) = snapshot
                .pending_operations
                .iter_mut()
                .find(|operation| operation.id == request.operation_id)
            else {
                return false;
            };
            if operation.state != OperationState::Running {
                return false;
            }
            if effect.verified {
                if let Some(local) = &effect.local {
                    snapshot.local_path = local.path.clone();
                }
            }
            operation.state = OperationState::AppliedUnconfirmed;
            operation.remote_id.clone_from(&effect.remote_id);
            operation.applied_effect = Some(effect.clone());
            operation.updated_at = request.recorded_at;
            if effect.remote_id.is_some() {
                snapshot.identity.remote_id.clone_from(&effect.remote_id);
            }
            true
        }
        PersistedEffect::OperationConfirmed => {
            let Some(operation) = snapshot
                .pending_operations
                .iter_mut()
                .find(|op| op.id == request.operation_id)
            else {
                return false;
            };
            if operation.state != OperationState::AppliedUnconfirmed
                || !operation
                    .applied_effect
                    .as_ref()
                    .is_some_and(|e| e.verified)
            {
                return false;
            }
            operation.state = OperationState::Confirmed;
            operation.updated_at = request.recorded_at;
            true
        }
        PersistedEffect::BaselineConfirmed { baseline, .. } => {
            let Some(operation) = snapshot
                .pending_operations
                .iter_mut()
                .find(|operation| operation.id == request.operation_id)
            else {
                return false;
            };
            if operation.state != OperationState::AppliedUnconfirmed
                || !operation
                    .applied_effect
                    .as_ref()
                    .is_some_and(|effect| effect.verified)
                || baseline.state != BaselineState::Synced
                || baseline.confirmed_at.is_none()
                || baseline.local.is_none()
                || baseline.remote.is_none()
            {
                return false;
            }
            snapshot.local_path = baseline.local.as_ref().and_then(|local| local.path.clone());
            snapshot.baseline = baseline.clone();
            operation.state = OperationState::Confirmed;
            operation.updated_at = request.recorded_at;
            true
        }
        PersistedEffect::OperationFailed => {
            let Some(operation) = snapshot
                .pending_operations
                .iter_mut()
                .find(|operation| operation.id == request.operation_id)
            else {
                return false;
            };
            if !matches!(
                operation.state,
                OperationState::Pending | OperationState::Running
            ) {
                return false;
            }
            operation.state = OperationState::Failed;
            operation.updated_at = request.recorded_at;
            true
        }
        PersistedEffect::ConflictUpdated(conflict) => {
            if conflict
                .as_ref()
                .is_some_and(|conflict| conflict.item_id != request.item_id)
            {
                return false;
            }
            snapshot.prior_conflict.clone_from(conflict);
            true
        }
    }
}

struct IndexValues {
    remote_id: Option<String>,
    local_device_id: Option<String>,
    local_inode: Option<String>,
    local_mount_id: Option<String>,
    local_path: Option<Vec<u8>>,
}

impl IndexValues {
    fn from_snapshot(snapshot: &InventorySnapshot) -> Result<Self, ()> {
        let local_identity = match &snapshot.identity.local_identity {
            Evidence::Known(identity) => Some(identity),
            _ => None,
        };
        let local_path = snapshot.local_path.as_ref().map(path_bytes).transpose()?;
        Ok(Self {
            remote_id: snapshot.identity.remote_id.as_ref().map(|id| id.0.clone()),
            local_device_id: local_identity.map(|identity| identity.device_id.to_string()),
            local_inode: local_identity.map(|identity| identity.inode.to_string()),
            local_mount_id: local_identity
                .and_then(|identity| identity.mount_id)
                .map(|id| id.to_string()),
            local_path,
        })
    }
}

fn path_bytes(path: &crate::models::Path) -> Result<Vec<u8>, ()> {
    if path.is_absolute()
        || path.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(path.as_os_str().as_bytes().to_vec())
    }
    #[cfg(not(unix))]
    {
        path.to_str()
            .map(|value| value.as_bytes().to_vec())
            .ok_or(())
    }
}
