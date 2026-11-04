//! Private root-identity registry used by folder configuration.
mod legacy_consent;
mod roots;
pub(crate) use legacy_consent::revoke_upload_consent;
pub(crate) use roots::Roots;

use crate::EngineError;
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::{
    fs::{self, DirBuilder, OpenOptions},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
};

fn failed(_: impl std::fmt::Debug) -> EngineError {
    EngineError::StateStorage
}

/// One verified registry connection. Existing foreign or corrupt data is never reset.
pub(crate) struct Store(Connection);

impl Store {
    fn open_format(
        path: &Path,
        app_id: i64,
        schema_version: i64,
        schema: &str,
    ) -> Result<Self, EngineError> {
        Self::open_verified(path, app_id, schema_version, Some(schema))
    }

    fn open_existing_format(
        path: &Path,
        app_id: i64,
        schema_version: i64,
    ) -> Result<Self, EngineError> {
        Self::open_verified(path, app_id, schema_version, None)
    }

    fn open_verified(
        path: &Path,
        app_id: i64,
        schema_version: i64,
        schema: Option<&str>,
    ) -> Result<Self, EngineError> {
        if !path.is_absolute() {
            return Err(EngineError::StateStorage);
        }
        let parent = path.parent().ok_or(EngineError::StateStorage)?;
        let created = if schema.is_some() {
            DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)
                .map_err(failed)?;
            match OpenOptions::new()
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
            }
        } else {
            false
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
            tx.execute_batch(schema.ok_or(EngineError::StateStorage)?)
                .map_err(failed)?;
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
}
