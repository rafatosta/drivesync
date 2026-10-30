//! Atomic scoped reconciliation observations, independent of installed baselines.
use super::{failed, CheckpointScope, Store};
use crate::{
    changes::Notice,
    local::{Evidence, Inventory},
    Cancellation, ConfirmedBase, DownloadPlan, EngineError, ReconciliationEntry, RemoteInventory,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::{Component, Path},
};
const SCHEMA:&str="CREATE TABLE snapshots(client TEXT NOT NULL,account TEXT NOT NULL,remote TEXT NOT NULL,local TEXT NOT NULL,revision TEXT NOT NULL,record TEXT NOT NULL,PRIMARY KEY(client,account,remote,local));";
const MAX_BYTES: usize = 64 * 1024 * 1024;

/// B/L/R evidence and decisions from preparation, never proof of completed work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Reconciliation {
    pub local: Inventory,
    pub remote: RemoteInventory,
    pub plan: DownloadPlan,
    pub bases: HashMap<String, ConfirmedBase>,
    pub history: Vec<Notice>,
    pub entries: Vec<ReconciliationEntry>,
}
impl Reconciliation {
    /// Carries a disappeared unrepresented local path as an observation only.
    pub(crate) fn new(
        local: Inventory,
        remote: RemoteInventory,
        plan: DownloadPlan,
        bases: HashMap<String, ConfirmedBase>,
        history: Vec<Notice>,
        previous: Option<&Self>,
    ) -> Self {
        let mut entries = crate::changes::classify(&local, &plan, &history);
        let represented: HashSet<_> = entries
            .iter()
            .filter_map(|e| e.relative_path.clone())
            .collect();
        if let Some(previous) = previous {
            for path in previous.local.entries.keys() {
                if !local.entries.contains_key(path) && !represented.contains(path) {
                    entries.push(ReconciliationEntry {
                        relative_path: Some(path.clone()),
                        remote_id: None,
                        kind: if local.observe(path) == crate::LocalObservation::Unknown {
                            crate::ChangeKind::Unknown
                        } else {
                            crate::ChangeKind::LocalMissing
                        },
                    });
                }
            }
        }
        entries.sort_by(|a, b| {
            (&a.relative_path, &a.remote_id).cmp(&(&b.relative_path, &b.remote_id))
        });
        Self {
            local,
            remote,
            plan,
            bases,
            history,
            entries,
        }
    }
}
/// This database cannot confirm installation or advance a remote feed cursor.
pub(crate) struct Reconciliations(Store);
impl Reconciliations {
    pub(crate) fn open(path: &Path) -> Result<Self, EngineError> {
        Ok(Self(Store::open_format(path, 0x44535243, 2, SCHEMA)?))
    }
    pub(crate) fn load(
        &self,
        scope: &CheckpointScope<'_>,
    ) -> Result<Option<(String, Reconciliation)>, EngineError> {
        if scope.client.is_empty()
            || scope.account.0.is_empty()
            || !scope.local.is_absolute()
            || !crate::configuration::valid_id(scope.remote)
        {
            return Err(EngineError::InvalidPlan);
        }
        let row:Option<(String,String)>=self.0.0.query_row("SELECT revision,record FROM snapshots WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4",params![scope.client,scope.account.0,scope.remote,scope.local.to_str().ok_or(EngineError::InvalidPlan)?],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(failed)?;
        row.map(|(revision, json)| {
            if revision.is_empty() || json.len() > MAX_BYTES {
                return Err(EngineError::StateStorage);
            }
            let record: Reconciliation = serde_json::from_str(&json).map_err(failed)?;
            validate(scope, &record).map_err(|_| EngineError::StateStorage)?;
            Ok((revision, record))
        })
        .transpose()
    }
    /// CAS and a single transaction preserve the prior record on failure/cancellation.
    pub(crate) fn save(
        &mut self,
        scope: &CheckpointScope<'_>,
        expected: Option<&str>,
        record: &Reconciliation,
        cancel: &Cancellation,
    ) -> Result<(), EngineError> {
        validate(scope, record)?;
        let json = serde_json::to_string(record).map_err(failed)?;
        if json.len() > MAX_BYTES {
            return Err(EngineError::InventoryLimit);
        }
        crate::local::check(cancel)?;
        let tx = self
            .0
             .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failed)?;
        let actual:Option<String>=tx.query_row("SELECT revision FROM snapshots WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4",params![scope.client,scope.account.0,scope.remote,scope.local.to_str()],|r|r.get(0)).optional().map_err(failed)?;
        if actual.as_deref() != expected {
            return Err(EngineError::SyncBusy);
        }
        let revision = oauth2::CsrfToken::new_random().secret().clone();
        tx.execute("INSERT INTO snapshots VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(client,account,remote,local) DO UPDATE SET revision=excluded.revision,record=excluded.record",params![scope.client,scope.account.0,scope.remote,scope.local.to_str(),revision,json]).map_err(failed)?;
        crate::local::check(cancel)?;
        tx.commit().map_err(failed)
    }
}
fn safe(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.components().all(|p| matches!(p, Component::Normal(_)))
        && !crate::local::ignored(path)
}
fn validate(scope: &CheckpointScope<'_>, record: &Reconciliation) -> Result<(), EngineError> {
    let plan = &record.plan;
    if scope.client.is_empty()
        || scope.account.0.is_empty()
        || !scope.local.is_absolute()
        || &plan.account_id != scope.account
        || plan.remote_root != scope.remote
        || plan.local_root != scope.local
        || plan.entries.len() != record.remote.entries.len()
        || record.local.entries.len() > 100_000
        || record.bases.len() > 100_000
        || record.entries.len() > 300_000
        || record.history.len() > 200_000
    {
        return Err(EngineError::InvalidPlan);
    }
    crate::changes::validate(scope.account, scope.remote, &record.remote)?;
    for (item, entry) in record.remote.entries.iter().zip(&plan.entries) {
        if item.id != entry.remote_id || entry.relative_path.as_ref().is_some_and(|p| !safe(p)) {
            return Err(EngineError::InvalidPlan);
        }
    }
    for (path, evidence) in &record.local.entries {
        if !safe(path) {
            return Err(EngineError::InvalidPlan);
        }
        if let Evidence::Stable { fingerprint, stamp } = evidence {
            if crate::planner::fingerprint_parts(fingerprint)
                .is_none_or(|(_, dev, ino)| dev != stamp.device || ino != stamp.inode)
            {
                return Err(EngineError::InvalidPlan);
            }
        }
    }
    for (id, base) in &record.bases {
        if !crate::configuration::valid_id(id)
            || base.remote_version == 0
            || crate::planner::fingerprint_parts(&base.local_fingerprint).is_none()
            || base.location.as_ref().is_some_and(|location| {
                !safe(&location.relative_path)
                    || location.relative_path.file_name().and_then(|n| n.to_str())
                        != Some(location.remote_name.as_str())
                    || !crate::configuration::valid_id(&location.remote_parent)
            })
        {
            return Err(EngineError::InvalidPlan);
        }
    }
    let mut seen = HashSet::new();
    for entry in &record.entries {
        if entry.relative_path.as_ref().is_some_and(|p| !safe(p))
            || entry
                .remote_id
                .as_ref()
                .is_some_and(|id| !crate::configuration::valid_id(id))
            || !seen.insert((&entry.relative_path, &entry.remote_id))
        {
            return Err(EngineError::InvalidPlan);
        }
    }
    for notice in &record.history {
        if !crate::configuration::valid_id(&notice.id)
            || notice.path.as_ref().is_some_and(|p| !safe(p))
        {
            return Err(EngineError::InvalidPlan);
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests;
