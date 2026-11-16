//! Filter-bound keyset cursors with an insertion high-water mark.
use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    filter: String,
    generation: u64,
    after: i64,
    through: i64,
}
pub(super) fn bounds(
    db: &Connection,
    table: &str,
    filter: &impl Serialize,
    page: &Pagination,
) -> InventoryResult<(i64, i64, String)> {
    if page.limit == 0 || page.limit > 1000 {
        return Err(failure(InventoryError::InvalidArgument));
    }
    let bytes = serde_json::to_vec(filter).map_err(|_| failure(InventoryError::InvalidArgument))?;
    let info = sync_info(db)?;
    let generation = info.generation;
    let root: Vec<u8> = db
        .query_row("SELECT local_root FROM sync_info WHERE id=1", [], |r| {
            r.get(0)
        })
        .map_err(sql)?;
    let signature = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                info.sync_id,
                info.account_id,
                info.remote_root_id,
                root,
                bytes
            ))
            .map_err(|_| failure(InventoryError::InvalidCursor))?
        )
    );
    if let Some(text) = &page.after {
        if text.len() > 4096 {
            return Err(failure(InventoryError::InvalidCursor));
        }
        let cursor: Cursor = URL_SAFE_NO_PAD
            .decode(text)
            .ok()
            .and_then(|v| serde_json::from_slice(&v).ok())
            .ok_or_else(|| failure(InventoryError::InvalidCursor))?;
        if cursor.filter != signature
            || cursor.generation != generation
            || cursor.after < 0
            || cursor.through < cursor.after
        {
            return Err(failure(InventoryError::InvalidCursor));
        }
        return Ok((cursor.after, cursor.through, signature));
    }
    let maximum: i64 = db
        .query_row(
            &format!("SELECT COALESCE(MAX(seq),0) FROM {table}"),
            [],
            |r| r.get(0),
        )
        .map_err(sql)?;
    Ok((0, maximum, signature))
}
pub(super) fn next(
    db: &Connection,
    filter: String,
    after: i64,
    through: i64,
) -> InventoryResult<String> {
    let cursor = Cursor {
        filter,
        generation: sync_info(db)?.generation,
        after,
        through,
    };
    Ok(URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&cursor).map_err(|_| failure(InventoryError::SerializationFailed))?,
    ))
}
pub(super) fn enum_text(value: &impl Serialize) -> InventoryResult<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .ok_or_else(|| failure(InventoryError::InvalidArgument))
}
pub(super) fn enum_value<T: serde::de::DeserializeOwned>(text: String) -> InventoryResult<T> {
    serde_json::from_value(serde_json::Value::String(text))
        .map_err(|_| failure(InventoryError::CorruptDatabase))
}
