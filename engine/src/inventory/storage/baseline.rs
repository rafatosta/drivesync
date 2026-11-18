//! Explicit verified checkpoints committed atomically with item state, work completion and history.
use super::{
    activity, items, operations,
    pagination::{enum_text, enum_value},
    *,
};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};

fn side_load(
    db: &Connection,
    id: &SyncItemId,
    side: i32,
) -> InventoryResult<Option<(BaselineSide, Timestamp)>> {
    let r=db.query_row("SELECT presence,name,local_name,parent_local,parent_account,parent_remote,version,checksum,size,modified,confirmed FROM baselines WHERE item=?1 AND side=?2",params![id.0,side],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,Option<Vec<u8>>>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,Option<String>>(8)?,r.get::<_,Option<i64>>(9)?,r.get::<_,i64>(10)?))).optional().map_err(sql)?;
    r.map(|r| {
        Ok((
            BaselineSide {
                presence: enum_value(r.0)?,
                name: r.1,
                local_name_bytes: r.2,
                parent_identity: match (r.3, r.4, r.5) {
                    (Some(id), None, None) => Some(BaselineParent::Local(SyncItemId(id))),
                    (None, Some(account), Some(file)) => {
                        Some(BaselineParent::Remote(RemoteFileRef {
                            account_id: AccountId(account),
                            file_id: RemoteFileId(file),
                        }))
                    }
                    (None, None, None) => None,
                    _ => return Err(failure(InventoryError::CorruptDatabase)),
                },
                version: r.6,
                checksum: r.7,
                size: r
                    .8
                    .map(|s| {
                        s.parse::<u64>()
                            .map_err(|_| failure(InventoryError::CorruptDatabase))
                    })
                    .transpose()?,
                modified_at: r.9.map(|unix_millis| Timestamp { unix_millis }),
            },
            Timestamp { unix_millis: r.10 },
        ))
    })
    .transpose()
}
pub(super) fn load(db: &Connection, id: &SyncItemId) -> InventoryResult<Option<SyncBaseline>> {
    match (side_load(db, id, 0)?, side_load(db, id, 1)?) {
        (None, None) => Ok(None),
        (Some((local, a)), Some((remote, b))) if a == b => Ok(Some(SyncBaseline {
            local,
            remote,
            confirmed_at: a,
        })),
        _ => Err(failure(InventoryError::CorruptDatabase)),
    }
}
fn validate_side(
    db: &Connection,
    item: &SyncItem,
    side: &BaselineSide,
    remote: bool,
) -> InventoryResult<()> {
    if side
        .name
        .as_ref()
        .is_some_and(|v| v.len() > 4096 || v.as_bytes().contains(&0))
        || side.version.as_ref().is_some_and(|v| v.len() > 4096)
        || side
            .checksum
            .as_ref()
            .is_some_and(|v| v.is_empty() || v.len() > 4096)
        || side.local_name_bytes.as_ref().is_some_and(|v| {
            v.is_empty()
                || v.len() > 4096
                || v.contains(&0)
                || v.contains(&b'/')
                || v == b"."
                || v == b".."
        })
    {
        return Err(failure(InventoryError::InvalidArgument));
    }
    if remote && side.local_name_bytes.is_some() {
        return Err(failure(InventoryError::InvalidArgument));
    }
    if let Some(parent) = &side.parent_identity {
        match (parent, remote) {
            (BaselineParent::Local(id), false) if id != &item.id => {
                if items::resolve(db, &ItemSelector::Id(id.clone()))?.is_none() {
                    return Err(failure(InventoryError::NotFound));
                }
            }
            (BaselineParent::Remote(reference), true)
                if reference.account_id == sync_info(db)?.account_id
                    && !reference.file_id.0.is_empty() => {}
            _ => return Err(failure(InventoryError::InvalidIdentity)),
        }
    }
    if side.presence == PresenceState::Unknown {
        return Err(failure(InventoryError::MissingObservation));
    }
    let observed = if remote {
        &item.remote_presence
    } else {
        &item.local_presence
    };
    if observed != &side.presence {
        return Err(failure(InventoryError::MissingObservation));
    }
    if side.presence == PresenceState::Present
        && (if remote {
            item.remote.is_none()
        } else {
            item.local.is_none()
        })
    {
        return Err(failure(InventoryError::MissingObservation));
    }
    Ok(())
}
fn store(db: &Connection, id: &SyncItemId, b: &SyncBaseline) -> InventoryResult<()> {
    for (side, data) in [(0, &b.local), (1, &b.remote)] {
        let (parent_local, parent_account, parent_remote) = match &data.parent_identity {
            Some(BaselineParent::Local(id)) => (Some(&id.0), None, None),
            Some(BaselineParent::Remote(r)) => (None, Some(&r.account_id.0), Some(&r.file_id.0)),
            None => (None, None, None),
        };
        db.execute("INSERT INTO baselines VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13) ON CONFLICT(item,side) DO UPDATE SET presence=excluded.presence,name=excluded.name,local_name=excluded.local_name,parent_local=excluded.parent_local,parent_account=excluded.parent_account,parent_remote=excluded.parent_remote,version=excluded.version,checksum=excluded.checksum,size=excluded.size,modified=excluded.modified,confirmed=excluded.confirmed",params![id.0,side,enum_text(&data.presence)?,data.name,data.local_name_bytes,parent_local,parent_account,parent_remote,data.version,data.checksum,data.size.map(|v|v.to_string()),data.modified_at.as_ref().map(|v|v.unix_millis),b.confirmed_at.unix_millis]).map_err(sql)?;
    }
    Ok(())
}
fn activity_matches(op: &SyncOperation, a: &NewActivity) -> bool {
    let expected = match op.kind {
        OperationKind::Create => ActivityType::Create,
        OperationKind::Rename => ActivityType::Rename,
        OperationKind::Move => ActivityType::Move,
        OperationKind::Trash => ActivityType::Trash,
        OperationKind::Delete => ActivityType::Delete,
        OperationKind::Restore => ActivityType::Restore,
        OperationKind::UpdateContent => match op.direction {
            OperationDirection::Upload => ActivityType::Upload,
            OperationDirection::Download => ActivityType::Download,
        },
    };
    a.event_type == expected
        || (op.kind == OperationKind::Create
            && a.event_type
                == match op.direction {
                    OperationDirection::Upload => ActivityType::Upload,
                    OperationDirection::Download => ActivityType::Download,
                })
}
impl InventoryManager {
    /// Commits caller-verified facts, never a detector hint or an unverified transfer response.
    /// The caller must verify physical effects before calling; this method performs no filesystem/network I/O.
    /// Revision, claim attempt, baseline, item, completed work, history and retry receipt share one transaction.
    pub fn confirm_sync(&self, request: ConfirmSyncRequest) -> InventoryResult<ConfirmSyncResult> {
        self.run(true,|db|{
            let digest=Sha256::digest(serde_json::to_vec(&request).map_err(|_|failure(InventoryError::SerializationFailed))?).to_vec();
            let receipt:Option<(Vec<u8>,Option<i64>,Option<i64>)>=db.query_row("SELECT digest,operation,activity FROM confirmations WHERE item=?1 AND revision=?2",params![request.item_id.0,request.expected_item_revision],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(sql)?;
            if let Some((old,op,activity))=receipt{
                if old!=digest{return Err(failure(InventoryError::VersionMismatch));}
                return Ok(ConfirmSyncResult{item:items::load(db,&request.item_id)?,completed_operation:op.map(|id|operations::load(db,&OperationId(id))).transpose()?,activity:activity.map(|id|activity::load(db,&ActivityId(id))).transpose()?});
            }
            let item=items::load(db,&request.item_id)?;items::revision(&item,request.expected_item_revision)?;
            let b=&request.confirmed.baseline;
            if request.confirmed.status!=SyncStatus::Synced || b.confirmed_at!=request.confirmed.verified_at
                || b.local.presence!=b.remote.presence {
                return Err(failure(InventoryError::InvalidArgument));
            }
            validate_side(db,&item,&b.local,false)?;validate_side(db,&item,&b.remote,true)?;
            if b.local.presence==PresenceState::Present &&
                (matches!((b.local.size,b.remote.size),(Some(a),Some(b))if a!=b)
                || matches!((&b.local.checksum,&b.remote.checksum),(Some(a),Some(b))if a!=b)) {
                return Err(failure(InventoryError::InvalidArgument));
            }
            if item.last_synced_at.as_ref().is_some_and(|v|v.unix_millis>request.confirmed.verified_at.unix_millis){return Err(failure(InventoryError::InvalidArgument));}
            let completed=if let Some(id)=&request.completed_operation{
                let op=operations::load(db,id)?;
                if op.item_id!=item.id{return Err(failure(InventoryError::InvalidIdentity));}
                if op.state!=OperationState::Running || Some(op.attempt_count)!=request.expected_operation_attempt{return Err(failure(InventoryError::VersionMismatch));}
                if op.started_at.as_ref().is_some_and(|v|v.unix_millis>request.confirmed.verified_at.unix_millis){return Err(failure(InventoryError::InvalidArgument));}
                let a=request.activity.as_ref().ok_or_else(||failure(InventoryError::MissingActivity))?;
                if a.item_id!=item.id || a.operation_id.as_ref()!=Some(id) || a.outcome!=ActivityOutcome::Completed || !activity_matches(&op,a) || a.occurred_at!=request.confirmed.verified_at{return Err(failure(InventoryError::InvalidArgument));}
                db.execute("UPDATE operations SET state='Completed',completed=?2,next_attempt=NULL,last_error=NULL WHERE seq=?1",params![id.0,request.confirmed.verified_at.unix_millis]).map_err(sql)?;
                Some(operations::load(db,id)?)
            }else{
                if request.expected_operation_attempt.is_some() || request.activity.as_ref().is_some_and(|a|a.operation_id.is_some()){return Err(failure(InventoryError::InvalidArgument));}
                None
            };
            // Append deliberately follows work completion; any validation/SQL failure rolls everything back.
            store(db,&item.id,b)?;
            items::save_causal_marker(db, &item.id, false, request.confirmed.local_causal_order, None)?;
            let remote_version = b.remote.version.as_deref();
            items::save_causal_marker(db, &item.id, true, request.confirmed.remote_causal_order, remote_version)?;
            db.execute("UPDATE items SET status='Synced',conflict_reason=NULL,last_synced=?2 WHERE id=?1",params![item.id.0,request.confirmed.verified_at.unix_millis]).map_err(sql)?;
            items::bump(db,&item.id)?;
            if request.activity.as_ref().is_some_and(|a|a.item_id!=item.id){return Err(failure(InventoryError::InvalidIdentity));}
            let activity=request.activity.as_ref().map(|a|activity::append(db,a)).transpose()?;
            db.execute("INSERT INTO confirmations VALUES(?1,?2,?3,?4,?5)",params![item.id.0,request.expected_item_revision,digest,completed.as_ref().map(|op|op.id.0),activity.as_ref().map(|a|a.id.0)]).map_err(sql)?;
            Ok(ConfirmSyncResult{item:items::load(db,&item.id)?,completed_operation:completed,activity})
        })
    }
}
