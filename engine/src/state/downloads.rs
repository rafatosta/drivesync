//! Operation intents and confirmed installation evidence, isolated by root scope.
use super::{failed, Store};
use crate::{
    download::{Operation, Phase},
    DownloadPlan, EngineError,
};
use rusqlite::params;
impl Store {
    /// Persists intent/ready/installed evidence before the caller advances effects.
    pub(crate) fn put_operation(
        &self,
        client: &str,
        scope: &DownloadPlan,
        op: &Operation,
    ) -> Result<(), EngineError> {
        let local = scope.local_root.to_str().ok_or(EngineError::InvalidPlan)?;
        let json = serde_json::to_string(op).map_err(failed)?;
        self.0.execute("INSERT INTO downloads VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(client,account,remote,local,operation) DO UPDATE SET record=excluded.record WHERE file=excluded.file",params![client,scope.account_id.0,scope.remote_root,local,op.id,op.item.id,json]).map_err(failed)?;
        Ok(())
    }
    /// Returns journal evidence only; callers must check inode and content again.
    pub(crate) fn operations(
        &self,
        client: &str,
        scope: &DownloadPlan,
    ) -> Result<Vec<Operation>, EngineError> {
        let local = scope.local_root.to_str().ok_or(EngineError::InvalidPlan)?;
        let mut stmt=self.0.prepare("SELECT operation,file,record FROM downloads WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 ORDER BY rowid").map_err(failed)?;
        let rows = stmt
            .query_map(
                params![client, scope.account_id.0, scope.remote_root, local],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )
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
fn valid(op: &Operation) -> bool {
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
            Phase::Ready { sha256, .. } | Phase::Installed { sha256, .. } => {
                sha256.len() == 64 && sha256.bytes().all(|b| b.is_ascii_hexdigit())
            }
        }
}
