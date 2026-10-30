//! SQLite observations, plans and durable download-operation evidence.
pub(crate) mod recovery;
mod roots;
pub(crate) use roots::Roots;
mod checkpoints;
mod conflict_history;
pub(crate) use conflict_history::ConflictHistory;
mod reconciliation;
pub(crate) use reconciliation::{Reconciliation, Reconciliations};
mod uploads;
pub(crate) use uploads::Uploads;
mod downloads;
mod preservation;
use crate::{DownloadPlan, EngineError, RemoteInventory};
pub(crate) use checkpoints::{Checkpoints, Scope as CheckpointScope};
pub(crate) use preservation::preservation_key;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::{
    fs::{self, DirBuilder, OpenOptions},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
};

const APP_ID: i64 = 0x4453594e;
const SCHEMA:&str="CREATE TABLE roots(client TEXT NOT NULL, account TEXT NOT NULL, remote TEXT NOT NULL, local TEXT NOT NULL, generation INTEGER NOT NULL CHECK(generation>0), PRIMARY KEY(client,account,remote,local));
CREATE TABLE observations(client TEXT NOT NULL,account TEXT NOT NULL,remote TEXT NOT NULL,local TEXT NOT NULL,file TEXT NOT NULL,generation INTEGER NOT NULL,item TEXT NOT NULL,plan TEXT NOT NULL,PRIMARY KEY(client,account,remote,local,file),FOREIGN KEY(client,account,remote,local) REFERENCES roots(client,account,remote,local));
CREATE TABLE downloads(client TEXT NOT NULL,account TEXT NOT NULL,remote TEXT NOT NULL,local TEXT NOT NULL,operation TEXT NOT NULL,file TEXT NOT NULL,record TEXT NOT NULL,PRIMARY KEY(client,account,remote,local,operation),FOREIGN KEY(client,account,remote,local) REFERENCES roots(client,account,remote,local));
CREATE TABLE conflict_copies(client TEXT NOT NULL,account TEXT NOT NULL,remote TEXT NOT NULL,local TEXT NOT NULL,operation TEXT NOT NULL,record TEXT NOT NULL,PRIMARY KEY(client,account,remote,local,operation),FOREIGN KEY(client,account,remote,local) REFERENCES roots(client,account,remote,local));";
fn failed(_: impl std::fmt::Debug) -> EngineError {
    EngineError::StateStorage
}

/// A connection belongs to one worker; SQLite serializes independent workers.
pub(crate) struct Store(Connection);
impl Store {
    /// Opens the private database. Existing corrupt/foreign/empty databases fail closed.
    pub(crate) fn open(path: &Path) -> Result<Self, EngineError> {
        Self::open_format(path, APP_ID, 3, SCHEMA)
    }

    /// Opens one exact internal format, without migrations or automatic resets.
    fn open_format(
        path: &Path,
        app_id: i64,
        schema_version: i64,
        schema: &str,
    ) -> Result<Self, EngineError> {
        if !path.is_absolute() {
            return Err(EngineError::StateStorage);
        }
        let parent = path.parent().ok_or(EngineError::StateStorage)?;
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .map_err(failed)?;
        let created = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
        {
            Ok(file) => {
                file.sync_all().map_err(failed)?;
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => false,
            Err(e) => return Err(failed(e)),
        };
        if !fs::symlink_metadata(path).map_err(failed)?.is_file() {
            return Err(EngineError::StateStorage);
        }
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(failed)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(failed)?;
        conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;")
            .map_err(failed)?;
        if created {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(failed)?;
            tx.execute_batch(schema).map_err(failed)?;
            tx.pragma_update(None, "application_id", app_id)
                .map_err(failed)?;
            tx.pragma_update(None, "user_version", schema_version)
                .map_err(failed)?;
            tx.commit().map_err(failed)?;
            fs::File::open(parent)
                .and_then(|f| f.sync_all())
                .map_err(failed)?;
        }
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(failed)?;
        let app: i64 = conn
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .map_err(failed)?;
        let integrity: String = conn
            .pragma_query_value(None, "quick_check", |r| r.get(0))
            .map_err(failed)?;
        let journal: String = conn
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .map_err(failed)?;
        if version != schema_version || app != app_id || integrity != "ok" || journal != "delete" {
            return Err(EngineError::StateStorage);
        }
        let broken: Option<i64> = conn
            .prepare("PRAGMA foreign_key_check")
            .map_err(failed)?
            .query_row([], |_| Ok(1))
            .optional()
            .map_err(failed)?;
        if broken.is_some() {
            return Err(EngineError::StateStorage);
        }
        Ok(Self(conn))
    }

    /// Atomically advances only this root's observation generation, retaining old
    /// items as unobserved evidence. Never records a confirmed installed base.
    pub(crate) fn save(
        &mut self,
        client: &str,
        inventory: &RemoteInventory,
        plan: &DownloadPlan,
        cancel: &crate::Cancellation,
    ) -> Result<(), EngineError> {
        if client.is_empty()
            || inventory.account_id != plan.account_id
            || inventory.root_id != plan.remote_root
            || inventory.entries.len() != plan.entries.len()
        {
            return Err(EngineError::InvalidPlan);
        }
        let local = plan
            .local_root
            .to_str()
            .filter(|_| plan.local_root.is_absolute())
            .ok_or(EngineError::InvalidPlan)?;
        let mut bytes = 0usize;
        let mut rows = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (item, decision) in inventory.entries.iter().zip(&plan.entries) {
            if item.id != decision.remote_id || !seen.insert(&item.id) {
                return Err(EngineError::InvalidPlan);
            }
            let a = serde_json::to_string(item).map_err(failed)?;
            let b = serde_json::to_string(decision).map_err(failed)?;
            bytes += a.len() + b.len();
            if bytes > 64 * 1024 * 1024 {
                return Err(EngineError::InventoryLimit);
            }
            rows.push((&item.id, a, b));
        }
        let account = &inventory.account_id.0;
        let remote = &inventory.root_id;
        if cancel.is_cancelled() {
            return Err(EngineError::Authentication(crate::AuthError::Cancelled));
        }
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failed)?;
        tx.execute("INSERT INTO roots VALUES(?1,?2,?3,?4,1) ON CONFLICT(client,account,remote,local) DO UPDATE SET generation=generation+1",params![client,account,remote,local]).map_err(failed)?;
        let generation:i64=tx.query_row("SELECT generation FROM roots WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4",params![client,account,remote,local],|r|r.get(0)).map_err(failed)?;
        for (id, item, plan) in rows {
            if cancel.is_cancelled() {
                return Err(EngineError::Authentication(crate::AuthError::Cancelled));
            }
            tx.execute("INSERT INTO observations VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(client,account,remote,local,file) DO UPDATE SET generation=excluded.generation,item=excluded.item,plan=excluded.plan",params![client,account,remote,local,id,generation,item,plan]).map_err(failed)?;
        }
        if cancel.is_cancelled() {
            return Err(EngineError::Authentication(crate::AuthError::Cancelled));
        }
        tx.commit().map_err(failed)
    }
    /// Reads the current generation only; older observations are not deletions.
    pub(crate) fn load(
        &self,
        client: &str,
        account: &crate::AccountId,
        remote: &str,
        local: &Path,
    ) -> Result<Option<DownloadPlan>, EngineError> {
        let snapshot = self.0.unchecked_transaction().map_err(failed)?;
        let local_text = local.to_str().ok_or(EngineError::InvalidPlan)?;
        let generation:Option<i64>=snapshot.query_row("SELECT generation FROM roots WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4",params![client,account.0,remote,local_text],|r|r.get(0)).optional().map_err(failed)?;
        let Some(generation) = generation else {
            return Ok(None);
        };
        let mut stmt=snapshot.prepare("SELECT file,plan FROM observations WHERE client=?1 AND account=?2 AND remote=?3 AND local=?4 AND generation=?5 ORDER BY file").map_err(failed)?;
        let rows = stmt
            .query_map(
                params![client, account.0, remote, local_text, generation],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .map_err(failed)?;
        let mut entries = Vec::new();
        for row in rows {
            let (id, json) = row.map_err(failed)?;
            let entry: crate::PlanEntry = serde_json::from_str(&json).map_err(failed)?;
            if entry.remote_id != id {
                return Err(EngineError::StateStorage);
            }
            entries.push(entry);
        }
        Ok(Some(DownloadPlan {
            account_id: account.clone(),
            remote_root: remote.into(),
            local_root: local.into(),
            entries,
        }))
    }
}

#[cfg(test)]
mod tests;
