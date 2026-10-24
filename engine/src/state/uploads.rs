//! Private durable create-only intents and exact-root opt-in settings.
use super::{failed, Store};
use crate::{upload::Intent, EngineError, FolderConfiguration};
use rusqlite::{params, OptionalExtension};
use std::path::Path;
const SCHEMA: &str = "CREATE TABLE settings(scope TEXT PRIMARY KEY,enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),root TEXT); CREATE TABLE intents(scope TEXT NOT NULL,path TEXT NOT NULL,record TEXT NOT NULL,PRIMARY KEY(scope,path));";

pub(crate) struct Uploads {
    db: Store,
    scope: String,
    local: std::path::PathBuf,
}
impl Uploads {
    pub fn open(
        path: &Path,
        client: &str,
        folder: &FolderConfiguration,
    ) -> Result<Self, EngineError> {
        let scope = serde_json::to_string(&(client, folder)).map_err(failed)?;
        Ok(Self {
            db: Store::open_format(path, 0x44535550, 1, SCHEMA)?,
            scope,
            local: folder.local.clone(),
        })
    }
    pub fn enabled(&self) -> Result<bool, EngineError> {
        self.db
            .0
            .query_row(
                "SELECT enabled FROM settings WHERE scope=?1",
                [&self.scope],
                |r| r.get(0),
            )
            .optional()
            .map(|v| v.unwrap_or(false))
            .map_err(failed)
    }
    pub fn enable(&self, enabled: bool) -> Result<(), EngineError> {
        let root = if enabled {
            let root = crate::download::filesystem::Root::open(&self.local)?;
            Some(
                serde_json::to_string(&crate::download::filesystem::identity(&root.file)?)
                    .map_err(failed)?,
            )
        } else {
            None
        };
        self.db.0.execute("INSERT INTO settings VALUES(?1,?2,?3) ON CONFLICT(scope) DO UPDATE SET enabled=excluded.enabled,root=excluded.root", params![self.scope, enabled, root]).map_err(failed)?;
        Ok(())
    }
    /// The opt-in is bound to the selected inode, not just its pathname.
    pub fn validate_root(
        &self,
        root: &crate::download::filesystem::Root,
    ) -> Result<(), EngineError> {
        let json: Option<String> = self
            .db
            .0
            .query_row(
                "SELECT root FROM settings WHERE scope=?1 AND enabled=1",
                [&self.scope],
                |r| r.get(0),
            )
            .optional()
            .map_err(failed)?
            .flatten();
        let expected: crate::download::Identity =
            serde_json::from_str(&json.ok_or(EngineError::InvalidPlan)?).map_err(failed)?;
        if crate::download::filesystem::identity(&root.file)? != expected {
            return Err(EngineError::DownloadConflict);
        }
        Ok(())
    }
    pub fn load(&self, path: &Path) -> Result<Option<Intent>, EngineError> {
        let json: Option<String> = self
            .db
            .0
            .query_row(
                "SELECT record FROM intents WHERE scope=?1 AND path=?2",
                params![self.scope, path.to_str().ok_or(EngineError::InvalidPlan)?],
                |r| r.get(0),
            )
            .optional()
            .map_err(failed)?;
        json.map(|s| {
            if s.len() > 32 * 1024 {
                return Err(EngineError::StateStorage);
            }
            let op: Intent = serde_json::from_str(&s).map_err(failed)?;
            if op.path != path || !valid(&op) {
                return Err(EngineError::StateStorage);
            }
            Ok(op)
        })
        .transpose()
    }
    /// Each write is durable before the next provider effect. Existing identity
    /// cannot be replaced by a retry or another worker.
    pub fn save(&self, op: &Intent) -> Result<(), EngineError> {
        if !valid(op) {
            return Err(EngineError::InvalidPlan);
        }
        let old = self.load(&op.path)?;
        if old.as_ref().is_some_and(|o| {
            o.root != op.root
                || o.source != op.source
                || o.id != op.id
                || o.parent != op.parent
                || o.sha != op.sha
                || o.directory != op.directory
        }) {
            return Err(EngineError::StateStorage);
        }
        self.db.0.execute("INSERT INTO intents VALUES(?1,?2,?3) ON CONFLICT(scope,path) DO UPDATE SET record=excluded.record", params![self.scope, op.path.to_str().ok_or(EngineError::InvalidPlan)?, serde_json::to_string(op).map_err(failed)?]).map_err(failed)?;
        Ok(())
    }
}
fn valid(op: &Intent) -> bool {
    op.root.inode > 0
        && !op.path.as_os_str().is_empty()
        && op
            .path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
        && crate::configuration::valid_id(&op.id)
        && crate::configuration::valid_id(&op.parent)
        && (op.directory
            || (op.source.is_some_and(|s| s.inode > 0)
                && op.sha.len() == 64
                && op.sha.bytes().all(|b| b.is_ascii_hexdigit())
                && op.md5.len() == 32
                && op.md5.bytes().all(|b| b.is_ascii_hexdigit())
                && op.snapshot == format!("upload-{}.snapshot", op.id)))
        && op
            .session
            .as_ref()
            .is_none_or(|s| crate::upload::valid_session(s))
}
