//! Individual approved metadata fields retain last conclusive value and latest collection state.
use super::*;
use rusqlite::{params, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
fn encode<T: Serialize>(value: &T) -> InventoryResult<String> {
    serde_json::to_string(value).map_err(|_| failure(InventoryError::SerializationFailed))
}
fn read<T: DeserializeOwned>(value: &str) -> InventoryResult<T> {
    serde_json::from_str(value).map_err(|_| failure(InventoryError::CorruptDatabase))
}
fn put<T: Serialize>(
    db: &Connection,
    id: &str,
    side: i32,
    field: &str,
    value: &MetadataValue<T>,
) -> InventoryResult<()> {
    let text = encode(value)?;
    if text.len() > 1024 * 1024 {
        return Err(failure(InventoryError::StorageFull));
    }
    let conclusive = matches!(value, MetadataValue::Known(_));
    db.execute("INSERT INTO metadata_fields VALUES(?1,?2,?3,?4,?5) ON CONFLICT(item,side,field) DO UPDATE SET latest=excluded.latest,known=COALESCE(excluded.known,metadata_fields.known)",params![id,side,field,text,conclusive.then_some(&text)]).map_err(sql)?;
    Ok(())
}
fn get<T: DeserializeOwned>(
    db: &Connection,
    id: &str,
    side: i32,
    field: &str,
    latest: bool,
) -> InventoryResult<MetadataValue<T>> {
    let text:String=db.query_row("SELECT CASE WHEN ?4 OR latest='\"NotApplicable\"' THEN latest ELSE COALESCE(known,latest) END FROM metadata_fields WHERE item=?1 AND side=?2 AND field=?3",params![id,side,field,latest],|r|r.get(0)).map_err(sql)?;
    read(&text)
}
pub(super) fn local_apply(db: &Connection, id: &str, file: &LocalFile) -> InventoryResult<()> {
    file.validate()
        .map_err(|_| failure(InventoryError::InvalidArgument))?;
    let bytes = file.relative_path.as_bytes();
    let parent = bytes
        .iter()
        .rposition(|b| *b == b'/')
        .map_or(&[][..], |n| &bytes[..n]);
    db.execute("INSERT INTO local_files VALUES(?1,?2,?3) ON CONFLICT(item) DO UPDATE SET path=excluded.path,parent_path=excluded.parent_path",params![id,bytes,parent]).map_err(sql)?;
    macro_rules! fields{($($key:literal=>$value:expr),*$(,)?)=>{$(put(db,id,0,$key,$value)?;)*};}
    fields!("kind"=>&file.kind,"mode"=>&file.mode,"uid"=>&file.owner.uid,"gid"=>&file.owner.gid,"modified"=>&file.times.modified_at,"changed"=>&file.times.changed_at,"created"=>&file.times.created_at,"size"=>&file.storage.size_bytes,"nlink"=>&file.storage.hard_link_count,"inode"=>&file.storage.inode,"device"=>&file.storage.filesystem_device,"attributes"=>&file.flags.statx_attributes,"mask"=>&file.flags.statx_attributes_mask,"target"=>&file.symlink.target);
    Ok(())
}
pub(super) fn remote_apply(db: &Connection, id: &str, file: &RemoteFile) -> InventoryResult<()> {
    file.validate()
        .map_err(|_| failure(InventoryError::InvalidArgument))?;
    if let MetadataValue::Known(payload) = &file.provider_data {
        let allowed = [
            "id",
            "name",
            "mimeType",
            "parents",
            "trashed",
            "version",
            "createdTime",
            "modifiedTime",
            "size",
            "md5Checksum",
            "sha256Checksum",
        ];
        let value: serde_json::Value = serde_json::from_slice(&payload.payload)
            .map_err(|_| failure(InventoryError::SerializationFailed))?;
        if payload.provider.0 != "google"
            || payload.format != "google.drive.detector-file"
            || payload.format_version != 1
            || value
                .as_object()
                .is_none_or(|object| object.keys().any(|key| !allowed.contains(&key.as_str())))
        {
            return Err(failure(InventoryError::InvalidArgument));
        }
    }

    db.execute("INSERT INTO remote_files VALUES(?1,?2) ON CONFLICT(item) DO UPDATE SET provider=excluded.provider",params![id,file.provider.0]).map_err(sql)?;
    macro_rules! fields{($($key:literal=>$value:expr),*$(,)?)=>{$(put(db,id,1,$key,$value)?;)*};}
    fields!("name"=>&file.name,"kind"=>&file.kind,"mime"=>&file.mime_type,"parents"=>&file.parent_ids,"trashed"=>&file.trashed,"version"=>&file.version,"created"=>&file.created_time,"modified"=>&file.modified_time,"size"=>&file.size,"md5"=>&file.md5_checksum,"sha256"=>&file.sha256_checksum,"provider_data"=>&file.provider_data);
    if let MetadataValue::Known(parents) = &file.parent_ids {
        db.execute("DELETE FROM remote_parents WHERE item=?1", [id])
            .map_err(sql)?;
        for parent in parents {
            db.execute(
                "INSERT INTO remote_parents VALUES(?1,?2,?3) ON CONFLICT(item,parent) DO NOTHING",
                params![id, file.reference.account_id.0, parent.0],
            )
            .map_err(sql)?;
        }
    }
    Ok(())
}
pub(super) fn local_load(
    db: &Connection,
    id: &str,
    latest: bool,
) -> InventoryResult<Option<LocalFile>> {
    let identity: Option<(Option<String>, Vec<u8>)> = db
        .query_row(
            "SELECT i.local_id,l.path FROM local_files l JOIN items i ON i.id=l.item WHERE item=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((local_id, path)) = identity else {
        return Ok(None);
    };
    let load = |key| -> InventoryResult<serde_json::Value> {
        serde_json::to_value(get::<serde_json::Value>(db, id, 0, key, latest)?)
            .map_err(|_| failure(InventoryError::CorruptDatabase))
    };
    // Reassemble only the selected LocalFile profile; no file snapshot is serialized in storage.
    let file:LocalFile=serde_json::from_value(serde_json::json!({"local_id":local_id,"relative_path":path,"kind":load("kind")?,"mode":load("mode")?,"owner":{"uid":load("uid")?,"gid":load("gid")?},"times":{"modified_at":load("modified")?,"changed_at":load("changed")?,"created_at":load("created")?},"storage":{"size_bytes":load("size")?,"hard_link_count":load("nlink")?,"inode":load("inode")?,"filesystem_device":load("device")?},"flags":{"statx_attributes":load("attributes")?,"statx_attributes_mask":load("mask")?},"symlink":{"target":load("target")?}})).map_err(|_|failure(InventoryError::CorruptDatabase))?;
    if !latest && file.validate().is_err() {
        // Type-dependent historical fields cannot be combined into a contradictory snapshot.
        let current =
            local_load(db, id, true)?.ok_or_else(|| failure(InventoryError::CorruptDatabase))?;
        let mut coherent = file;
        coherent.kind = current.kind;
        coherent.mode = current.mode;
        coherent.symlink = current.symlink;
        coherent
            .validate()
            .map_err(|_| failure(InventoryError::CorruptDatabase))?;
        return Ok(Some(coherent));
    }
    file.validate()
        .map_err(|_| failure(InventoryError::CorruptDatabase))?;
    Ok(Some(file))
}
pub(super) fn remote_load(
    db: &Connection,
    id: &str,
    latest: bool,
) -> InventoryResult<Option<RemoteFile>> {
    let identity:Option<(String,String,String)>=db.query_row("SELECT i.remote_account,i.remote_id,r.provider FROM remote_files r JOIN items i ON i.id=r.item WHERE item=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(sql)?;
    let Some((account, file_id, provider)) = identity else {
        return Ok(None);
    };
    let file = RemoteFile {
        reference: RemoteFileRef {
            account_id: AccountId(account),
            file_id: RemoteFileId(file_id),
        },
        provider: ProviderId(provider),
        name: get(db, id, 1, "name", latest)?,
        kind: get(db, id, 1, "kind", latest)?,
        mime_type: get(db, id, 1, "mime", latest)?,
        parent_ids: get(db, id, 1, "parents", latest)?,
        trashed: get(db, id, 1, "trashed", latest)?,
        version: get(db, id, 1, "version", latest)?,
        created_time: get(db, id, 1, "created", latest)?,
        modified_time: get(db, id, 1, "modified", latest)?,
        size: get(db, id, 1, "size", latest)?,
        md5_checksum: get(db, id, 1, "md5", latest)?,
        sha256_checksum: get(db, id, 1, "sha256", latest)?,
        provider_data: get(db, id, 1, "provider_data", latest)?,
    };
    file.validate()
        .map_err(|_| failure(InventoryError::CorruptDatabase))?;
    Ok(Some(file))
}
