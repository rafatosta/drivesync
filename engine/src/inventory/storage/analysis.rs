//! Atomic persistence of analysis facts and non-executable blocked work intent.
use super::{
    items,
    pagination::{self, enum_text, enum_value},
    *,
};
use rusqlite::{params, types::Value, OptionalExtension};
use sha2::{Digest, Sha256};

fn digest(value: &impl serde::Serialize) -> InventoryResult<Vec<u8>> {
    Ok(Sha256::digest(
        serde_json::to_vec(value).map_err(|_| failure(InventoryError::SerializationFailed))?,
    )
    .to_vec())
}
fn assessment(record: &RecordedAnalysis) -> NewAnalysisAssessment {
    NewAnalysisAssessment {
        plan: record.plan.clone(),
        disposition: record.disposition,
        findings: record.findings.clone(),
        requirements: record.requirements.clone(),
        intents: record.decisions.iter().map(|d| d.intent.clone()).collect(),
    }
}
pub(super) fn load(db: &Connection, id: i64) -> InventoryResult<RecordedAnalysis> {
    let mut record = db.query_row(
        "SELECT item,item_kind,base_revision,result_revision,input_digest,recorded,disposition,baseline_confirmed,local_complete,remote_complete,local_content,remote_content,current_content,local_parent,current_parents FROM analyses WHERE seq=?1", [id], |r| {
            Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,u64>(2)?,r.get::<_,u64>(3)?,r.get::<_,Vec<u8>>(4)?,r.get::<_,i64>(5)?,r.get::<_,String>(6)?,r.get::<_,bool>(7)?,r.get::<_,bool>(8)?,r.get::<_,bool>(9)?,r.get::<_,String>(10)?,r.get::<_,String>(11)?,r.get::<_,String>(12)?,r.get::<_,Option<String>>(13)?,r.get::<_,String>(14)?))
        }).map_err(sql).and_then(|r| Ok(RecordedAnalysis { plan: None,
            id: AnalysisId(id), item_id: SyncItemId(r.0), item_kind: enum_value(r.1)?, analyzed_item_revision:r.2, resulting_item_revision:r.3,input_fingerprint:r.4,recorded_at:Timestamp{unix_millis:r.5},disposition:enum_value(r.6)?,
            evidence:AnalysisEvidence{baseline_confirmed:r.7,local_scope_complete:r.8,remote_scope_complete:r.9,local_content:enum_value(r.10)?,remote_content:enum_value(r.11)?,current_content:enum_value(r.12)?,local_parent:r.13.map(SyncItemId),current_parents:enum_value(r.14)?},
            findings:vec![],requirements:vec![],decisions:vec![],stale:false,
        }))?;
    let mut stmt=db.prepare("SELECT origin,dimension,status,reason FROM analysis_findings WHERE analysis=?1 ORDER BY position").map_err(sql)?;
    for row in stmt
        .query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(sql)?
    {
        let r = row.map_err(sql)?;
        record.findings.push(AnalysisFinding {
            origin: enum_value(r.0)?,
            dimension: enum_value(r.1)?,
            status: enum_value(r.2)?,
            reason: enum_value(r.3)?,
        });
    }
    let mut stmt = db
        .prepare("SELECT kind FROM analysis_requirements WHERE analysis=?1 ORDER BY position")
        .map_err(sql)?;
    for row in stmt
        .query_map([id], |r| r.get::<_, String>(0))
        .map_err(sql)?
    {
        record.requirements.push(enum_value(row.map_err(sql)?)?);
    }
    let mut stmt=db.prepare("SELECT direction,kind,operation FROM analysis_decisions WHERE analysis=?1 ORDER BY position").map_err(sql)?;
    for row in stmt
        .query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(sql)?
    {
        let r = row.map_err(sql)?;
        record.decisions.push(AnalysisDecision {
            intent: AnalysisIntent {
                direction: enum_value(r.0)?,
                kind: enum_value(r.1)?,
            },
            operation_id: OperationId(r.2),
        });
    }
    record.plan = super::planning::load_plan(db, id)?;
    record.stale = !super::planning::plan_current(db, record.plan.as_ref())?
        || items::load(db, &record.item_id)?.revision != record.resulting_item_revision;
    Ok(record)
}
fn validate(request: &RecordAnalysisRequest) -> InventoryResult<()> {
    let a = &request.assessment;
    if a.findings.is_empty()
        || a.findings.len() > 64
        || a.requirements.len() > 64
        || a.intents.len() > 32
        || request.evidence.baseline_confirmed && request.expected_item.baseline.is_none()
    {
        return Err(failure(InventoryError::InvalidArgument));
    }
    let conflict = a
        .findings
        .iter()
        .any(|f| f.status == AnalysisFindingStatus::Divergent);
    let expected = if conflict {
        AnalysisDisposition::PendingUserDecision
    } else if !a.requirements.is_empty()
        || a.plan
            .as_ref()
            .is_some_and(|p| p.steps.iter().any(|s| !s.blocked_by.is_empty()))
    {
        AnalysisDisposition::PendingEvidence
    } else if !a.intents.is_empty() {
        AnalysisDisposition::PendingOperations
    } else {
        AnalysisDisposition::NoChanges
    };
    if a.disposition != expected
        || a.requirements
            .iter()
            .enumerate()
            .any(|(i, r)| a.requirements[..i].contains(r))
        || a.intents
            .iter()
            .enumerate()
            .any(|(i, r)| a.intents[..i].contains(r))
    {
        return Err(failure(InventoryError::InvalidArgument));
    }
    if a.findings
        .iter()
        .any(|f| f.status == AnalysisFindingStatus::Unknown)
        && a.requirements.is_empty()
    {
        return Err(failure(InventoryError::InvalidArgument));
    }
    if !a.intents.is_empty()
        && !matches!(
            request.expected_item.kind,
            ItemKind::File | ItemKind::Directory
        )
    {
        return Err(failure(InventoryError::InvalidArgument));
    }
    if let Some(plan) = &a.plan {
        if plan.steps.len() != a.intents.len() {
            return Err(failure(InventoryError::InvalidArgument));
        }
    }
    for intent in &a.intents {
        if !matches!(
            intent.kind,
            OperationKind::Create
                | OperationKind::UpdateContent
                | OperationKind::Rename
                | OperationKind::Move
                | OperationKind::Trash
                | OperationKind::Restore
        ) {
            return Err(failure(InventoryError::InvalidArgument));
        }
        if intent.kind == OperationKind::UpdateContent
            && request.expected_item.kind != ItemKind::File
        {
            return Err(failure(InventoryError::InvalidArgument));
        }
        if intent.kind == OperationKind::Trash
            && intent.direction == OperationDirection::Upload
            && !a
                .requirements
                .contains(&AnalysisRequirement::AuthorizeDeletion)
        {
            return Err(failure(InventoryError::InvalidArgument));
        }
        if intent.kind == OperationKind::Restore
            && a.plan
                .as_ref()
                .and_then(|p| p.restore_request.as_ref())
                .is_none_or(|r| r.decision_id.trim().is_empty())
        {
            return Err(failure(InventoryError::InvalidArgument));
        }
        if matches!(intent.kind, OperationKind::Trash | OperationKind::Restore) {
            if request.assessment.plan.is_none() {
                return Err(failure(InventoryError::InvalidArgument));
            }
            continue;
        }
        let present = match intent.direction {
            OperationDirection::Upload => &request.expected_item.local_presence,
            OperationDirection::Download => &request.expected_item.remote_presence,
        };
        if *present != PresenceState::Present {
            return Err(failure(InventoryError::MissingObservation));
        }
    }
    Ok(())
}
impl InventoryManager {
    /// Records one evaluation, its diagnostic status and blocked operations in one
    /// transaction. Does not confirm baseline or emit completion activities.
    /// Exact retries ignore the new timestamp and return the original receipt.
    /// An unchanged current receipt is reused without bumping revisions or queuing duplicates.
    pub fn record_analysis(
        &self,
        request: RecordAnalysisRequest,
    ) -> InventoryResult<RecordAnalysisResult> {
        validate(&request)?;
        self.run(true, |db| {
            if sync_info(db)?.sync_id!=request.expected_sync_id {return Err(failure(InventoryError::InvalidIdentity));}
            let request_digest=digest(&(&request.expected_sync_id,&request.expected_item,&request.evidence,&request.assessment))?;
            let existing:Option<(i64,Vec<u8>)>=db.query_row("SELECT seq,request_digest FROM analyses WHERE item=?1 AND base_revision=?2",params![request.expected_item.id.0,request.expected_item.revision],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql)?;
            if let Some((id,old))=existing {
                if old!=request_digest{return Err(failure(InventoryError::VersionMismatch));}
                return Ok(RecordAnalysisResult{analysis:load(db,id)?});
            }
            let item=items::load(db,&request.expected_item.id)?;
            if item!=request.expected_item {return Err(failure(InventoryError::VersionMismatch));}
            let previous:Option<i64>=db.query_row("SELECT seq FROM analyses WHERE item=?1 AND result_revision=?2 ORDER BY seq DESC LIMIT 1",params![item.id.0,item.revision],|r|r.get(0)).optional().map_err(sql)?;
            if let Some(id)=previous {let old=load(db,id)?;if old.evidence==request.evidence && assessment(&old)==request.assessment {return Ok(RecordAnalysisResult{analysis:old});}}
            super::planning::validate_plan(db,request.assessment.plan.as_ref())?;
            let running:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM operations WHERE item=?1 AND state='Running')",[&item.id.0],|r|r.get(0)).map_err(sql)?;
            if running {return Err(failure(InventoryError::InvalidTransition));}
            items::bump(db,&item.id)?;
            let revision=item.revision+1;
            let status=if request.assessment.disposition==AnalysisDisposition::PendingUserDecision {SyncStatus::Conflict} else if request.assessment.disposition==AnalysisDisposition::NoChanges && item.status==SyncStatus::Synced && item.baseline.is_some() {SyncStatus::Synced} else {SyncStatus::PendingReconciliation};
            db.execute("UPDATE items SET status=?2,conflict_reason=?3 WHERE id=?1",params![item.id.0,enum_text(&status)?,if status==SyncStatus::Conflict{Some("AnalyzerUserDecisionRequired")}else{None}]).map_err(sql)?;
            let evidence=&request.evidence;
            db.execute("INSERT INTO analyses(item,item_kind,base_revision,result_revision,input_digest,request_digest,recorded,disposition,baseline_confirmed,local_complete,remote_complete,local_content,remote_content,current_content,local_parent,current_parents) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",params![item.id.0,enum_text(&item.kind)?,item.revision,revision,digest(&item)?,request_digest,request.recorded_at.unix_millis,enum_text(&request.assessment.disposition)?,evidence.baseline_confirmed,evidence.local_scope_complete,evidence.remote_scope_complete,enum_text(&evidence.local_content)?,enum_text(&evidence.remote_content)?,enum_text(&evidence.current_content)?,evidence.local_parent.as_ref().map(|p|&p.0),enum_text(&evidence.current_parents)?]).map_err(sql)?;
            let id=db.last_insert_rowid();
            for (position,f) in request.assessment.findings.iter().enumerate() {db.execute("INSERT INTO analysis_findings VALUES(?1,?2,?3,?4,?5,?6)",params![id,position as i64,enum_text(&f.origin)?,enum_text(&f.dimension)?,enum_text(&f.status)?,enum_text(&f.reason)?]).map_err(sql)?;}
            for (position,r) in request.assessment.requirements.iter().enumerate() {db.execute("INSERT INTO analysis_requirements VALUES(?1,?2,?3)",params![id,position as i64,enum_text(r)?]).map_err(sql)?;}
            for (position,intent) in request.assessment.intents.iter().enumerate() {
                let block=match request.assessment.disposition {AnalysisDisposition::PendingUserDecision=>"AnalyzerConflictPending",AnalysisDisposition::PendingEvidence=>"AnalyzerEvidencePending",_=>"AnalyzerExecutionUnavailable"};
                let version=item.remote_latest.as_ref().and_then(|r|if let MetadataValue::Known(v)=&r.version{Some(v)}else{None});
                db.execute("INSERT INTO operations(item,direction,kind,state,base_revision,expected_version,created,last_error,fingerprint) VALUES(?1,?2,?3,'Blocked',?4,?5,?6,?7,?8)",params![item.id.0,enum_text(&intent.direction)?,enum_text(&intent.kind)?,revision,version,request.recorded_at.unix_millis,block,digest(&(id,position,intent))?]).map_err(sql)?;
                let operation=db.last_insert_rowid();
                db.execute("INSERT INTO analysis_decisions VALUES(?1,?2,?3,?4,?5,?6)",params![id,item.id.0,position as i64,enum_text(&intent.direction)?,enum_text(&intent.kind)?,operation]).map_err(sql)?;
            }
            super::planning::save_plan(db,id,request.assessment.plan.as_ref())?;
            Ok(RecordAnalysisResult{analysis:load(db,id)?})
        })
    }
    /// Reads a durable receipt with current staleness, including after restart.
    pub fn get_analysis(&self, request: GetAnalysisRequest) -> InventoryResult<GetAnalysisResult> {
        self.run(false, |db| {
            let id = match &request.selector {
                AnalysisSelector::Id(id) => {
                    db.query_row("SELECT seq FROM analyses WHERE seq=?1", [id.0], |r| {
                        r.get(0)
                    })
                }
                AnalysisSelector::LatestForItem(item) => db.query_row(
                    "SELECT seq FROM analyses WHERE item=?1 ORDER BY seq DESC LIMIT 1",
                    [&item.0],
                    |r| r.get(0),
                ),
            }
            .optional()
            .map_err(sql)?;
            Ok(GetAnalysisResult {
                analysis: id.map(|id| load(db, id)).transpose()?,
            })
        })
    }
    /// Lists bounded, filter-bound pages. Current-only excludes stale historical decisions.
    pub fn list_analyses(
        &self,
        request: ListAnalysesRequest,
    ) -> InventoryResult<ListAnalysesResult> {
        self.run(false,|db|{
            let filter=&request.filter;
            let (after,through,signature)=pagination::bounds(db,"analyses",&("analyses",filter),&request.pagination)?;
            let mut query="SELECT a.seq FROM analyses a JOIN items i ON i.id=a.item WHERE a.seq>? AND a.seq<=?".to_owned();let mut values=vec![Value::Integer(after),Value::Integer(through)];
            if let Some(item)=&filter.item_id{query.push_str(" AND a.item=?");values.push(Value::Text(item.0.clone()));}
            if filter.current_only{query.push_str(" AND a.result_revision=i.revision");}
            if !filter.dispositions.is_empty(){query.push_str(&format!(" AND a.disposition IN ({})",vec!["?";filter.dispositions.len()].join(",")));for d in &filter.dispositions{values.push(Value::Text(enum_text(d)?));}}
            query.push_str(" ORDER BY a.seq LIMIT ?");values.push(Value::Integer(i64::from(request.pagination.limit)+1));
            let mut stmt=db.prepare(&query).map_err(sql)?;
            let mut ids=stmt.query_map(rusqlite::params_from_iter(values),|r|r.get::<_,i64>(0)).map_err(sql)?.collect::<Result<Vec<_>,_>>().map_err(sql)?;
            let more=ids.len()>request.pagination.limit as usize;if more{ids.pop();}
            let next_cursor=if more{Some(pagination::next(db,signature,*ids.last().unwrap(),through)?)}else{None};
            let entries=ids.into_iter().map(|id|load(db,id)).collect::<InventoryResult<Vec<_>>>()?.into_iter().filter(|a| !filter.current_only || !a.stale).collect();
            Ok(ListAnalysesResult{page:Page{entries,next_cursor}})
        })
    }
}
/// Analysis-linked operations cannot be released through generic queue APIs,
/// even when a plan has a command or an execution checkpoint exists. Eligibility
/// and execution authorization require a separate future integration protocol.
pub(super) fn ensure_executable(db: &Connection, id: &OperationId) -> InventoryResult<()> {
    let incomplete: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM analysis_decisions WHERE operation=?1)",
            [id.0],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if incomplete {
        return Err(failure(InventoryError::InvalidTransition));
    }
    Ok(())
}
