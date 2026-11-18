//! Analyzer execution ownership. Generic claim/requeue guards stay intact.
use super::*;
use crate::{inventory::creation::*, operations::*};
use rusqlite::{params, OptionalExtension};

fn binding(db: &Connection, item: &SyncItemId) -> InventoryResult<Option<CreationBinding>> {
    let row:Option<(i64,String)>=db.query_row("SELECT o.seq,o.dedup FROM operations o JOIN analysis_decisions d ON d.operation=o.seq WHERE o.item=?1 AND o.dedup LIKE 'execution:%' ORDER BY o.seq DESC LIMIT 1",[&item.0],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql)?;
    row.map(|(op, key)| {
        let id = ExecutionId::new(key.trim_start_matches("execution:").to_owned())
            .map_err(|_| failure(InventoryError::CorruptDatabase))?;
        Ok(CreationBinding {
            operation: super::operations::load(db, &OperationId(op))?,
            intent: super::execution::load(db, &id)?.intent,
        })
    })
    .transpose()
}
fn same_source(item: &SyncItem, intent: &ExecutionIntent) -> bool {
    if item.baseline.is_some()
        && matches!(
            intent.command,
            OperationCommand::Upload { .. }
                | OperationCommand::Download { .. }
                | OperationCommand::CreateDirectory { .. }
        )
    {
        return false;
    }
    match &intent.context.source {
        Some(SourceEvidence::Local { metadata, .. }) => {
            item.local_presence == PresenceState::Present
                && item.local_latest.as_ref().or(item.local.as_ref()) == Some(metadata.as_ref())
        }
        Some(SourceEvidence::Remote { metadata, .. }) => {
            item.remote_presence == PresenceState::Present
                && item.remote_latest.as_ref().or(item.remote.as_ref()) == Some(metadata.as_ref())
        }
        None => false,
    }
}
impl InventoryManager {
    /// Finds due original claims without scanning every item or exposing SQL.
    pub fn due_creations(&self, r: DueCreationsRequest) -> InventoryResult<Vec<SyncItemId>> {
        if r.limit == 0 || r.limit > 256 {
            return Err(failure(InventoryError::InvalidArgument));
        }
        self.run(false,|db| {
            let mut statement=db.prepare("SELECT DISTINCT item FROM operations WHERE dedup LIKE 'execution:%' AND state IN ('Running','RetryWait') AND attempts<5 AND (next_attempt IS NULL OR next_attempt<=?1) LIMIT ?2").map_err(sql)?;
            let rows=statement.query_map(params![r.at.unix_millis,r.limit],|row|Ok(SyncItemId(row.get(0)?))).map_err(sql)?;
            rows.collect::<Result<Vec<_>,_>>().map_err(sql)
        })
    }
    /// Reads the original execution ID even if newer analyses have been recorded.
    pub fn get_creation(&self, r: GetCreationRequest) -> InventoryResult<Option<CreationBinding>> {
        self.run(false, |db| binding(db, &r.item_id))
    }
    /// Exact current plans alone may start execution. Resumption keeps its
    /// intent/ID and source proof; completed effects are only reverified.
    pub fn begin_creation(&self, r: BeginCreationRequest) -> InventoryResult<CreationBinding> {
        self.run(true,|db|{
            let analysis=super::analysis::load(db,r.analysis_id.0)?;
            let item=super::items::load(db,&analysis.item_id)?;
            super::items::revision(&item,r.expected_item_revision)?;
            let existing=binding(db,&item.id)?.filter(|b| b.operation.state!=OperationState::Completed || b.intent.context.id==r.intent.context.id);
            let op=if let Some(existing)=existing {
                if existing.intent!=r.intent {return Err(failure(InventoryError::VersionMismatch));}
                let dispatched=super::execution::load(db,&existing.intent.context.id)?.state.phase!=CheckpointPhase::Prepared;
                if !dispatched && !same_source(&item,&r.intent) {return Err(failure(InventoryError::VersionMismatch));}
                if existing.operation.state==OperationState::Completed {return Ok(existing);}
                if existing.operation.next_attempt_at.is_some_and(|t|t.unix_millis>r.started_at.unix_millis)||existing.operation.attempt_count>=5 {return Err(failure(InventoryError::InvalidTransition));}
                existing.operation
            } else {
                if analysis.stale || !analysis.requirements.is_empty() || !matches!(analysis.disposition,AnalysisDisposition::PendingOperations|AnalysisDisposition::PendingEvidence) || !same_source(&item,&r.intent) {return Err(failure(InventoryError::InvalidTransition));}
                let plan=analysis.plan.as_ref().ok_or_else(||failure(InventoryError::InvalidTransition))?;
                if !plan.manual.is_empty()||plan.steps.is_empty()||!plan.steps[0].blocked_by.is_empty()||plan.steps[0].command.as_ref()!=Some(&r.intent.command)||!super::planning::plan_current(db,Some(plan))? {return Err(failure(InventoryError::VersionMismatch));}
                let decision=analysis.decisions.first().ok_or_else(||failure(InventoryError::InvalidTransition))?;
                let unbound=match decision.intent.direction {OperationDirection::Upload=>item.remote_ref.is_none()&&item.remote.is_none()&&item.remote_latest.is_none(),OperationDirection::Download=>item.local_file_id.is_none()&&item.local.is_none()&&item.local_latest.is_none()};
                if decision.intent.kind==OperationKind::Create && !unbound {return Err(failure(InventoryError::DuplicateAssociation));}
                let local_trash = decision.intent.kind == OperationKind::Trash
                    && item.kind == ItemKind::File
                    && r.intent.direction == OperationDirection::Download
                    && item.local_presence == PresenceState::Present
                    && item.remote_presence == PresenceState::AbsentConfirmed
                    && item.baseline.as_ref().is_some_and(|baseline| {
                        baseline.local.presence == PresenceState::Present
                            && baseline.remote.presence == PresenceState::Present
                    })
                    && matches!(&r.intent.command, OperationCommand::Trash { target: ResourceRef::Local { .. }, .. });
                if decision.intent.kind!=OperationKind::Create && (!matches!(decision.intent.kind,OperationKind::Rename|OperationKind::Move|OperationKind::UpdateContent)||item.baseline.is_none()) && !local_trash {return Err(failure(InventoryError::InvalidTransition));}
                super::execution::prepare(db,PrepareExecutionRequest {intent:r.intent.clone()})?;
                db.execute("UPDATE operations SET dedup=?2 WHERE seq=?1 AND dedup IS NULL",params![decision.operation_id.0,format!("execution:{}",r.intent.context.id.as_str())]).map_err(sql)?;
                super::operations::load(db,&decision.operation_id)?
            };
            if op.attempt_count>=5 {return Err(failure(InventoryError::InvalidTransition));}
            db.execute("UPDATE operations SET state='Running',attempts=attempts+1,started=?2,next_attempt=NULL,last_error=NULL WHERE seq=?1",params![op.id.0,r.started_at.unix_millis]).map_err(sql)?;
            Ok(CreationBinding {operation:super::operations::load(db,&op.id)?,intent:r.intent})
        })
    }
    /// Completes one verified physical step without confirming a partially
    /// converged baseline. Subsequent observations must produce a fresh plan.
    pub fn complete_execution_step(&self, r: CompleteExecutionStepRequest) -> InventoryResult<()> {
        self.run(true, |db| {
            let current = binding(db, &r.binding.operation.item_id)?.ok_or_else(|| failure(InventoryError::NotFound))?;
            if current.intent != r.binding.intent || current.operation.attempt_count != r.binding.operation.attempt_count {
                return Err(failure(InventoryError::VersionMismatch));
            }
            if current.operation.state == OperationState::Completed { return Ok(()); }
            let checkpoint = super::execution::load(db, &current.intent.context.id)?;
            if current.operation.state != OperationState::Running || checkpoint.state.phase != CheckpointPhase::Completed {
                return Err(failure(InventoryError::InvalidTransition));
            }
            db.execute("UPDATE operations SET state='Completed',completed=?2,next_attempt=NULL,last_error=NULL WHERE seq=?1", params![current.operation.id.0,r.at.unix_millis]).map_err(sql)?;
            super::activity::append(db, &NewActivity {
                item_id: current.operation.item_id.clone(), operation_id: Some(current.operation.id),
                event_type: match current.operation.kind {
                    OperationKind::Rename => ActivityType::Rename,
                    OperationKind::Move => ActivityType::Move,
                    _ => if current.operation.direction == OperationDirection::Upload { ActivityType::Upload } else { ActivityType::Download },
                }, origin: ActivityOrigin::System, outcome: ActivityOutcome::Completed, occurred_at: r.at,
                description: Some("Verified execution step; reconciliation still pending".into()),
                error_kind: None,item_name: None,item_remote_id: None,item_local_path: None,
            })?;
            super::items::bump(db, &current.operation.item_id)?;
            Ok(())
        })
    }
    /// Atomically records a verified remote-to-local trash effect and completes
    /// its durable operation. The private trash path is never indexed as a live item.
    pub fn record_trashed_local(&self, r: RecordTrashedLocalRequest) -> InventoryResult<SyncItem> {
        self.run(true, |db| {
            let current = binding(db, &r.binding.operation.item_id)?
                .ok_or_else(|| failure(InventoryError::NotFound))?;
            if current.intent != r.binding.intent
                || current.operation.attempt_count != r.binding.operation.attempt_count
            {
                return Err(failure(InventoryError::VersionMismatch));
            }
            let OperationCommand::Trash {
                target:
                    ResourceRef::Local {
                        relative_path,
                        local_id: Some(local_id),
                    },
                ..
            } = &current.intent.command
            else {
                return Err(failure(InventoryError::InvalidTransition));
            };
            if current.intent.direction != OperationDirection::Download
                || current.operation.kind != OperationKind::Trash
                || current
                    .operation
                    .started_at
                    .is_some_and(|started| r.trashed_at.unix_millis < started.unix_millis)
            {
                return Err(failure(InventoryError::InvalidTransition));
            }
            let checkpoint = super::execution::load(db, &current.intent.context.id)?;
            if checkpoint.state.phase != CheckpointPhase::Completed
                || checkpoint.state.result.as_ref() != Some(&r.result)
                || r.result.verification != VerificationStatus::Verified
            {
                return Err(failure(InventoryError::InvalidTransition));
            }
            let trash_path = crate::models::LocalRelativePath::new(
                format!(".drivesync/trash/{}/item", current.intent.context.id.as_str())
                    .into_bytes(),
            )
            .map_err(|_| failure(InventoryError::InvalidArgument))?;
            match (&r.result.resource, &r.result.metadata) {
                (
                    ResourceRef::Local {
                        relative_path,
                        local_id: Some(result_id),
                    },
                    Some(FileData::Local(metadata)),
                ) if relative_path == &trash_path
                    && result_id == local_id
                    && metadata.relative_path == trash_path
                    && metadata.local_id.as_ref() == Some(result_id)
                    && metadata.validate().is_ok() => {}
                _ => return Err(failure(InventoryError::InvalidIdentity)),
            }
            let item = super::items::load(db, &current.operation.item_id)?;
            let Some(SourceEvidence::Local { metadata, .. }) = &current.intent.context.source else {
                return Err(failure(InventoryError::InvalidIdentity));
            };
            if current.operation.state == OperationState::Completed
                && item.local_presence == PresenceState::AbsentConfirmed
                && item.remote_presence == PresenceState::AbsentConfirmed
                && item.local_file_id.is_none()
            {
                return super::items::load(db, &current.operation.item_id);
            }
            if item.kind != ItemKind::File
                || item.local_presence != PresenceState::Present
                || item.remote_presence != PresenceState::AbsentConfirmed
                || item.local_file_id.as_ref() != Some(local_id)
                || item.local_latest.as_ref().or(item.local.as_ref()) != Some(metadata.as_ref())
                || metadata.relative_path != *relative_path
                || metadata.local_id.as_ref() != Some(local_id)
                || current.operation.state != OperationState::Running
            {
                return Err(failure(InventoryError::VersionMismatch));
            }
            let deleted = db
                .execute(
                    "DELETE FROM local_files WHERE item=?1 AND path=?2",
                    params![item.id.0, relative_path.as_bytes()],
                )
                .map_err(sql)?;
            if deleted != 1 {
                return Err(failure(InventoryError::VersionMismatch));
            }
            let absent = enum_text(&PresenceState::AbsentConfirmed)?;
            db.execute(
                "UPDATE items SET local_id=NULL,local_presence=?2,remote_presence=?2,status='Synced',conflict_reason=NULL,last_checked=?3,last_synced=?3 WHERE id=?1",
                params![item.id.0, absent, r.trashed_at.unix_millis],
            )
            .map_err(sql)?;
            let baselines = db
                .execute(
                    "UPDATE baselines SET presence=?2,confirmed=?3 WHERE item=?1",
                    params![item.id.0, absent, r.trashed_at.unix_millis],
                )
                .map_err(sql)?;
            if baselines != 2 {
                return Err(failure(InventoryError::MissingObservation));
            }
            let completed = db.execute(
                "UPDATE operations SET state='Completed',completed=?2,next_attempt=NULL,last_error=NULL WHERE seq=?1 AND state='Running'",
                params![current.operation.id.0, r.trashed_at.unix_millis],
            )
            .map_err(sql)?;
            if completed != 1 {
                return Err(failure(InventoryError::VersionMismatch));
            }
            super::activity::append(
                db,
                &NewActivity {
                    item_id: item.id.clone(),
                    operation_id: Some(current.operation.id),
                    event_type: ActivityType::Trash,
                    origin: ActivityOrigin::System,
                    outcome: ActivityOutcome::Completed,
                    occurred_at: r.trashed_at,
                    description: Some("Remote deletion moved the local item to trash".into()),
                    error_kind: None,
                    item_name: Some(
                        String::from_utf8_lossy(
                            relative_path.as_bytes().rsplit(|byte| *byte == b'/').next().unwrap(),
                        )
                        .into_owned(),
                    ),
                    item_remote_id: item.remote_ref.as_ref().map(|reference| reference.file_id.0.clone()),
                    item_local_path: Some(relative_path.as_bytes().to_vec()),
                },
            )?;
            super::items::bump(db, &item.id)?;
            super::items::load(db, &item.id)
        })
    }
    /// Records only the exact resulting identity verified by this execution.
    /// A duplicate observation of that identity may be retired; unrelated items,
    /// active executions and preexisting baselines are never merged or overwritten.
    pub fn record_created_replica(
        &self,
        r: RecordCreatedReplicaRequest,
    ) -> InventoryResult<SyncItem> {
        self.run(true,|db|{
            let current=binding(db,&r.binding.operation.item_id)?.ok_or_else(||failure(InventoryError::NotFound))?;
            if current.intent!=r.binding.intent||current.operation.attempt_count!=r.binding.operation.attempt_count {return Err(failure(InventoryError::VersionMismatch));}
            let checkpoint=super::execution::load(db,&current.intent.context.id)?;
            let result=checkpoint.state.result.as_ref().filter(|_|checkpoint.state.phase==CheckpointPhase::Completed).ok_or_else(||failure(InventoryError::InvalidTransition))?;
            if result.item().resource!=r.result.resource||r.result.verification!=VerificationStatus::Verified {return Err(failure(InventoryError::InvalidIdentity));}
            let item=super::items::load(db,&current.operation.item_id)?;
            if current.operation.state==OperationState::Completed {return Ok(item);}
            let data=r.result.metadata.as_ref().ok_or_else(||failure(InventoryError::MissingObservation))?;
            let duplicate:Option<String>=match data {
                FileData::Local(local)=>db.query_row("SELECT item FROM local_files WHERE path=?1 AND item!=?2",params![local.relative_path.as_bytes(),item.id.0],|r|r.get(0)).optional().map_err(sql)?,
                FileData::Remote(remote)=>db.query_row("SELECT id FROM items WHERE remote_account=?1 AND remote_id=?2 AND id!=?3",params![remote.reference.account_id.0,remote.reference.file_id.0,item.id.0],|r|r.get(0)).optional().map_err(sql)?,
            };
            if let Some(duplicate)=duplicate {
                let old=super::items::load(db,&SyncItemId(duplicate.clone()))?;
                let safe=old.baseline.is_none() && binding(db,&old.id)?.is_none() && match data {
                    FileData::Local(local)=>old.remote_ref.is_none() && old.local.as_ref().is_some_and(|observed|observed.storage.inode==local.storage.inode&&observed.storage.filesystem_device==local.storage.filesystem_device&&observed.times.created_at==local.times.created_at),
                    FileData::Remote(remote)=>old.local.is_none()&&old.remote_ref.as_ref()==Some(&remote.reference),
                };
                if !safe {return Err(failure(InventoryError::DuplicateAssociation));}
                let side=if matches!(data,FileData::Remote(_)){1}else{0};
                for table in if side==0 {vec!["local_files"]}else{vec!["remote_files","remote_parents"]} {db.execute(&format!("UPDATE {table} SET item=?1 WHERE item=?2"),params![item.id.0,duplicate]).map_err(sql)?;}
                db.execute("UPDATE metadata_fields SET item=?1 WHERE item=?2 AND side=?3",params![item.id.0,duplicate,side]).map_err(sql)?;
                db.execute("UPDATE receipts SET item=?1 WHERE item=?2 AND side=?3",params![item.id.0,duplicate,side]).map_err(sql)?;
                db.execute("UPDATE items SET local_id=NULL,remote_account=NULL,remote_id=NULL,local_presence='Unknown',remote_presence='Unknown',status='PendingReconciliation',conflict_reason='ReplicaObservationRetired',revision=revision+1 WHERE id=?1",[duplicate]).map_err(sql)?;
            }
            match data {
                FileData::Local(local)=>{
                    let mutation=matches!(current.intent.command,OperationCommand::Rename {..}|OperationCommand::Move {..}|OperationCommand::UpdateContent {..});
                    if !mutation && (item.local_file_id.as_ref().is_some_and(|id|Some(id)!=local.local_id.as_ref()) || item.local.as_ref().is_some_and(|old|old.relative_path!=local.relative_path)) {return Err(failure(InventoryError::DuplicateAssociation));}
                    super::metadata::local_apply(db,&item.id.0,local)?;
                    db.execute("UPDATE items SET local_id=?2,local_presence='Present' WHERE id=?1",params![item.id.0,local.local_id.as_ref().map(|id|&id.0)]).map_err(sql)?;
                },
                FileData::Remote(remote)=>{
                    if item.remote_ref.as_ref().is_some_and(|r|*r!=remote.reference) {return Err(failure(InventoryError::DuplicateAssociation));}
                    db.execute("UPDATE items SET remote_account=?2,remote_id=?3,remote_presence='Present' WHERE id=?1",params![item.id.0,remote.reference.account_id.0,remote.reference.file_id.0]).map_err(sql)?;
                    super::metadata::remote_apply(db,&item.id.0,remote)?;
                },
            }
            super::items::bump(db,&item.id)?;
            super::items::load(db,&item.id)
        })
    }
    /// Records a bounded retry without losing the original execution identity.
    pub fn fail_creation(&self, r: FailCreationRequest) -> InventoryResult<()> {
        self.run(true, |db| {
            let current = super::operations::load(db, &r.binding.operation.id)?;
            if current.attempt_count != r.binding.operation.attempt_count
                || !matches!(
                    current.state,
                    OperationState::Running | OperationState::RetryWait
                )
            {
                return Err(failure(InventoryError::VersionMismatch));
            }
            let retry = matches!(
                r.error,
                OperatorError::Network
                    | OperatorError::IntegrityMismatch
                    | OperatorError::PermissionDenied
                    | OperatorError::CheckpointUnavailable
                    | OperatorError::CheckpointConflict
                    | OperatorError::IndeterminateEffect
                    | OperatorError::Cancelled
            ) && current.attempt_count < 5;
            let time =
                r.at.unix_millis
                    .saturating_add(30_000i64.saturating_mul(1i64 << current.attempt_count.min(5)));
            db.execute(
                "UPDATE operations SET state=?2,next_attempt=?3,last_error=?4 WHERE seq=?1",
                params![
                    current.id.0,
                    if retry { "RetryWait" } else { "Blocked" },
                    retry.then_some(time),
                    format!("{:?}", r.error)
                ],
            )
            .map_err(sql)?;
            Ok(())
        })
    }
}
