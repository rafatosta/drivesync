//! Operation intents and confirmed installation evidence, isolated by root scope.
use super::{failed, Store};
use crate::{
    download::{Operation, Phase},
    AccountId, ConfirmedBase, DownloadPlan, EngineError,
};
use rusqlite::params;
use std::{collections::HashMap, path::Path};
impl Store {
    /// Persists intent/ready/installed evidence before the caller advances effects.
    pub(crate) fn put_operation(
        &self,
        client: &str,
        scope: &DownloadPlan,
        op: &Operation,
    ) -> Result<(), EngineError> {
        let local = scope.local_root.to_str().ok_or(EngineError::InvalidPlan)?;
        if !valid(op) {
            return Err(EngineError::InvalidPlan);
        }
        let json = serde_json::to_string(op).map_err(failed)?;
        let changed = self.0.execute("INSERT INTO downloads VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(client,account,remote,local,operation) DO UPDATE SET record=excluded.record WHERE file=excluded.file",params![client,scope.account_id.0,scope.remote_root,local,op.id,op.item.id,json]).map_err(failed)?;
        if changed != 1 {
            return Err(EngineError::StateStorage);
        }
        Ok(())
    }
    /// Returns journal evidence only; callers must check inode and content again.
    pub(crate) fn operations(
        &self,
        client: &str,
        scope: &DownloadPlan,
    ) -> Result<Vec<Operation>, EngineError> {
        self.operations_for(
            client,
            &scope.account_id,
            &scope.remote_root,
            &scope.local_root,
        )
    }

    /// Returns only each item's latest durably installed version as its base.
    /// Later incomplete operations invalidate the previous candidate until replay.
    pub(crate) fn confirmed_bases(
        &self,
        client: &str,
        account: &AccountId,
        remote: &str,
        local: &Path,
    ) -> Result<HashMap<String, ConfirmedBase>, EngineError> {
        if client.is_empty() || account.0.is_empty() || remote.is_empty() || !local.is_absolute() {
            return Err(EngineError::InvalidPlan);
        }
        let operations = self.operations_for(client, account, remote, local)?;
        let mut latest = HashMap::new();
        for operation in operations {
            let base = match &operation.phase {
                Phase::Installed {
                    identity, sha256, ..
                }
                | Phase::Updated {
                    identity, sha256, ..
                } if operation.item.kind == crate::InventoryKind::Binary
                    && !operation.item.id.is_empty()
                    && operation.item.version.is_some_and(|version| version > 0) =>
                {
                    operation.item.version.map(|version| ConfirmedBase {
                        remote_version: version,
                        local_fingerprint: identity.fingerprint(sha256),
                        location: Some(crate::ConfirmedLocation {
                            relative_path: operation.relative.clone(),
                            remote_name: operation.item.name.clone(),
                            remote_parent: operation.item.parent_id.clone(),
                        }),
                    })
                }
                _ => None,
            };
            latest.insert(operation.item.id, base);
        }
        let scope = DownloadPlan {
            account_id: account.clone(),
            remote_root: remote.into(),
            local_root: local.into(),
            entries: vec![],
        };
        for intent in self.resolutions(client, &scope)? {
            if !matches!(
                intent.stage,
                crate::resolution::Stage::Confirmed | crate::resolution::Stage::Aborted
            ) {
                latest.remove(&intent.plan.remote.item.id);
                if let Some(copy) = intent.plan.copy_id {
                    latest.remove(&copy);
                }
            }
        }
        Ok(latest
            .into_iter()
            .filter_map(|(id, base)| base.map(|base| (id, base)))
            .collect())
    }

    fn operations_for(
        &self,
        client: &str,
        account: &AccountId,
        remote: &str,
        local: &Path,
    ) -> Result<Vec<Operation>, EngineError> {
        let local = local.to_str().ok_or(EngineError::InvalidPlan)?;
        let mut stmt=self.0.prepare("SELECT operation,file,record FROM downloads WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 ORDER BY rowid").map_err(failed)?;
        let rows = stmt
            .query_map(params![client, account.0, remote, local], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(failed)?;
        let mut result = Vec::new();
        let mut bytes = 0usize;
        for row in rows {
            let (id, file, json) = row.map_err(failed)?;
            bytes += json.len();
            if result.len() >= 100_000 || bytes > 64 * 1024 * 1024 {
                return Err(EngineError::StateStorage);
            }
            let op: Operation = serde_json::from_str(&json).map_err(failed)?;
            if op.id != id || op.item.id != file || !valid(&op) {
                return Err(EngineError::StateStorage);
            }
            result.push(op);
        }
        Ok(result)
    }
}
pub(super) fn valid(op: &Operation) -> bool {
    !op.id.is_empty()
        && !op.relative.as_os_str().is_empty()
        && op
            .relative
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
        && op.temporary.starts_with(".drivesync-")
        && op.temporary.ends_with(".part")
        && op.temporary.len() <= 255
        && op
            .temporary
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && match &op.phase {
            Phase::Started => true,
            Phase::Updating { previous } => previous_valid(previous),
            Phase::UpdateAbandoned { backup } => backup_valid(backup),
            Phase::UpdateReady {
                sha256,
                previous,
                backup,
                ..
            }
            | Phase::Updated {
                sha256,
                previous,
                backup,
                ..
            } => hash_valid(sha256) && previous_valid(previous) && backup_valid(backup),
            Phase::Ready { sha256, .. } | Phase::Installed { sha256, .. } => {
                sha256.len() == 64 && sha256.bytes().all(|b| b.is_ascii_hexdigit())
            }
        }
}

fn hash_valid(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}
fn previous_valid(previous: &crate::download::Previous) -> bool {
    previous.version > 0 && hash_valid(&previous.sha256)
}
fn backup_valid(name: &str) -> bool {
    name.starts_with("DriveSync backup ")
        && name.ends_with(".bak")
        && name.len() <= 255
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b" ._-".contains(&b))
}
