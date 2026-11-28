//! Indexed, account- and root-scoped snapshot lookups.

use crate::models::{Evidence, Path};
use rusqlite::{params, Params};
use std::collections::BTreeSet;

use super::{
    models::{InventoryLookup, InventoryLookupResult, InventoryLookupStatus, InventorySnapshot},
    Database,
};
use crate::inventory::activity::SqliteFailure;

pub(super) fn lookup(
    db: &Database,
    request: InventoryLookup,
    sqlite_failure: &mut Option<SqliteFailure>,
) -> InventoryLookupResult {
    if request.account_id != db.account_id {
        return InventoryLookupResult::unavailable(
            "Requested account is outside this Inventory scope.",
        );
    }
    if request
        .local_path
        .as_ref()
        .is_some_and(|path| path.is_absolute())
    {
        return InventoryLookupResult::new(
            InventoryLookupStatus::Error,
            None,
            Some("Inventory lookup paths must be relative to the sync root.".into()),
        );
    }

    let mut key_matches = Vec::new();
    let scope = (&request.account_id.0, &request.sync_root_id.0);
    let add_result = |set: rusqlite::Result<BTreeSet<i64>>,
                      matches: &mut Vec<BTreeSet<i64>>|
     -> rusqlite::Result<()> { set.map(|ids| matches.push(ids)) };

    if let Some(item_id) = &request.item_id {
        if let Err(error) = add_result(
            ids_for(
                db,
                "SELECT record_id FROM inventory_items
                 WHERE account_id = ?1 AND sync_root_id = ?2 AND item_id = ?3",
                params![scope.0, scope.1, item_id.0],
            ),
            &mut key_matches,
        ) {
            *sqlite_failure = SqliteFailure::classify(&error);
            return lookup_error();
        }
    }
    if let Some(remote_id) = &request.remote_id {
        if let Err(error) = add_result(
            ids_for(
                db,
                "SELECT record_id FROM inventory_items
                 WHERE account_id = ?1 AND sync_root_id = ?2 AND remote_id = ?3",
                params![scope.0, scope.1, remote_id.0],
            ),
            &mut key_matches,
        ) {
            *sqlite_failure = SqliteFailure::classify(&error);
            return lookup_error();
        }
    }
    if let Evidence::Known(identity) = &request.local_identity {
        if let Err(error) = add_result(
            ids_for(
                db,
                "SELECT record_id FROM inventory_items
                 WHERE account_id = ?1 AND sync_root_id = ?2
                   AND local_device_id = ?3 AND local_inode = ?4
                   AND local_mount_id IS ?5",
                params![
                    scope.0,
                    scope.1,
                    identity.device_id.to_string(),
                    identity.inode.to_string(),
                    identity.mount_id.map(|mount| mount.to_string())
                ],
            ),
            &mut key_matches,
        ) {
            *sqlite_failure = SqliteFailure::classify(&error);
            return lookup_error();
        }
    }
    if let Some(path) = &request.local_path {
        let bytes = match path_bytes(path) {
            Ok(bytes) => bytes,
            Err(()) => {
                return InventoryLookupResult::new(
                    InventoryLookupStatus::Error,
                    None,
                    Some("Inventory lookup path cannot be represented safely.".into()),
                );
            }
        };
        if let Err(error) = add_result(
            ids_for(
                db,
                "SELECT record_id FROM inventory_items
                 WHERE account_id = ?1 AND sync_root_id = ?2 AND local_path = ?3",
                params![scope.0, scope.1, bytes],
            ),
            &mut key_matches,
        ) {
            *sqlite_failure = SqliteFailure::classify(&error);
            return lookup_error();
        }
    }

    if key_matches.is_empty() {
        return InventoryLookupResult::new(
            InventoryLookupStatus::Error,
            None,
            Some("At least one item identity key is required.".into()),
        );
    }

    let all_ids = key_matches
        .iter()
        .flat_map(|ids| ids.iter().copied())
        .collect::<BTreeSet<_>>();
    if all_ids.is_empty() {
        return InventoryLookupResult::new(InventoryLookupStatus::NotFound, None, None);
    }
    if all_ids.len() > 1 {
        return ambiguous();
    }
    let record_id = match all_ids.first() {
        Some(record_id) => *record_id,
        None => return InventoryLookupResult::new(InventoryLookupStatus::NotFound, None, None),
    };
    if key_matches.iter().any(|ids| !ids.contains(&record_id)) {
        return ambiguous();
    }

    let record = db.connection.query_row(
        "SELECT snapshot_json, revision FROM inventory_items WHERE record_id = ?1",
        [record_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
    );
    let (json, revision) = match record {
        Ok(record) => record,
        Err(error) => {
            *sqlite_failure = SqliteFailure::classify(&error);
            return InventoryLookupResult::new(
                InventoryLookupStatus::Error,
                None,
                Some("Inventory record could not be read.".into()),
            );
        }
    };
    let mut snapshot: InventorySnapshot = match serde_json::from_str(&json) {
        Ok(snapshot) => snapshot,
        Err(_) => {
            return InventoryLookupResult::new(
                InventoryLookupStatus::Error,
                None,
                Some("Inventory record is invalid.".into()),
            );
        }
    };
    if snapshot.identity.account_id != request.account_id
        || snapshot.identity.sync_root_id != request.sync_root_id
        || request
            .item_id
            .as_ref()
            .is_some_and(|item_id| snapshot.identity.item_id.as_ref() != Some(item_id))
    {
        return InventoryLookupResult::new(
            InventoryLookupStatus::Error,
            None,
            Some("Inventory record scope is inconsistent.".into()),
        );
    }
    snapshot.revision = match u64::try_from(revision) {
        Ok(revision) => revision,
        Err(_) => {
            return InventoryLookupResult::new(
                InventoryLookupStatus::Error,
                None,
                Some("Inventory revision is invalid.".into()),
            );
        }
    };
    InventoryLookupResult::new(InventoryLookupStatus::Found, Some(snapshot), None)
}

fn ids_for(db: &Database, sql: &str, params: impl Params) -> rusqlite::Result<BTreeSet<i64>> {
    let mut statement = db.connection.prepare(sql)?;
    let ids = statement
        .query_map(params, |row| row.get(0))?
        .collect::<rusqlite::Result<BTreeSet<_>>>();
    ids
}

fn ambiguous() -> InventoryLookupResult {
    InventoryLookupResult::new(
        InventoryLookupStatus::Ambiguous,
        None,
        Some("Supplied item keys identify different records.".into()),
    )
}

fn lookup_error() -> InventoryLookupResult {
    InventoryLookupResult::new(
        InventoryLookupStatus::Error,
        None,
        Some("Inventory lookup failed.".into()),
    )
}

fn path_bytes(path: &Path) -> Result<Vec<u8>, ()> {
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
