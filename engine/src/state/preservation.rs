//! Durable conflict copies never participate in installation/baseline queries.
use super::{failed, Store};
use crate::{download::Preservation, DownloadPlan, EngineError};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::{Component, Path};

/// Deterministic identity within the database's client/account/root scope.
pub(crate) fn preservation_key(
    item: &crate::InventoryEntry,
    original: &Path,
) -> Result<String, EngineError> {
    let bytes = serde_json::to_vec(&(item, original)).map_err(failed)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn valid(record: &Preservation) -> bool {
    let op = &record.operation;
    !record.original.as_os_str().is_empty()
        && record
            .original
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        && super::downloads::valid(op)
        && preservation_key(&op.item, &record.original).ok().as_ref() == Some(&op.id)
        && op.relative
            == record
                .original
                .with_file_name(format!("DriveSync backup conflict-{}.bak", op.id))
        && matches!(
            op.phase,
            crate::download::Phase::Started
                | crate::download::Phase::Ready { .. }
                | crate::download::Phase::Installed { .. }
        )
        && op.item.kind == crate::InventoryKind::Binary
        && op.item.version.is_some_and(|v| v > 0)
        && op.item.size.is_some()
        && op
            .item
            .md5_checksum
            .as_ref()
            .is_some_and(|h| h.len() == 32 && h.bytes().all(|b| b.is_ascii_hexdigit()))
}
impl Store {
    /// Persists intent or verified publication in a distinct durable namespace.
    pub(crate) fn put_preservation(
        &self,
        client: &str,
        scope: &DownloadPlan,
        record: &Preservation,
    ) -> Result<(), EngineError> {
        if !valid(record) {
            return Err(EngineError::InvalidPlan);
        }
        let local = scope.local_root.to_str().ok_or(EngineError::InvalidPlan)?;
        let json = serde_json::to_string(record).map_err(failed)?;
        self.0.execute("INSERT INTO conflict_copies VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(client,account,remote,local,operation) DO UPDATE SET record=excluded.record", params![client,scope.account_id.0,scope.remote_root,local,record.operation.id,json]).map_err(failed)?;
        Ok(())
    }
    /// Reads bounded metadata; filesystem identity/content must still be verified.
    pub(crate) fn preservation(
        &self,
        client: &str,
        scope: &DownloadPlan,
        id: &str,
    ) -> Result<Option<Preservation>, EngineError> {
        let local = scope.local_root.to_str().ok_or(EngineError::InvalidPlan)?;
        let json: Option<String> = self.0.query_row("SELECT record FROM conflict_copies WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 AND operation=?5",params![client,scope.account_id.0,scope.remote_root,local,id],|r| r.get(0)).optional().map_err(failed)?;
        json.map(|json| {
            if json.len() > 1024 * 1024 {
                return Err(EngineError::StateStorage);
            }
            let record: Preservation = serde_json::from_str(&json).map_err(failed)?;
            if !valid(&record) || record.operation.id != id {
                return Err(EngineError::StateStorage);
            }
            Ok(record)
        })
        .transpose()
    }
}
