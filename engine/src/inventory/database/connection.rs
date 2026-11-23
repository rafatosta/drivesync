//! SQLite file setup, schema, and account binding.

use crate::models::AccountId;
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    time::Duration,
};

use crate::inventory::models::InventoryError;

const SCHEMA_VERSION: i64 = 1;

pub(super) fn ensure_private_directory(directory: &Path) -> Result<(), InventoryError> {
    if !directory.exists() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(directory)
            .map_err(|_| InventoryError::Unavailable)?;
    }

    let metadata = fs::symlink_metadata(directory).map_err(|_| InventoryError::Unavailable)?;
    if !metadata.is_dir() {
        return Err(InventoryError::Unavailable);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.mode() & 0o077 != 0 {
            return Err(InventoryError::Unavailable);
        }
    }
    Ok(())
}

pub(super) fn database_path(directory: &Path, account_id: &AccountId) -> PathBuf {
    let digest = Sha256::digest(account_id.0.as_bytes());
    let mut name = String::with_capacity(64 + 12);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(name, "{byte:02x}");
    }
    name.push_str(".sqlite3");
    directory.join(name)
}

pub(super) fn open_connection(path: &Path) -> Result<Connection, InventoryError> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    options
        .open(path)
        .map_err(|_| InventoryError::Unavailable)?;

    let metadata = fs::symlink_metadata(path).map_err(|_| InventoryError::Unavailable)?;
    if !metadata.is_file() {
        return Err(InventoryError::Unavailable);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.mode() & 0o077 != 0 {
            return Err(InventoryError::Unavailable);
        }
    }

    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection =
        Connection::open_with_flags(path, flags).map_err(|_| InventoryError::Unavailable)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|_| InventoryError::Unavailable)?;
    Ok(connection)
}

pub(super) fn initialize(
    connection: &Connection,
    account_id: &AccountId,
) -> Result<(), InventoryError> {
    let transaction = connection
        .unchecked_transaction()
        .map_err(|_| InventoryError::Unavailable)?;
    transaction
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS inventory_meta (
                singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                schema_version INTEGER NOT NULL,
                account_id TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS inventory_items (
                record_id INTEGER PRIMARY KEY,
                account_id TEXT NOT NULL,
                sync_root_id TEXT NOT NULL,
                item_id TEXT NOT NULL,
                remote_id TEXT,
                local_device_id TEXT,
                local_inode TEXT,
                local_mount_id TEXT,
                local_path BLOB,
                snapshot_json TEXT NOT NULL,
                revision INTEGER NOT NULL CHECK (revision > 0),
                UNIQUE (account_id, sync_root_id, item_id)
            );
            CREATE INDEX IF NOT EXISTS inventory_remote_lookup
                ON inventory_items (account_id, sync_root_id, remote_id);
            CREATE INDEX IF NOT EXISTS inventory_local_identity_lookup
                ON inventory_items (
                    account_id, sync_root_id, local_device_id, local_inode, local_mount_id
                );
            CREATE INDEX IF NOT EXISTS inventory_local_path_lookup
                ON inventory_items (account_id, sync_root_id, local_path);",
        )
        .map_err(|_| InventoryError::Unavailable)?;
    transaction
        .execute(
            "INSERT OR IGNORE INTO inventory_meta (singleton, schema_version, account_id)
             VALUES (1, ?1, ?2)",
            rusqlite::params![SCHEMA_VERSION, account_id.0],
        )
        .map_err(|_| InventoryError::Unavailable)?;
    let stored: (i64, String) = transaction
        .query_row(
            "SELECT schema_version, account_id FROM inventory_meta WHERE singleton = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| InventoryError::Unavailable)?;
    if stored.0 != SCHEMA_VERSION || stored.1 != account_id.0 {
        return Err(InventoryError::Unavailable);
    }
    transaction
        .commit()
        .map_err(|_| InventoryError::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_file_names_are_opaque_and_stable() {
        let directory = Path::new("/private/inventory");
        let first = database_path(directory, &AccountId("account/a".into()));
        let same = database_path(directory, &AccountId("account/a".into()));
        let other = database_path(directory, &AccountId("account-b".into()));
        assert_eq!(first, same);
        assert_ne!(first, other);
        assert!(first
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with(".sqlite3"));
        assert!(!first.to_string_lossy().contains("account/a"));
    }
}
