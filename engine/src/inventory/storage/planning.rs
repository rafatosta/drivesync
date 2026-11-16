//! Transactional plan facts and explicit user association; never executes commands.
use super::*;
use super::{
    items,
    pagination::{enum_text, enum_value},
};
use crate::inventory::planning::*;
use rusqlite::{params, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
fn enc<T: Serialize>(v: &T) -> InventoryResult<String> {
    serde_json::to_string(v).map_err(|_| failure(InventoryError::SerializationFailed))
}
fn dec<T: DeserializeOwned>(v: String) -> InventoryResult<T> {
    serde_json::from_str(&v).map_err(|_| failure(InventoryError::CorruptDatabase))
}
fn digest(item: &SyncItem) -> InventoryResult<Vec<u8>> {
    Ok(Sha256::digest(enc(item)?).to_vec())
}
pub(super) fn plan_current(db: &Connection, plan: Option<&AnalysisPlan>) -> InventoryResult<bool> {
    if let Some(plan) = plan {
        for guard in &plan.namespaces {
            if namespace(db, &guard.directory)? != *guard {
                return Ok(false);
            }
        }
        for guard in &plan.guards {
            if ItemGuard::for_item(&items::load(db, &guard.item_id)?)? != *guard {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
pub(super) fn validate_plan(db: &Connection, plan: Option<&AnalysisPlan>) -> InventoryResult<()> {
    let Some(p) = plan else { return Ok(()) };
    if p.steps.len() > 32
        || p.guards.len() > 4096
        || p.manual.len() > 64
        || !plan_current(db, Some(p))?
    {
        return Err(failure(InventoryError::VersionMismatch));
    }
    let account = sync_info(db)?.account_id;
    for (position, step) in p.steps.iter().enumerate() {
        if let Some(c) = &step.command {
            command_scope(c, &account)?;
            use crate::operations::OperationCommand as C;
            let matches = matches!(
                (&step.method, c),
                (PlannedMethod::CreateFile, C::CreateFile { .. })
                    | (PlannedMethod::CreateDirectory, C::CreateDirectory { .. })
                    | (PlannedMethod::Upload, C::Upload { .. })
                    | (PlannedMethod::Download, C::Download { .. })
                    | (PlannedMethod::UpdateContent, C::UpdateContent { .. })
                    | (PlannedMethod::Rename, C::Rename { .. })
                    | (PlannedMethod::Move, C::Move { .. })
                    | (PlannedMethod::Trash, C::Trash { .. })
                    | (PlannedMethod::Restore, C::Restore { .. })
            );
            if !matches {
                return Err(failure(InventoryError::InvalidArgument));
            }
        }
        if step
            .depends_on_steps
            .iter()
            .any(|i| *i as usize >= position)
        {
            return Err(failure(InventoryError::InvalidArgument));
        }
        if step.command.is_none() && step.blocked_by.is_empty() {
            return Err(failure(InventoryError::InvalidArgument));
        }
    }
    Ok(())
}
pub(super) fn save_plan(
    db: &Connection,
    id: i64,
    plan: Option<&AnalysisPlan>,
) -> InventoryResult<()> {
    let Some(p) = plan else { return Ok(()) };
    db.execute("INSERT INTO analysis_plans VALUES(?1)", [id])
        .map_err(sql)?;
    if let Some(r) = &p.restore_request {
        db.execute(
            "INSERT INTO analysis_plan_requests VALUES(?1,?2)",
            params![id, enc(r)?],
        )
        .map_err(sql)?;
    }
    for (i, g) in p.namespaces.iter().enumerate() {
        db.execute(
            "INSERT INTO analysis_plan_namespaces VALUES(?1,?2,?3,?4)",
            params![id, i as i64, enc(&g.directory)?, g.fingerprint],
        )
        .map_err(sql)?;
    }
    for (i, g) in p.guards.iter().enumerate() {
        db.execute(
            "INSERT INTO analysis_plan_guards VALUES(?1,?2,?3,?4)",
            params![id, i as i64, g.item_id.0, g.fingerprint],
        )
        .map_err(sql)?;
    }
    for (i, s) in p.steps.iter().enumerate() {
        db.execute(
            "INSERT INTO analysis_plan_steps VALUES(?1,?2,?3,?4)",
            params![
                id,
                i as i64,
                enum_text(&s.method)?,
                s.command.as_ref().map(enc).transpose()?
            ],
        )
        .map_err(sql)?;
        for prior in &s.depends_on_steps {
            db.execute(
                "INSERT INTO analysis_plan_order VALUES(?1,?2,?3)",
                params![id, i as i64, i64::from(*prior)],
            )
            .map_err(sql)?;
        }
        for (j, b) in s.blocked_by.iter().enumerate() {
            db.execute(
                "INSERT INTO analysis_plan_blocks VALUES(?1,?2,?3,?4)",
                params![id, i as i64, j as i64, enum_text(b)?],
            )
            .map_err(sql)?;
        }
        for (j, d) in s.depends_on.iter().enumerate() {
            db.execute(
                "INSERT INTO analysis_plan_dependencies VALUES(?1,?2,?3,?4)",
                params![id, i as i64, j as i64, d.0],
            )
            .map_err(sql)?;
        }
    }
    for (i, m) in p.manual.iter().enumerate() {
        db.execute(
            "INSERT INTO analysis_plan_manual VALUES(?1,?2,?3)",
            params![id, i as i64, enum_text(&m.reason)?],
        )
        .map_err(sql)?;
        for (j, r) in m.resources.iter().enumerate() {
            db.execute(
                "INSERT INTO analysis_plan_resources VALUES(?1,?2,?3,?4)",
                params![id, i as i64, j as i64, enc(r)?],
            )
            .map_err(sql)?;
        }
    }
    Ok(())
}
fn texts(db: &Connection, query: &str, id: i64, step: i64) -> InventoryResult<Vec<String>> {
    let mut stmt = db.prepare(query).map_err(sql)?;
    let result = stmt
        .query_map(params![id, step], |r| r.get(0))
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql);
    result
}
pub(super) fn load_plan(db: &Connection, id: i64) -> InventoryResult<Option<AnalysisPlan>> {
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM analysis_plans WHERE analysis=?1)",
            [id],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if !exists {
        return Ok(None);
    }
    let mut p = AnalysisPlan {
        restore_request: db
            .query_row(
                "SELECT request FROM analysis_plan_requests WHERE analysis=?1",
                [id],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(sql)?
            .map(dec)
            .transpose()?,
        ..Default::default()
    };
    let mut namespaces=db.prepare("SELECT directory,fingerprint FROM analysis_plan_namespaces WHERE analysis=?1 ORDER BY position").map_err(sql)?;
    for row in namespaces
        .query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
        })
        .map_err(sql)?
    {
        let (d, f) = row.map_err(sql)?;
        p.namespaces.push(NamespaceGuard {
            directory: dec(d)?,
            fingerprint: f,
        });
    }

    let mut stmt = db
        .prepare(
            "SELECT item,fingerprint FROM analysis_plan_guards WHERE analysis=?1 ORDER BY position",
        )
        .map_err(sql)?;
    p.guards = stmt
        .query_map([id], |r| {
            Ok(ItemGuard {
                item_id: SyncItemId(r.get(0)?),
                fingerprint: r.get(1)?,
            })
        })
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)?;
    let mut stmt=db.prepare("SELECT position,method,command FROM analysis_plan_steps WHERE analysis=?1 ORDER BY position").map_err(sql)?;
    for row in stmt
        .query_map([id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(sql)?
    {
        let (i, m, c) = row.map_err(sql)?;
        p.steps.push(PlannedStep{depends_on_steps: {
            let mut stmt=db.prepare("SELECT prior FROM analysis_plan_order WHERE analysis=?1 AND step=?2 ORDER BY prior").map_err(sql)?;
            let v=stmt.query_map(params![id,i],|r|r.get::<_,u32>(0)).map_err(sql)?.collect::<Result<Vec<_>,_>>().map_err(sql)?; v
        },method:enum_value(m)?,command:c.map(dec).transpose()?,blocked_by:texts(db,"SELECT kind FROM analysis_plan_blocks WHERE analysis=?1 AND step=?2 ORDER BY position",id,i)?.into_iter().map(enum_value).collect::<InventoryResult<_>>()?,depends_on:texts(db,"SELECT item FROM analysis_plan_dependencies WHERE analysis=?1 AND step=?2 ORDER BY position",id,i)?.into_iter().map(SyncItemId).collect()});
    }
    let mut stmt = db
        .prepare(
            "SELECT position,reason FROM analysis_plan_manual WHERE analysis=?1 ORDER BY position",
        )
        .map_err(sql)?;
    for row in stmt
        .query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map_err(sql)?
    {
        let (i, r) = row.map_err(sql)?;
        p.manual.push(ManualIssue{reason:enum_value(r)?,resources:texts(db,"SELECT resource FROM analysis_plan_resources WHERE analysis=?1 AND issue=?2 ORDER BY position",id,i)?.into_iter().map(dec).collect::<InventoryResult<_>>()?});
    }
    Ok(Some(p))
}
fn association(db: &Connection, id: &SyncItemId) -> InventoryResult<Option<ItemAssociation>> {
    db.query_row("SELECT canonical,retired,decision,decided FROM item_associations WHERE canonical=?1 OR retired=?1",[&id.0],|r|Ok(ItemAssociation{canonical_item:SyncItemId(r.get(0)?),retired_item:SyncItemId(r.get(1)?),decision_id:r.get(2)?,decided_at:Timestamp{unix_millis:r.get(3)?}})).optional().map_err(sql)
}
impl InventoryManager {
    /// Atomically associates explicitly selected, previously unbound present items.
    /// Local identity remains canonical; the remote historical item is retired,
    /// not deleted. Does not compare names/hashes or confirm a baseline. Exact
    /// decision retries reuse provenance; stale or conflicting choices fail closed.
    pub fn associate_items(
        &self,
        r: AssociateItemsRequest,
    ) -> InventoryResult<AssociateItemsResult> {
        self.run(true,|db|{
        if r.decision_id.trim().is_empty(){return Err(failure(InventoryError::InvalidArgument))}
        if let Some(a)=association(db,&r.expected_local.id)? {
            let stored:(Vec<u8>,Vec<u8>)=db.query_row("SELECT local_digest,remote_digest FROM item_associations WHERE decision=?1",[&r.decision_id],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(sql)?.ok_or_else(||failure(InventoryError::DuplicateAssociation))?;
            if a.decision_id!=r.decision_id || a.retired_item!=r.expected_remote.id || stored!=(digest(&r.expected_local)?,digest(&r.expected_remote)?){return Err(failure(InventoryError::VersionMismatch))}
            return Ok(AssociateItemsResult{item:items::load(db,&a.canonical_item)?,association:a})
        }
        let info=sync_info(db)?;
        let account=info.account_id;
        let l=items::load(db,&r.expected_local.id)?;let remote=items::load(db,&r.expected_remote.id)?;
        if l!=r.expected_local || remote!=r.expected_remote{return Err(failure(InventoryError::VersionMismatch))}
        if l.id==remote.id || l.kind!=remote.kind || l.baseline.is_some() || remote.baseline.is_some() || l.local_presence!=PresenceState::Present || remote.remote_presence!=PresenceState::Present || l.remote_ref.is_some() || remote.local.is_some() || remote.local_latest.is_some() || remote.local_file_id.is_some() || remote.remote_ref.as_ref().is_none_or(|v| v.account_id!=account || v.file_id.0==info.remote_root_id){return Err(failure(InventoryError::InvalidIdentity))}
        let running:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM operations WHERE item IN (?1,?2) AND state='Running')",params![l.id.0,remote.id.0],|row|row.get(0)).map_err(sql)?;
        if running{return Err(failure(InventoryError::InvalidTransition))}
        db.execute("UPDATE items SET remote_account=NULL,remote_id=NULL,remote_presence='Unknown',status='Error',conflict_reason='ExplicitAssociationRetired' WHERE id=?1",[&remote.id.0]).map_err(sql)?;
        let reference=remote.remote_ref.as_ref().ok_or_else(||failure(InventoryError::InvalidIdentity))?;
        db.execute("UPDATE items SET remote_account=?2,remote_id=?3,remote_presence=?4,status='PendingReconciliation',conflict_reason=NULL WHERE id=?1",params![l.id.0,reference.account_id.0,reference.file_id.0,enum_text(&remote.remote_presence)?]).map_err(sql)?;
        for table in ["remote_files","remote_parents"]{db.execute(&format!("UPDATE {table} SET item=?1 WHERE item=?2"),params![l.id.0,remote.id.0]).map_err(sql)?;}
        db.execute("UPDATE metadata_fields SET item=?1 WHERE item=?2 AND side=1",params![l.id.0,remote.id.0]).map_err(sql)?;
        db.execute("UPDATE receipts SET item=?1 WHERE item=?2 AND side=1",params![l.id.0,remote.id.0]).map_err(sql)?;
        items::bump(db,&l.id)?;items::bump(db,&remote.id)?;
        db.execute("INSERT INTO item_associations VALUES(?1,?2,?3,?4,?5,?6)",params![l.id.0,remote.id.0,r.decision_id,digest(&l)?,digest(&remote)?,r.decided_at.unix_millis]).map_err(sql)?;
        Ok(AssociateItemsResult{item:items::load(db,&l.id)?,association:association(db,&l.id)?.ok_or_else(||failure(InventoryError::TransactionFailed))?})
    })
    }
    pub fn get_association(
        &self,
        r: GetAssociationRequest,
    ) -> InventoryResult<GetAssociationResult> {
        self.run(false, |db| {
            Ok(GetAssociationResult {
                association: association(db, &r.item_id)?,
            })
        })
    }
}

/// Reads a destination namespace by identity, not by name-based association.
fn namespace(
    db: &Connection,
    directory: &crate::operations::DirectoryRef,
) -> InventoryResult<NamespaceGuard> {
    if let crate::operations::DirectoryRef::Remote { reference, .. } = directory {
        if reference.account_id != sync_info(db)?.account_id {
            return Err(failure(InventoryError::InvalidIdentity));
        }
    }
    // A conservative scope epoch covers new children, metadata, identity and
    // baseline changes without rescanning every child for each planned file.
    let epoch: i64 = db
        .query_row("SELECT epoch FROM planning_clock WHERE id=1", [], |r| {
            r.get(0)
        })
        .map_err(sql)?;
    Ok(NamespaceGuard {
        directory: directory.clone(),
        fingerprint: Sha256::digest(enc(&(directory, epoch))?).to_vec(),
    })
}

impl InventoryManager {
    /// A new child invalidates a prepared destination even if old item revisions stay unchanged.
    pub fn get_namespace(&self, r: GetNamespaceRequest) -> InventoryResult<GetNamespaceResult> {
        self.run(false, |db| {
            Ok(GetNamespaceResult {
                guard: namespace(db, &r.directory)?,
            })
        })
    }
}

/// Structural validation of persisted command references; policy remains Analyzer-owned.
pub(super) fn command_scope(
    command: &crate::operations::OperationCommand,
    account: &AccountId,
) -> InventoryResult<()> {
    use crate::operations::*;
    fn resource(r: &ResourceRef, account: &AccountId) -> InventoryResult<()> {
        match r {
            ResourceRef::Local { relative_path, .. }
                if relative_path.as_bytes().split(|b| *b == b'/').next()
                    == Some(b".drivesync".as_slice()) =>
            {
                Err(failure(InventoryError::InvalidIdentity))
            }
            ResourceRef::Remote {
                provider_id,
                reference,
            } if reference.account_id != *account
                || reference.file_id.0.trim().is_empty()
                || provider_id.0.trim().is_empty() =>
            {
                Err(failure(InventoryError::InvalidIdentity))
            }
            _ => Ok(()),
        }
    }
    fn directory(d: &DirectoryRef, a: &AccountId) -> InventoryResult<()> {
        match d {
            DirectoryRef::LocalRoot => Ok(()),
            DirectoryRef::Local {
                relative_path,
                local_id,
            } => resource(
                &ResourceRef::Local {
                    relative_path: relative_path.clone(),
                    local_id: local_id.clone(),
                },
                a,
            ),
            DirectoryRef::Remote {
                provider_id,
                reference,
            } => resource(
                &ResourceRef::Remote {
                    provider_id: provider_id.clone(),
                    reference: reference.clone(),
                },
                a,
            ),
        }
    }
    fn source(s: &ContentSource, a: &AccountId) -> InventoryResult<()> {
        match s {
            ContentSource::RemoteFile { resource: r } => resource(r, a),
            ContentSource::LocalFile {
                relative_path,
                local_id,
            } => resource(
                &ResourceRef::Local {
                    relative_path: relative_path.clone(),
                    local_id: local_id.clone(),
                },
                a,
            ),
            _ => Ok(()),
        }
    }
    match command {
        OperationCommand::CreateFile {
            parent, content, ..
        } => {
            directory(parent, account)?;
            if let Some(c) = content {
                source(c, account)?
            }
        }
        OperationCommand::CreateDirectory { parent, .. } => directory(parent, account)?,
        OperationCommand::Upload {
            source: s,
            destination,
            ..
        }
        | OperationCommand::Download {
            source: s,
            destination,
            ..
        } => {
            resource(s, account)?;
            directory(destination, account)?
        }
        OperationCommand::UpdateContent {
            target, source: s, ..
        } => {
            resource(target, account)?;
            source(s, account)?
        }
        OperationCommand::Rename { target, .. } | OperationCommand::Trash { target, .. } => {
            resource(target, account)?
        }
        OperationCommand::Move {
            target,
            destination,
            ..
        } => {
            resource(target, account)?;
            directory(destination, account)?
        }
        OperationCommand::Restore {
            target,
            destination,
            ..
        } => {
            resource(target, account)?;
            if let Some(d) = destination {
                directory(d, account)?
            }
        }
    }
    Ok(())
}
