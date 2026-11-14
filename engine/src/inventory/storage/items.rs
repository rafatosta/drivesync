//! Indexed item identity, explicit associations and optimistic observation updates.
use super::*;
use super::{
    metadata,
    pagination::{self, enum_text, enum_value},
};
use rusqlite::{params, types::Value, OptionalExtension};
use sha2::{Digest, Sha256};
use std::io::Read;
pub(super) fn resolve(
    db: &Connection,
    selector: &ItemSelector,
) -> InventoryResult<Option<SyncItemId>> {
    let value: Option<String> = match selector {
        ItemSelector::Id(id) => db
            .query_row("SELECT id FROM items WHERE id=?1", [&id.0], |r| r.get(0))
            .optional(),
        ItemSelector::RemoteRef(reference) => {
            if reference.account_id != sync_info(db)?.account_id {
                return Err(failure(InventoryError::InvalidIdentity));
            }
            db.query_row(
                "SELECT id FROM items WHERE remote_account=?1 AND remote_id=?2",
                params![reference.account_id.0, reference.file_id.0],
                |r| r.get(0),
            )
            .optional()
        }
        ItemSelector::LocalId(id) => db
            .query_row("SELECT id FROM items WHERE local_id=?1", [&id.0], |r| {
                r.get(0)
            })
            .optional(),
        ItemSelector::LocalRelativePath(path) => db
            .query_row(
                "SELECT item FROM local_files WHERE path=?1",
                [path.as_bytes()],
                |r| r.get(0),
            )
            .optional(),
    }
    .map_err(sql)?;
    Ok(value.map(SyncItemId))
}
pub(super) fn load(db: &Connection, id: &SyncItemId) -> InventoryResult<SyncItem> {
    let mut item=db.query_row("SELECT kind,local_presence,remote_presence,status,conflict_reason,revision,last_checked,last_synced,local_id,remote_account,remote_id FROM items WHERE id=?1",[&id.0],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,u64>(5)?,r.get::<_,Option<i64>>(6)?,r.get::<_,Option<i64>>(7)?,r.get::<_,Option<String>>(8)?,r.get::<_,Option<String>>(9)?,r.get::<_,Option<String>>(10)?))).optional().map_err(sql)?.map(|r|->InventoryResult<SyncItem>{Ok(SyncItem{id:id.clone(),kind:enum_value(r.0)?,local_presence:enum_value(r.1)?,remote_presence:enum_value(r.2)?,status:enum_value(r.3)?,conflict_reason:r.4,revision:r.5,last_checked_at:r.6.map(|unix_millis|Timestamp{unix_millis}),last_synced_at:r.7.map(|unix_millis|Timestamp{unix_millis}),local_file_id:r.8.map(LocalFileId),remote_ref:match(r.9,r.10){(Some(a),Some(f))=>Some(RemoteFileRef{account_id:AccountId(a),file_id:RemoteFileId(f)}),_=>None},local:None,remote:None,local_latest:None,remote_latest:None,baseline:None})}).transpose()?.ok_or_else(||failure(InventoryError::NotFound))?;
    item.local = metadata::local_load(db, &id.0, false)?;
    item.local_latest = metadata::local_load(db, &id.0, true)?;
    item.remote = metadata::remote_load(db, &id.0, false)?;
    item.remote_latest = metadata::remote_load(db, &id.0, true)?;
    item.baseline = super::baseline::load(db, id)?;
    Ok(item)
}
pub(super) fn revision(item: &SyncItem, expected: u64) -> InventoryResult<()> {
    if item.revision != expected {
        return Err(failure(InventoryError::VersionMismatch));
    }
    Ok(())
}
pub(super) fn bump(db: &Connection, id: &SyncItemId) -> InventoryResult<()> {
    let changed = db
        .execute(
            "UPDATE items SET revision=revision+1 WHERE id=?1 AND revision<9223372036854775807",
            [&id.0],
        )
        .map_err(sql)?;
    if changed != 1 {
        return Err(failure(InventoryError::VersionMismatch));
    }
    Ok(())
}
fn create(db: &Connection) -> InventoryResult<SyncItemId> {
    let mut random = [0u8; 24];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut random))
        .map_err(|_| failure(InventoryError::StorageUnavailable))?;
    let id = SyncItemId(format!("{:x}", Sha256::digest(random)));
    db.execute("INSERT INTO items(id,kind,local_presence,remote_presence,status,revision) VALUES(?1,'Other','Unknown','Unknown','PendingReconciliation',1)",[&id.0]).map_err(sql)?;
    Ok(id)
}
fn same_target(
    db: &Connection,
    target: &ObservationTarget,
    id: &SyncItemId,
) -> InventoryResult<bool> {
    Ok(match target {
        ObservationTarget::NewItem => true,
        ObservationTarget::Existing(expected) => expected == id,
        ObservationTarget::ByRemoteRef(reference) => {
            resolve(db, &ItemSelector::RemoteRef(reference.clone()))?.as_ref() == Some(id)
        }
        ObservationTarget::ByLocalId(local) => {
            resolve(db, &ItemSelector::LocalId(local.clone()))?.as_ref() == Some(id)
        }
    })
}
#[allow(
    clippy::too_many_arguments,
    reason = "One private transaction receives the corresponding typed public observation request"
)]
fn record(
    db: &Connection,
    target: &ObservationTarget,
    file: &File,
    presence: &PresenceState,
    proof: &ObservationProof,
    expected: Option<u64>,
    remote: bool,
) -> InventoryResult<RecordObservationResult> {
    file.validate()
        .map_err(|_| failure(InventoryError::InvalidArgument))?;
    let info = sync_info(db)?;
    if file.sync_id != info.sync_id {
        return Err(failure(InventoryError::InvalidIdentity));
    }
    if remote != matches!(file.scope, FileScope::Remote(_))
        || matches!(file.scope, FileScope::WholeSync)
    {
        return Err(failure(InventoryError::InvalidArgument));
    }
    if let FileScope::Remote(reference) = &file.scope {
        if reference.account_id != info.account_id {
            return Err(failure(InventoryError::InvalidIdentity));
        }
    }
    if remote
        != matches!(
            proof.source,
            ObservationSource::RemoteListing | ObservationSource::RemoteChange
        )
    {
        return Err(failure(InventoryError::InvalidArgument));
    }
    if *presence == PresenceState::Present && file.data.is_none() {
        return Err(failure(InventoryError::MissingObservation));
    }
    if *presence == PresenceState::AbsentConfirmed
        && (file.data.is_some()
            || !proof.complete_scope
            || proof
                .scan_or_generation
                .as_ref()
                .is_none_or(|s| s.trim().is_empty())
            || !matches!(
                proof.source,
                ObservationSource::LocalScan | ObservationSource::RemoteListing
            ))
    {
        return Err(failure(InventoryError::UnconfirmedAbsence));
    }
    let digest = Sha256::digest(
        serde_json::to_vec(&(file, presence, proof))
            .map_err(|_| failure(InventoryError::SerializationFailed))?,
    )
    .to_vec();
    let receipt: Option<(String, Vec<u8>)> = db
        .query_row(
            "SELECT item,digest FROM receipts WHERE event=?1",
            [&file.id.0],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    if let Some((id, old)) = receipt {
        let id = SyncItemId(id);
        if old != digest || !same_target(db, target, &id)? {
            return Err(failure(InventoryError::VersionMismatch));
        }
        return Ok(RecordObservationResult {
            item: load(db, &id)?,
            created: false,
        });
    }
    let existing = match target {
        ObservationTarget::Existing(id) => Some(id.clone()),
        ObservationTarget::ByRemoteRef(reference) => {
            if remote && file.scope != FileScope::Remote(reference.clone()) {
                return Err(failure(InventoryError::InvalidIdentity));
            }
            resolve(db, &ItemSelector::RemoteRef(reference.clone()))?
        }
        ObservationTarget::ByLocalId(id) => resolve(db, &ItemSelector::LocalId(id.clone()))?,
        ObservationTarget::NewItem => None,
    };
    if existing.is_none()
        && matches!(
            (remote, target),
            (true, ObservationTarget::ByLocalId(_)) | (false, ObservationTarget::ByRemoteRef(_))
        )
    {
        return Err(failure(InventoryError::NotFound));
    }
    let created = existing.is_none();
    if created && (expected.is_some() || file.data.is_none() || *presence != PresenceState::Present)
    {
        return Err(failure(InventoryError::MissingObservation));
    }
    let id = if let Some(id) = existing {
        let item = load(db, &id)?;
        revision(
            &item,
            expected.ok_or_else(|| failure(InventoryError::VersionMismatch))?,
        )?;
        id
    } else {
        create(db)?
    };
    let current = load(db, &id)?;
    match &file.data {
        Some(FileData::Local(local)) if !remote => {
            if matches!(target,ObservationTarget::ByLocalId(value)if local.local_id.as_ref()!=Some(value))
            {
                return Err(failure(InventoryError::InvalidIdentity));
            }
            if let (Some(old), Some(new)) = (&current.local_file_id, &local.local_id) {
                if old != new {
                    return Err(failure(InventoryError::DuplicateAssociation));
                }
            }
            db.execute(
                "UPDATE items SET local_id=COALESCE(?2,local_id) WHERE id=?1",
                params![id.0, local.local_id.as_ref().map(|v| &v.0)],
            )
            .map_err(sql)?;
            metadata::local_apply(db, &id.0, local)?;
            if let MetadataValue::Known(kind) = local.kind {
                let kind = match kind {
                    LocalEntryKind::Regular => ItemKind::File,
                    LocalEntryKind::Directory => ItemKind::Directory,
                    LocalEntryKind::Symlink => ItemKind::Symlink,
                    _ => ItemKind::Other,
                };
                db.execute(
                    "UPDATE items SET kind=?2 WHERE id=?1",
                    params![id.0, enum_text(&kind)?],
                )
                .map_err(sql)?;
            }
        }
        Some(FileData::Remote(value)) if remote => {
            if current
                .remote
                .as_ref()
                .is_some_and(|old| old.provider != value.provider)
            {
                return Err(failure(InventoryError::InvalidIdentity));
            }
            if let Some(old) = &current.remote_ref {
                if old != &value.reference {
                    return Err(failure(InventoryError::DuplicateAssociation));
                }
            }
            db.execute(
                "UPDATE items SET remote_account=?2,remote_id=?3 WHERE id=?1",
                params![
                    id.0,
                    value.reference.account_id.0,
                    value.reference.file_id.0
                ],
            )
            .map_err(sql)?;
            metadata::remote_apply(db, &id.0, value)?;
            if let MetadataValue::Known(kind) = value.kind {
                let kind = match kind {
                    RemoteEntryKind::Directory => ItemKind::Directory,
                    RemoteEntryKind::File => ItemKind::File,
                    _ => ItemKind::Other,
                };
                db.execute(
                    "UPDATE items SET kind=?2 WHERE id=?1",
                    params![id.0, enum_text(&kind)?],
                )
                .map_err(sql)?;
            }
        }
        None => {
            let associated = match &file.scope {
                FileScope::Remote(reference) => current.remote_ref.as_ref() == Some(reference),
                FileScope::Local(path) => current
                    .local
                    .as_ref()
                    .is_some_and(|local| &local.relative_path == path),
                _ => false,
            };
            if !associated {
                return Err(failure(InventoryError::MissingObservation));
            }
        }
        _ => return Err(failure(InventoryError::InvalidArgument)),
    }
    db.execute(
        if remote {
            "UPDATE items SET remote_presence=?2,last_checked=?3 WHERE id=?1"
        } else {
            "UPDATE items SET local_presence=?2,last_checked=?3 WHERE id=?1"
        },
        params![id.0, enum_text(presence)?, proof.observed_at.unix_millis],
    )
    .map_err(sql)?;
    if !created {
        bump(db, &id)?;
    }
    db.execute(
        "INSERT INTO receipts VALUES(?1,?2,?3,?4,?5)",
        params![
            file.id.0,
            id.0,
            i32::from(remote),
            digest,
            serde_json::to_string(proof)
                .map_err(|_| failure(InventoryError::SerializationFailed))?
        ],
    )
    .map_err(sql)?;
    Ok(RecordObservationResult {
        item: load(db, &id)?,
        created,
    })
}
impl InventoryManager {
    /// Resolves a single indexed logical, local or qualified remote identity.
    pub fn get_item(&self, request: GetItemRequest) -> InventoryResult<GetItemResult> {
        self.run(false, |db| {
            Ok(GetItemResult {
                item: resolve(db, &request.selector)?
                    .map(|id| load(db, &id))
                    .transpose()?,
            })
        })
    }
    /// Lists a bounded keyset page; new insertions do not move the original high-water mark.
    pub fn list_items(&self, request: ListItemsRequest) -> InventoryResult<ListItemsResult> {
        self.run(false,|db|{
        let filter=&request.filter;
        let (after,through,signature)=pagination::bounds(db,"items",&( "items",filter),&request.pagination)?;
        let mut query="SELECT i.seq,i.id FROM items i WHERE i.seq>? AND i.seq<=?".to_owned();let mut values=vec![Value::Integer(after),Value::Integer(through)];
        if !filter.statuses.is_empty(){query.push_str(" AND i.status IN (");query.push_str(&vec!["?";filter.statuses.len()].join(","));query.push(')');for status in &filter.statuses{values.push(Value::Text(enum_text(status)?));}}
        for (column,presence)in[("local_presence",&filter.local_presence),("remote_presence",&filter.remote_presence)]{if let Some(presence)=presence{query.push_str(&format!(" AND i.{column}=?"));values.push(Value::Text(enum_text(presence)?));}}
        if let Some(parent)=&filter.remote_parent{if parent.account_id!=sync_info(db)?.account_id{return Err(failure(InventoryError::InvalidIdentity));}query.push_str(" AND EXISTS(SELECT 1 FROM remote_parents p WHERE p.item=i.id AND p.account=? AND p.parent=?)");values.extend([Value::Text(parent.account_id.0.clone()),Value::Text(parent.file_id.0.clone())]);}
        if let Some(parent)=&filter.local_parent_item_id{query.push_str(" AND EXISTS(SELECT 1 FROM local_files c JOIN local_files p ON c.parent_path=p.path WHERE c.item=i.id AND p.item=?)");values.push(Value::Text(parent.0.clone()));}
        query.push_str(" ORDER BY i.seq LIMIT ?");values.push(Value::Integer(i64::from(request.pagination.limit)+1));
        let mut stmt=db.prepare(&query).map_err(sql)?;let rows=stmt.query_map(rusqlite::params_from_iter(values),|r|Ok((r.get::<_,i64>(0)?,SyncItemId(r.get(1)?)))).map_err(sql)?;
        let mut ids=rows.collect::<Result<Vec<_>,_>>().map_err(sql)?;let more=ids.len()>request.pagination.limit as usize;if more{ids.pop();}
        let next_cursor=if more{Some(pagination::next(db,signature,ids.last().unwrap().0,through)?)}else{None};
        let entries=ids.into_iter().map(|(_,id)|load(db,&id)).collect::<InventoryResult<Vec<_>>>()?;Ok(ListItemsResult{page:Page{entries,next_cursor}})
    })
    }
    /// Records local evidence without updating baseline or choosing an upload.
    pub fn record_local(
        &self,
        request: RecordLocalObservation,
    ) -> InventoryResult<RecordObservationResult> {
        self.run(true, |db| {
            record(
                db,
                &request.selector,
                &request.observation.observed,
                &request.observation.presence,
                &request.observation.proof,
                request.expected_revision,
                false,
            )
        })
    }
    /// Records remote evidence with account-qualified identity, never choosing a download.
    pub fn record_remote(
        &self,
        request: RecordRemoteObservation,
    ) -> InventoryResult<RecordObservationResult> {
        self.run(true, |db| {
            record(
                db,
                &request.selector,
                &request.observation.observed,
                &request.observation.presence,
                &request.observation.proof,
                request.expected_revision,
                true,
            )
        })
    }
    /// Applies explicit association patches; changing a resource clears only that side's observations.
    pub fn link_item(&self, request: LinkItemRequest) -> InventoryResult<LinkItemResult> {
        self.run(true,|db|{
        let item=load(db,&request.item_id)?;revision(&item,request.expected_revision)?;
        match &request.changes.local_file_id {
            FieldUpdate::Keep=>{},
            change=>{let value=match change{FieldUpdate::Set(value)if !value.0.trim().is_empty()=>Some(&value.0),FieldUpdate::Clear=>None,_=>return Err(failure(InventoryError::InvalidArgument))};
                db.execute("UPDATE items SET local_id=?2 WHERE id=?1",params![item.id.0,value]).map_err(sql)?;}
        }
        match &request.changes.remote_ref {
            FieldUpdate::Keep=>{},
            change=>{let value=match change{FieldUpdate::Set(value)if value.account_id==sync_info(db)?.account_id && !value.file_id.0.trim().is_empty()=>Some(value),FieldUpdate::Clear=>None,_=>return Err(failure(InventoryError::InvalidIdentity))};
                if value!=item.remote_ref.as_ref(){
                    db.execute("DELETE FROM remote_parents WHERE item=?1",[&item.id.0]).map_err(sql)?;
                    db.execute("DELETE FROM metadata_fields WHERE item=?1 AND side=1",[&item.id.0]).map_err(sql)?;
                    db.execute("DELETE FROM remote_files WHERE item=?1",[&item.id.0]).map_err(sql)?;
                }
                db.execute("UPDATE items SET remote_account=?2,remote_id=?3,remote_presence='Unknown' WHERE id=?1",params![item.id.0,value.map(|v|&v.account_id.0),value.map(|v|&v.file_id.0)]).map_err(sql)?;}
        }
        bump(db,&item.id)?;Ok(LinkItemResult{item:load(db,&item.id)?})
    })
    }
    /// Persists the analyzer's explicit diagnostic; synchronized state requires confirm_sync.
    pub fn update_item_status(
        &self,
        request: UpdateItemStatusRequest,
    ) -> InventoryResult<UpdateItemStatusResult> {
        self.run(true, |db| {
            let item = load(db, &request.item_id)?;
            revision(&item, request.expected_revision)?;
            if request.change.status == SyncStatus::Synced {
                return Err(failure(InventoryError::InvalidTransition));
            }
            let reason = match &request.change.conflict_reason {
                FieldUpdate::Keep => item.conflict_reason,
                FieldUpdate::Set(value) => Some(value.clone()),
                FieldUpdate::Clear => None,
            };
            if reason.as_ref().is_some_and(|v| v.len() > 4096) {
                return Err(failure(InventoryError::InvalidArgument));
            }
            db.execute(
                "UPDATE items SET status=?2,conflict_reason=?3 WHERE id=?1",
                params![item.id.0, enum_text(&request.change.status)?, reason],
            )
            .map_err(sql)?;
            bump(db, &item.id)?;
            Ok(UpdateItemStatusResult {
                item: load(db, &item.id)?,
            })
        })
    }
}
