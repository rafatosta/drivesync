//! Durable, scoped conflict snapshots and append-only lifecycle events.
use super::{failed, CheckpointScope, Store};
use crate::{
    local::Evidence, Cancellation, ConflictFailure, ConflictHistoryEvent, ConflictHistoryKind,
    ConflictOutcome, ConflictRecord, EngineError,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::path::{Component, Path};

const APP_ID: i64 = 0x44534346;
const SCHEMA: &str = "CREATE TABLE conflicts(client TEXT NOT NULL,account TEXT NOT NULL,remote TEXT NOT NULL,local TEXT NOT NULL,conflict_id TEXT NOT NULL,record TEXT NOT NULL,PRIMARY KEY(client,account,remote,local,conflict_id)); CREATE TABLE events(sequence INTEGER PRIMARY KEY AUTOINCREMENT,client TEXT NOT NULL,account TEXT NOT NULL,remote TEXT NOT NULL,local TEXT NOT NULL,conflict_id TEXT NOT NULL,event_id TEXT NOT NULL,record TEXT NOT NULL,UNIQUE(client,account,remote,local,event_id));";
const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_RECORDS: usize = 100_000;

/// Read-only identity of one account and root pair for history operations.
fn validate_scope<'a>(scope: &CheckpointScope<'a>) -> Result<&'a str, EngineError> {
    if scope.client.is_empty()
        || scope.account.0.is_empty()
        || !crate::configuration::valid_id(scope.remote)
        || !scope.local.is_absolute()
    {
        return Err(EngineError::InvalidPlan);
    }
    scope.local.to_str().ok_or(EngineError::InvalidPlan)
}

fn conflict_id(remote_id: Option<&str>, path: Option<&Path>) -> Result<String, EngineError> {
    let bytes = serde_json::to_vec(&(remote_id, path)).map_err(failed)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn valid_record(record: &ConflictRecord) -> bool {
    !record.conflict_id.is_empty()
        && record.conflict_id.len() == 64
        && record
            .conflict_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        && (record.remote_id.is_some() || record.relative_path.is_some())
        && record
            .remote_id
            .as_deref()
            .is_none_or(crate::configuration::valid_id)
        && record
            .remote_parent_id
            .as_deref()
            .is_none_or(crate::configuration::valid_id)
        && record.remote_name.as_ref().is_none_or(|name| {
            !name.is_empty()
                && name.len() <= 1024
                && !name.bytes().any(|byte| byte.is_ascii_control())
        })
        && record.relative_path.as_ref().is_none_or(|path| {
            !path.as_os_str().is_empty()
                && path
                    .components()
                    .all(|part| matches!(part, Component::Normal(_)))
                && !crate::local::ignored(path)
        })
        && record.local_fingerprint.as_ref().is_none_or(|value| {
            value.len() <= 256 && !value.bytes().any(|byte| byte.is_ascii_control())
        })
        && record.remote_version.is_none_or(|version| version > 0)
        && match record.status {
            crate::ConflictStatus::Pending => record.diagnosis.needs_attention(),
            crate::ConflictStatus::NoLongerConflicting => matches!(
                &record.diagnosis,
                crate::ChangeKind::Unchanged | crate::ChangeKind::Equivalent
            ),
        }
        && record.conflict_id
            == conflict_id(record.remote_id.as_deref(), record.relative_path.as_deref())
                .unwrap_or_default()
        && record.outcomes.iter().all(|outcome| match outcome {
            ConflictOutcome::CopyPreserved {
                copy_path,
                remote_version,
            } => {
                *remote_version > 0
                    && !copy_path.as_os_str().is_empty()
                    && copy_path
                        .components()
                        .all(|part| matches!(part, Component::Normal(_)))
                    && record.relative_path.as_ref().is_some_and(|original| {
                        copy_path.parent() == original.parent()
                            && copy_path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .is_some_and(|name| {
                                    name.starts_with("DriveSync backup conflict-")
                                        && name.ends_with(".bak")
                                })
                    })
            }
            ConflictOutcome::RetryBlocked { .. } => true,
        })
}

/// One private SQLite journal for durable conflict state and audit events.
pub(crate) struct ConflictHistory(Store);

impl ConflictHistory {
    /// Opens a new exact-format journal without modifying incompatible evidence.
    pub(crate) fn open(path: &Path) -> Result<Self, EngineError> {
        Ok(Self(Store::open_format(path, APP_ID, 1, SCHEMA)?))
    }

    /// Stores pending diagnoses atomically; identical evidence is idempotent.
    pub(crate) fn record_pending(
        &mut self,
        scope: &CheckpointScope<'_>,
        reconciliation: &super::Reconciliation,
        cancel: &Cancellation,
    ) -> Result<(), EngineError> {
        let local = validate_scope(scope)?;
        if reconciliation.plan.account_id != *scope.account
            || reconciliation.remote.account_id != *scope.account
            || reconciliation.plan.remote_root != scope.remote
            || reconciliation.remote.root_id != scope.remote
            || reconciliation.plan.local_root != scope.local
        {
            return Err(EngineError::InvalidPlan);
        }
        let mut records = Vec::new();
        for entry in &reconciliation.entries {
            let status = if entry.kind.needs_attention() {
                crate::ConflictStatus::Pending
            } else if matches!(
                &entry.kind,
                crate::ChangeKind::Unchanged | crate::ChangeKind::Equivalent
            ) {
                crate::ConflictStatus::NoLongerConflicting
            } else {
                continue;
            };
            let relative_path = entry.relative_path.clone();
            let remote_id = entry.remote_id.clone();
            let remote_item = remote_id.as_deref().and_then(|id| {
                reconciliation
                    .remote
                    .entries
                    .iter()
                    .find(|remote| remote.id == id)
            });
            let base = remote_id
                .as_deref()
                .and_then(|id| reconciliation.bases.get(id));
            let remote_version = remote_item
                .and_then(|item| item.version)
                .or_else(|| base.map(|item| item.remote_version));
            let remote_name = remote_item.map(|item| item.name.clone()).or_else(|| {
                base.and_then(|item| item.location.as_ref().map(|l| l.remote_name.clone()))
            });
            let remote_parent_id = remote_item.map(|item| item.parent_id.clone()).or_else(|| {
                base.and_then(|item| item.location.as_ref().map(|l| l.remote_parent.clone()))
            });
            let local_fingerprint = relative_path
                .as_deref()
                .and_then(|path| match reconciliation.local.entries.get(path) {
                    Some(Evidence::Stable { fingerprint, .. }) => Some(fingerprint.clone()),
                    _ => None,
                });
            let record = ConflictRecord {
                conflict_id: conflict_id(remote_id.as_deref(), relative_path.as_deref())?,
                remote_id,
                remote_parent_id,
                remote_name,
                relative_path,
                diagnosis: entry.kind.clone(),
                status,
                local_fingerprint,
                remote_version,
                outcomes: Vec::new(),
            };
            if !valid_record(&record) {
                return Err(EngineError::InvalidPlan);
            }
            records.push((record, status));
        }
        records.sort_by(|left, right| left.0.conflict_id.cmp(&right.0.conflict_id));
        if records.len() > MAX_RECORDS {
            return Err(EngineError::InventoryLimit);
        }

        crate::local::check(cancel)?;
        let tx = self
            .0
             .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failed)?;
        for (mut record, status) in records {
            crate::local::check(cancel)?;
            let old_json: Option<String> = tx
                .query_row(
                    "SELECT record FROM conflicts WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 AND conflict_id=?5",
                    params![scope.client, scope.account.0, scope.remote, local, record.conflict_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(failed)?;
            if status == crate::ConflictStatus::NoLongerConflicting && old_json.is_none() {
                continue;
            }
            if let Some(json) = old_json {
                let old: ConflictRecord = serde_json::from_str(&json).map_err(failed)?;
                if !valid_record(&old) {
                    return Err(EngineError::StateStorage);
                }
                if (status == crate::ConflictStatus::Pending
                    && old.status == crate::ConflictStatus::Pending)
                    || status == crate::ConflictStatus::NoLongerConflicting
                {
                    record.outcomes = old.outcomes.clone();
                }
                if record == old {
                    continue;
                }
            } else {
                let count: i64 = tx
                    .query_row(
                        "SELECT count(*) FROM conflicts WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4",
                        params![scope.client, scope.account.0, scope.remote, local],
                        |row| row.get(0),
                    )
                    .map_err(failed)?;
                if count as usize >= MAX_RECORDS {
                    return Err(EngineError::InventoryLimit);
                }
            }
            let json = serde_json::to_string(&record).map_err(failed)?;
            if json.len() > 1024 * 1024 {
                return Err(EngineError::InventoryLimit);
            }
            tx.execute(
                "INSERT INTO conflicts VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(client,account,remote,local,conflict_id) DO UPDATE SET record=excluded.record",
                params![scope.client, scope.account.0, scope.remote, local, record.conflict_id, json],
            )
            .map_err(failed)?;
            let event_kind = match status {
                crate::ConflictStatus::Pending => ConflictHistoryKind::PendingObserved,
                crate::ConflictStatus::NoLongerConflicting => {
                    ConflictHistoryKind::NoLongerConflicting
                }
            };
            insert_event(&tx, scope, local, &record, event_kind)?;
        }
        crate::local::check(cancel)?;
        tx.commit().map_err(failed)
    }

    /// Records a verified effect or sanitized retry failure for an existing item.
    pub(crate) fn record_outcome(
        &mut self,
        scope: &CheckpointScope<'_>,
        remote_id: Option<&str>,
        relative_path: Option<&Path>,
        outcome: ConflictOutcome,
    ) -> Result<(), EngineError> {
        let local = validate_scope(scope)?;
        let id = conflict_id(remote_id, relative_path)?;
        let tx = self
            .0
             .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failed)?;
        let json: String = tx
            .query_row(
                "SELECT record FROM conflicts WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 AND conflict_id=?5",
                params![scope.client, scope.account.0, scope.remote, local, id],
                |row| row.get(0),
            )
            .optional()
            .map_err(failed)?
            .ok_or(EngineError::StateStorage)?;
        let mut record: ConflictRecord = serde_json::from_str(&json).map_err(failed)?;
        if !valid_record(&record) {
            return Err(EngineError::StateStorage);
        }
        if record.outcomes.contains(&outcome) {
            return tx.commit().map_err(failed);
        }
        record.outcomes.push(outcome.clone());
        if !valid_record(&record) {
            return Err(EngineError::InvalidPlan);
        }
        let json = serde_json::to_string(&record).map_err(failed)?;
        tx.execute(
            "UPDATE conflicts SET record=?6 WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 AND conflict_id=?5",
            params![scope.client, scope.account.0, scope.remote, local, id, json],
        )
        .map_err(failed)?;
        let kind = match outcome {
            ConflictOutcome::CopyPreserved { .. } => ConflictHistoryKind::CopyPreserved,
            ConflictOutcome::RetryBlocked { reason } => {
                ConflictHistoryKind::RetryBlocked { reason }
            }
        };
        insert_event(&tx, scope, local, &record, kind)?;
        tx.commit().map_err(failed)
    }

    /// Loads current pending records for this exact account/root scope.
    pub(crate) fn records(
        &self,
        scope: &CheckpointScope<'_>,
    ) -> Result<Vec<ConflictRecord>, EngineError> {
        let local = validate_scope(scope)?;
        let mut stmt = self
            .0
            .0
            .prepare("SELECT conflict_id,record FROM conflicts WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 ORDER BY conflict_id")
            .map_err(failed)?;
        let rows = stmt
            .query_map(
                params![scope.client, scope.account.0, scope.remote, local],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .map_err(failed)?;
        let mut records = Vec::new();
        let mut bytes = 0usize;
        for (index, row) in rows.enumerate() {
            let (id, json) = row.map_err(failed)?;
            bytes = bytes.saturating_add(json.len());
            if index >= MAX_RECORDS || bytes > MAX_BYTES {
                return Err(EngineError::StateStorage);
            }
            let record: ConflictRecord = serde_json::from_str(&json).map_err(failed)?;
            if id != record.conflict_id || !valid_record(&record) {
                return Err(EngineError::StateStorage);
            }
            if record.status == crate::ConflictStatus::Pending {
                records.push(record);
            }
        }
        Ok(records)
    }

    /// Loads append-only events in stable order without exposing other scopes.
    pub(crate) fn events(
        &self,
        scope: &CheckpointScope<'_>,
    ) -> Result<Vec<ConflictHistoryEvent>, EngineError> {
        let local = validate_scope(scope)?;
        let mut stmt = self
            .0
            .0
            .prepare("SELECT sequence,conflict_id,record FROM events WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 ORDER BY sequence")
            .map_err(failed)?;
        let rows = stmt
            .query_map(
                params![scope.client, scope.account.0, scope.remote, local],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .map_err(failed)?;
        let mut events = Vec::new();
        let mut bytes = 0usize;
        for (index, row) in rows.enumerate() {
            let (sequence, id, json) = row.map_err(failed)?;
            bytes = bytes.saturating_add(json.len());
            if index >= MAX_RECORDS || bytes > MAX_BYTES || sequence <= 0 {
                return Err(EngineError::StateStorage);
            }
            let (record, kind): (ConflictRecord, ConflictHistoryKind) =
                serde_json::from_str(&json).map_err(failed)?;
            if id != record.conflict_id || !valid_record(&record) {
                return Err(EngineError::StateStorage);
            }
            events.push(ConflictHistoryEvent {
                sequence: sequence as u64,
                record,
                kind,
            });
        }
        Ok(events)
    }
}

fn insert_event(
    tx: &rusqlite::Transaction<'_>,
    scope: &CheckpointScope<'_>,
    local: &str,
    record: &ConflictRecord,
    kind: ConflictHistoryKind,
) -> Result<(), EngineError> {
    let predecessor: Option<i64> = tx
        .query_row(
            "SELECT max(sequence) FROM events WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 AND conflict_id=?5",
            params![scope.client, scope.account.0, scope.remote, local, record.conflict_id],
            |row| row.get(0),
        )
        .map_err(failed)?;
    let payload = serde_json::to_vec(&(record, &kind, predecessor)).map_err(failed)?;
    let event_id = format!("{:x}", Sha256::digest(payload));
    let json = serde_json::to_string(&(record, kind)).map_err(failed)?;
    if json.len() > 1024 * 1024 {
        return Err(EngineError::InventoryLimit);
    }
    let (count, bytes): (i64, i64) = tx
        .query_row(
            "SELECT count(*),coalesce(sum(length(record)),0) FROM events WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4",
            params![scope.client, scope.account.0, scope.remote, local],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(failed)?;
    if count as usize >= MAX_RECORDS || (bytes as usize).saturating_add(json.len()) > MAX_BYTES {
        return Err(EngineError::InventoryLimit);
    }
    tx.execute(
        "INSERT OR IGNORE INTO events(client,account,remote,local,conflict_id,event_id,record) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![scope.client, scope.account.0, scope.remote, local, record.conflict_id, event_id, json],
    )
    .map_err(failed)?;
    Ok(())
}

impl From<&crate::EngineError> for ConflictFailure {
    fn from(error: &crate::EngineError) -> Self {
        use crate::EngineError as E;
        match error {
            E::DownloadConflict => Self::LocalConflict,
            E::LocalWriting => Self::LocalWriting,
            E::LocalTemporary => Self::LocalTemporary,
            E::ObservationUnknown => Self::ObservationUnknown,
            E::DownloadIntegrity => Self::Integrity,
            E::DownloadIo => Self::LocalIo,
            E::RemoteChanged => Self::RemoteChanged,
            E::StateStorage | E::StateRecoveryPending => Self::StateUnavailable,
            E::Authentication(crate::AuthError::Network | crate::AuthError::TimedOut)
            | E::RootSuspended(crate::RootSuspension::Offline) => Self::RemoteUnavailable,
            _ => Self::ObservationUnknown,
        }
    }
}

#[cfg(test)]
mod tests;
