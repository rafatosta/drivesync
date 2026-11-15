//! Durable work intent, atomic reservations and explicit recovery without executing effects.
use super::{
    items,
    pagination::{self, enum_text, enum_value},
    *,
};
use rusqlite::{params, types::Value, OptionalExtension};
use sha2::{Digest, Sha256};

pub(super) fn load(db: &Connection, id: &OperationId) -> InventoryResult<SyncOperation> {
    let row = db.query_row(
        "SELECT item,direction,kind,state,base_revision,expected_version,attempts,next_attempt,last_error,created,started,completed FROM operations WHERE seq=?1",
        [id.0], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,u64>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,u32>(6)?,r.get::<_,Option<i64>>(7)?,r.get::<_,Option<String>>(8)?,r.get::<_,i64>(9)?,r.get::<_,Option<i64>>(10)?,r.get::<_,Option<i64>>(11)?)))
        .optional().map_err(sql)?.ok_or_else(||failure(InventoryError::NotFound))?;
    Ok(SyncOperation {
        id: id.clone(),
        item_id: SyncItemId(row.0),
        direction: enum_value(row.1)?,
        kind: enum_value(row.2)?,
        state: enum_value(row.3)?,
        base_item_revision: row.4,
        expected_remote_version: row.5,
        attempt_count: row.6,
        next_attempt_at: row.7.map(time),
        last_error_kind: row.8,
        created_at: time(row.9),
        started_at: row.10.map(time),
        completed_at: row.11.map(time),
    })
}
fn time(unix_millis: i64) -> Timestamp {
    Timestamp { unix_millis }
}
pub(super) fn error_code(value: &str) -> InventoryResult<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b"_.-".contains(&v))
    {
        return Err(failure(InventoryError::InvalidArgument));
    }
    Ok(())
}
fn check_version(item: &SyncItem, operation: &SyncOperation) -> InventoryResult<()> {
    items::revision(item, operation.base_item_revision)?;
    if let Some(expected) = &operation.expected_remote_version {
        if !item
            .remote_latest
            .as_ref()
            .is_some_and(|r| matches!(&r.version,MetadataValue::Known(value) if value==expected))
        {
            return Err(failure(InventoryError::VersionMismatch));
        }
    }
    Ok(())
}
impl InventoryManager {
    /// Persists explicit work intent. A key is scoped to this item and must describe identical intent.
    /// Enqueue never changes an item's revision or starts a transfer.
    pub fn enqueue_operation(
        &self,
        request: EnqueueOperationRequest,
    ) -> InventoryResult<EnqueueOperationResult> {
        self.run(true,|db|{
            let spec=&request.spec;
            if spec.deduplication_key.as_ref().is_some_and(|v|v.trim().is_empty() || v.len()>256)
                || spec.expected_remote_version.as_ref().is_some_and(|v|v.is_empty() || v.len()>4096) {
                return Err(failure(InventoryError::InvalidArgument));
            }
            let fingerprint=Sha256::digest(serde_json::to_vec(&request).map_err(|_|failure(InventoryError::SerializationFailed))?).to_vec();
            if let Some(key)=&spec.deduplication_key {
                let existing:Option<(i64,Vec<u8>)>=db.query_row("SELECT seq,fingerprint FROM operations WHERE item=?1 AND dedup=?2",params![request.item_id.0,key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql)?;
                if let Some((id,old))=existing {
                    if old!=fingerprint{return Err(failure(InventoryError::VersionMismatch));}
                    return Ok(EnqueueOperationResult{operation:load(db,&OperationId(id))?});
                }
            }
            let item=items::load(db,&request.item_id)?;
            items::revision(&item,request.expected_item_revision)?;
            // Resource existence is a precondition; choosing this action belongs to the analyzer.
            if (spec.direction==OperationDirection::Download && item.remote_ref.is_none())
                || (spec.direction==OperationDirection::Upload && item.local.is_none()) {
                return Err(failure(InventoryError::MissingObservation));
            }
            db.execute("INSERT INTO operations(item,direction,kind,state,base_revision,expected_version,created,dedup,fingerprint) VALUES(?1,?2,?3,'Pending',?4,?5,?6,?7,?8)",params![item.id.0,enum_text(&spec.direction)?,enum_text(&spec.kind)?,item.revision,spec.expected_remote_version,spec.created_at.unix_millis,spec.deduplication_key,fingerprint]).map_err(sql)?;
            Ok(EnqueueOperationResult{operation:load(db,&OperationId(db.last_insert_rowid()))?})
        })
    }
    /// Lists persisted work in insertion order; eligible_before includes only runnable states.
    pub fn list_operations(
        &self,
        request: ListOperationsRequest,
    ) -> InventoryResult<ListOperationsResult> {
        self.run(false,|db|{
            let f=&request.filter;
            let (after,through,signature)=pagination::bounds(db,"operations",&("operations",f),&request.pagination)?;
            let mut query="SELECT seq FROM operations WHERE seq>? AND seq<=?".to_owned();
            let mut values=vec![Value::Integer(after),Value::Integer(through)];
            if let Some(item)=&f.item_id{query.push_str(" AND item=?");values.push(Value::Text(item.0.clone()));}
            if let Some(direction)=&f.direction{query.push_str(" AND direction=?");values.push(Value::Text(enum_text(direction)?));}
            if !f.states.is_empty(){query.push_str(&format!(" AND state IN ({})",vec!["?";f.states.len()].join(",")));for s in &f.states{values.push(Value::Text(enum_text(s)?));}}
            if let Some(time)=&f.eligible_before{query.push_str(" AND state IN ('Pending','RetryWait') AND (next_attempt IS NULL OR next_attempt<=?)");values.push(Value::Integer(time.unix_millis));}
            query.push_str(" ORDER BY seq LIMIT ?");values.push(Value::Integer(i64::from(request.pagination.limit)+1));
            let mut stmt=db.prepare(&query).map_err(sql)?;
            let mut ids=stmt.query_map(rusqlite::params_from_iter(values),|r|r.get::<_,i64>(0)).map_err(sql)?.collect::<Result<Vec<_>,_>>().map_err(sql)?;
            let more=ids.len()>request.pagination.limit as usize;if more{ids.pop();}
            let next_cursor=if more{Some(pagination::next(db,signature,*ids.last().unwrap(),through)?)}else{None};
            let entries=ids.into_iter().map(|id|load(db,&OperationId(id))).collect::<InventoryResult<_>>()?;
            Ok(ListOperationsResult{page:Page{entries,next_cursor}})
        })
    }
    /// Atomically reserves a due operation after checking its item revision and observed remote version.
    /// A surviving Running claim stays unknown after restart; opening never releases it automatically.
    pub fn claim_operation(
        &self,
        request: ClaimOperationRequest,
    ) -> InventoryResult<ClaimOperationResult> {
        self.run(true,|db|{
            let operation=load(db,&request.operation_id)?;
            super::analysis::ensure_executable(db,&operation.id)?;
            if operation.state!=request.expected_state{return Err(failure(InventoryError::VersionMismatch));}
            if !matches!(operation.state,OperationState::Pending|OperationState::RetryWait)
                || operation.next_attempt_at.as_ref().is_some_and(|v|v.unix_millis>request.claimed_at.unix_millis)
                || request.claimed_at.unix_millis<operation.created_at.unix_millis {
                return Err(failure(InventoryError::InvalidTransition));
            }
            check_version(&items::load(db,&operation.item_id)?,&operation)?;
            let running:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM operations WHERE item=?1 AND state='Running')",[&operation.item_id.0],|r|r.get(0)).map_err(sql)?;
            if running{return Err(failure(InventoryError::InvalidTransition));}
            if operation.attempt_count==u32::MAX{return Err(failure(InventoryError::InvalidTransition));}
            db.execute("UPDATE operations SET state='Running',attempts=attempts+1,started=?2,next_attempt=NULL,last_error=NULL WHERE seq=?1",params![operation.id.0,request.claimed_at.unix_millis]).map_err(sql)?;
            Ok(ClaimOperationResult{operation:load(db,&operation.id)?})
        })
    }
    /// Applies a checked transition. Completion is reserved for confirm_sync.
    /// Explicit Requeue of an unknown Running effect is the caller's responsibility after verification.
    pub fn update_operation(
        &self,
        request: UpdateOperationRequest,
    ) -> InventoryResult<UpdateOperationResult> {
        self.run(true,|db|{
            let op=load(db,&request.operation_id)?;
            super::analysis::ensure_executable(db,&op.id)?;
            if op.state!=request.expected_state || op.attempt_count!=request.expected_attempt_count{return Err(failure(InventoryError::VersionMismatch));}
            let (state,next,error,completed)=match &request.transition {
                OperationTransition::Retry{next_attempt_at,error_kind} if op.state==OperationState::Running=>{
                    error_code(error_kind)?;
                    if op.started_at.as_ref().is_some_and(|v|next_attempt_at.unix_millis<v.unix_millis){return Err(failure(InventoryError::InvalidArgument));}
                    (OperationState::RetryWait,Some(next_attempt_at.unix_millis),error_kind,None)
                },
                OperationTransition::Block{error_kind} if op.state==OperationState::Running=>{error_code(error_kind)?;(OperationState::Blocked,None,error_kind,None)},
                OperationTransition::Fail{failed_at,error_kind} if op.state==OperationState::Running=>{
                    error_code(error_kind)?;
                    if op.started_at.as_ref().is_some_and(|v|failed_at.unix_millis<v.unix_millis){return Err(failure(InventoryError::InvalidArgument));}
                    (OperationState::Failed,None,error_kind,Some(failed_at.unix_millis))
                },
                OperationTransition::Requeue{reason} if matches!(op.state,OperationState::Blocked|OperationState::Failed|OperationState::RetryWait|OperationState::Running)=>{
                    error_code(reason)?;
                    (OperationState::Pending,None,reason,None)
                },
                _=>return Err(failure(InventoryError::InvalidTransition)),
            };
            db.execute("UPDATE operations SET state=?2,next_attempt=?3,last_error=?4,completed=?5 WHERE seq=?1",params![op.id.0,enum_text(&state)?,next,error,completed]).map_err(sql)?;
            Ok(UpdateOperationResult{operation:load(db,&op.id)?})
        })
    }
}
