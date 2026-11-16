//! Append-only item history with immutable snapshots and chronological keyset pagination.
use super::{
    items, operations,
    pagination::{self, enum_text, enum_value},
    *,
};
use rusqlite::{params, types::Value, OptionalExtension};

pub(super) fn load(db: &Connection, id: &ActivityId) -> InventoryResult<SyncActivity> {
    let r=db.query_row("SELECT item,operation,event_type,origin,outcome,occurred,description,error_kind,item_name,remote_id,local_path FROM activities WHERE seq=?1",[id.0],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<i64>>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,Option<String>>(8)?,r.get::<_,Option<String>>(9)?,r.get::<_,Option<Vec<u8>>>(10)?))).optional().map_err(sql)?.ok_or_else(||failure(InventoryError::NotFound))?;
    Ok(SyncActivity {
        id: id.clone(),
        item_id: SyncItemId(r.0),
        operation_id: r.1.map(OperationId),
        event_type: enum_value(r.2)?,
        origin: enum_value(r.3)?,
        outcome: enum_value(r.4)?,
        occurred_at: Timestamp { unix_millis: r.5 },
        description: r.6,
        error_kind: r.7,
        item_name: r.8,
        item_remote_id: r.9,
        item_local_path: r.10,
    })
}
pub(super) fn append(db: &Connection, a: &NewActivity) -> InventoryResult<SyncActivity> {
    if items::resolve(db, &ItemSelector::Id(a.item_id.clone()))?.is_none() {
        return Err(failure(InventoryError::NotFound));
    }
    if a.description.as_ref().is_some_and(|s| {
        s.len() > 4096 || s.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
    }) || a.item_name.as_ref().is_some_and(|s| s.len() > 4096)
        || a.item_remote_id
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 4096)
        || a.item_local_path
            .as_ref()
            .is_some_and(|p| p.len() > 4096 || LocalRelativePath::new(p.clone()).is_err())
    {
        return Err(failure(InventoryError::InvalidArgument));
    }
    if let Some(code) = &a.error_kind {
        operations::error_code(code)?;
    }
    if let Some(id) = &a.operation_id {
        let op = operations::load(db, id)?;
        if op.item_id != a.item_id {
            return Err(failure(InventoryError::InvalidIdentity));
        }
        let valid = match a.outcome {
            ActivityOutcome::Completed => op.state == OperationState::Completed,
            ActivityOutcome::Failed => op.state == OperationState::Failed,
            ActivityOutcome::Blocked => op.state == OperationState::Blocked,
            ActivityOutcome::Informational => true,
            ActivityOutcome::Cancelled => false,
        };
        if !valid {
            return Err(failure(InventoryError::InvalidTransition));
        }
    }
    db.execute("INSERT INTO activities(item,operation,event_type,origin,outcome,occurred,description,error_kind,item_name,remote_id,local_path) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![a.item_id.0,a.operation_id.as_ref().map(|id|id.0),enum_text(&a.event_type)?,enum_text(&a.origin)?,enum_text(&a.outcome)?,a.occurred_at.unix_millis,a.description,a.error_kind,a.item_name,a.item_remote_id,a.item_local_path]).map_err(sql)?;
    load(db, &ActivityId(db.last_insert_rowid()))
}
impl InventoryManager {
    /// Appends a historical fact supplied by the caller; does not infer a conflict or finish work.
    /// Independent calls are separate facts. Only confirm_sync retries deduplicate automatically.
    /// Descriptions must already be safe for persistence; the inventory cannot identify arbitrary secrets.
    pub fn record_activity(
        &self,
        request: RecordActivityRequest,
    ) -> InventoryResult<RecordActivityResult> {
        self.run(true, |db| {
            Ok(RecordActivityResult {
                activity: append(db, &request.activity)?,
            })
        })
    }
    /// Lists ascending event time with insertion id as tie-breaker and a fixed insertion high-water mark.
    pub fn list_activities(
        &self,
        request: ListActivitiesRequest,
    ) -> InventoryResult<ListActivitiesResult> {
        self.run(false, |db| {
            let f = &request.filter;
            if matches!((&f.from,&f.until),(Some(a),Some(b)) if a.unix_millis>b.unix_millis) {
                return Err(failure(InventoryError::InvalidArgument));
            }
            let (after, through, signature) =
                pagination::bounds(db, "activities", &("activities", f), &request.pagination)?;
            let mut query = "SELECT seq FROM activities WHERE seq<=?".to_owned();
            let mut values = vec![Value::Integer(through)];
            if after != 0 {
                let time: i64 = db
                    .query_row(
                        "SELECT occurred FROM activities WHERE seq=?1",
                        [after],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(sql)?
                    .ok_or_else(|| failure(InventoryError::InvalidCursor))?;
                query.push_str(" AND (occurred>? OR (occurred=? AND seq>?))");
                values.extend([
                    Value::Integer(time),
                    Value::Integer(time),
                    Value::Integer(after),
                ]);
            }
            if let Some(id) = &f.item_id {
                query.push_str(" AND item=?");
                values.push(Value::Text(id.0.clone()));
            }
            if let Some(id) = &f.operation_id {
                query.push_str(" AND operation=?");
                values.push(Value::Integer(id.0));
            }
            for (column, items) in [
                (
                    "event_type",
                    f.event_types
                        .iter()
                        .map(enum_text)
                        .collect::<InventoryResult<Vec<_>>>()?,
                ),
                (
                    "outcome",
                    f.outcomes
                        .iter()
                        .map(enum_text)
                        .collect::<InventoryResult<Vec<_>>>()?,
                ),
            ] {
                if !items.is_empty() {
                    query.push_str(&format!(
                        " AND {column} IN ({})",
                        vec!["?"; items.len()].join(",")
                    ));
                    values.extend(items.into_iter().map(Value::Text));
                }
            }
            if let Some(time) = &f.from {
                query.push_str(" AND occurred>=?");
                values.push(Value::Integer(time.unix_millis));
            }
            if let Some(time) = &f.until {
                query.push_str(" AND occurred<=?");
                values.push(Value::Integer(time.unix_millis));
            }
            query.push_str(" ORDER BY occurred,seq LIMIT ?");
            values.push(Value::Integer(i64::from(request.pagination.limit) + 1));
            let mut stmt = db.prepare(&query).map_err(sql)?;
            let mut ids = stmt
                .query_map(rusqlite::params_from_iter(values), |r| r.get::<_, i64>(0))
                .map_err(sql)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(sql)?;
            let more = ids.len() > request.pagination.limit as usize;
            if more {
                ids.pop();
            }
            let next_cursor = if more {
                Some(pagination::next(
                    db,
                    signature,
                    *ids.last().unwrap(),
                    through,
                )?)
            } else {
                None
            };
            let entries = ids
                .into_iter()
                .map(|id| load(db, &ActivityId(id)))
                .collect::<InventoryResult<_>>()?;
            Ok(ListActivitiesResult {
                page: Page {
                    entries,
                    next_cursor,
                },
            })
        })
    }
}
