//! Integration-contract RED tests: real facade entry, scripted stage ports.
//! No Inventory, filesystem, network, Operator, or stage algorithms.
use super::{apply_fixtures::{basic, new_observation, independent, multi, mutation}, decision_fixtures as _};
use drivesync_engine::{
    analyzer::{
        Analyzer, AnalyzerEngine, AnalysisContext, AnalysisDecision, AnalysisError,
        AnalysisErrorKind, AnalysisFeedback, AnalysisFuture, AnalysisItemResult,
        AnalysisResult, AnalysisSkipReason, AnalysisStage, AnalysisStatus, AnalysisUpdate,
        ApplyOutcome, ApplyService, ContextService, DecisionOutcome, DecisionService,
        DecisionTarget, NonEmpty,
        context::ContextFuture,
        apply::{ApplyBlock, InterruptionReason},
    },
    inventory::{AnalysisRequirement, OperationDirection, OperationKind, SyncItemId},
    models::File,
    operations::ExecutionId,
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Feedback(AnalysisUpdate),
    Context(File),
    Decision(AnalysisContext),
    Apply(AnalysisDecision),
}
type Log = Arc<Mutex<Vec<Call>>>;

struct ContextPort {
    log: Log,
    result: Result<AnalysisContext, AnalysisError>,
}
impl ContextService for ContextPort {
    fn build<'a>(&'a self, file: File) -> ContextFuture<'a> {
        Box::pin(async move {
            self.log.lock().unwrap().push(Call::Context(file));
            self.result.clone()
        })
    }
}
struct DecisionPort {
    log: Log,
    result: Result<AnalysisDecision, AnalysisError>,
}
impl DecisionService for DecisionPort {
    fn decide(&self, context: AnalysisContext) -> Result<AnalysisDecision, AnalysisError> {
        self.log.lock().unwrap().push(Call::Decision(context));
        self.result.clone()
    }
}
struct ApplyPort {
    log: Log,
    result: Result<AnalysisResult, AnalysisError>,
}
impl ApplyService for ApplyPort {
    fn apply<'a>(&'a self, decision: AnalysisDecision) -> AnalysisFuture<'a> {
        Box::pin(async move {
            self.log.lock().unwrap().push(Call::Apply(decision));
            self.result.clone()
        })
    }
}
struct Feedback(Log);
impl AnalysisFeedback for Feedback {
    fn report(&self, update: AnalysisUpdate) {
        self.0.lock().unwrap().push(Call::Feedback(update));
    }
}
struct Script {
    file: File,
    context: Result<AnalysisContext, AnalysisError>,
    decision: Result<AnalysisDecision, AnalysisError>,
    result: Result<AnalysisResult, AnalysisError>,
}
fn completed(d: &AnalysisDecision, outcome: ApplyOutcome) -> AnalysisResult {
    assert_eq!(d.items.len(), 1);
    AnalysisResult::Completed(vec![AnalysisItemResult { target: d.items[0].target.clone(), outcome }])
}
fn script(d: AnalysisDecision, result: AnalysisResult) -> Script {
    d.context.file.validate().expect("valid integration fixture");
    Script { file: d.context.file.clone(), context: Ok(d.context.clone()), decision: Ok(d), result: Ok(result) }
}
fn no_action() -> Script {
    let d = basic(DecisionOutcome::NoAction);
    let result = completed(&d, ApplyOutcome::NoAction);
    script(d, result)
}
fn error(kind: AnalysisErrorKind) -> AnalysisError { AnalysisError { kind } }
fn run(s: Script) -> (Result<AnalysisResult, AnalysisError>, Vec<Call>) {
    let log = Arc::new(Mutex::new(vec![]));
    let engine = AnalyzerEngine::new(
        ContextPort { log: log.clone(), result: s.context },
        DecisionPort { log: log.clone(), result: s.decision },
        ApplyPort { log: log.clone(), result: s.result },
    );
    let feedback = Feedback(log.clone());
    let analyzer: &dyn Analyzer = &engine;
    let result = tokio::runtime::Builder::new_current_thread().build().unwrap()
        .block_on(analyzer.analyze(s.file, &feedback));
    let calls = log.lock().unwrap().clone();
    (result, calls)
}
fn phases(calls: &[Call]) -> Vec<AnalysisStage> {
    calls.iter().filter_map(|c| match c {
        Call::Feedback(AnalysisUpdate { status: AnalysisStatus::InProgress(stage), .. }) => Some(*stage),
        _ => None,
    }).collect()
}
fn progress(file: &File, stage: AnalysisStage) -> Call {
    Call::Feedback(AnalysisUpdate::new(file, AnalysisStatus::InProgress(stage)))
}
fn full_trace(s: &Script) -> Vec<Call> {
    vec![
        progress(&s.file, AnalysisStage::Inspecting), Call::Context(s.file.clone()),
        progress(&s.file, AnalysisStage::Reconciling), Call::Decision(s.context.as_ref().unwrap().clone()),
        progress(&s.file, AnalysisStage::Applying), Call::Apply(s.decision.as_ref().unwrap().clone()),
    ]
}
fn assert_passthrough(s: Script) {
    let expected = s.result.clone();
    let trace = full_trace(&s);
    let (result, calls) = run(s);
    assert_eq!(calls, trace);
    assert_eq!(result, expected);
}
fn operation_id() -> ExecutionId { ExecutionId::new("facade-operation".into()).unwrap() }
fn action_script(outcome: ApplyOutcome) -> Script {
    let d = mutation(OperationKind::Rename, OperationDirection::Upload);
    let r = completed(&d, outcome);
    script(d, r)
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn a01_exact_stage_order_and_artifacts() {
    assert_passthrough(no_action());
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn a02_remote_payload_preserved() {
    let mut d = new_observation();
    d.context = super::decision_fixtures::unmatched(false);
    d.items[0].target = DecisionTarget::Observation(d.context.file.id.clone());
    d.items[0].outcome = DecisionOutcome::InsufficientEvidence { missing: NonEmpty::new(AnalysisRequirement::ObserveLocal, vec![]) };
    let r = completed(&d, ApplyOutcome::InsufficientEvidence { missing: NonEmpty::new(AnalysisRequirement::ObserveLocal, vec![]) });
    assert!(d.context.file.data.is_some());
    assert_passthrough(script(d, r));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn a03_local_payload_preserved() {
    let d = new_observation();
    assert!(d.context.file.data.is_some());
    let r = completed(&d, ApplyOutcome::InsufficientEvidence { missing: NonEmpty::new(AnalysisRequirement::ObserveRemote, vec![]) });
    assert_passthrough(script(d, r));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn a04_invalid_input_never_enters_stages() {
    let mut s = no_action();
    s.file.id.0.clear();
    let (r, calls) = run(s);
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidContext)));
    assert!(calls.is_empty());
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn a05_context_cannot_replace_original_file() {
    let mut s = no_action();
    s.context.as_mut().unwrap().file.observed_at.unix_millis += 1;
    let (r, calls) = run(s);
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidContext)));
    assert_eq!(phases(&calls), vec![AnalysisStage::Inspecting]);
    assert!(!calls.iter().any(|c| matches!(c, Call::Decision(_) | Call::Apply(_))));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn a06_decision_cannot_replace_context() {
    let mut s = no_action();
    s.decision.as_mut().unwrap().context.file.observed_at.unix_millis += 1;
    let (r, calls) = run(s);
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidDecision)));
    assert_eq!(phases(&calls), vec![AnalysisStage::Inspecting, AnalysisStage::Reconciling]);
    assert!(!calls.iter().any(|c| matches!(c, Call::Apply(_))));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn a07_empty_decision_never_reaches_apply() {
    let mut s = no_action();
    s.decision.as_mut().unwrap().items.clear();
    let (r, calls) = run(s);
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidDecision)));
    assert!(!calls.iter().any(|c| matches!(c, Call::Apply(_))));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn a08_duplicate_decision_targets_rejected() {
    let mut s = no_action();
    let d = s.decision.as_mut().unwrap();
    d.items.push(d.items[0].clone());
    let (r, calls) = run(s);
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidDecision)));
    assert!(!calls.iter().any(|c| matches!(c, Call::Apply(_))));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn b01_context_failure_short_circuits() {
    let mut s = no_action();
    s.context = Err(error(AnalysisErrorKind::Inventory));
    let expected = vec![progress(&s.file, AnalysisStage::Inspecting), Call::Context(s.file.clone())];
    let (r, calls) = run(s);
    assert_eq!(r, Err(error(AnalysisErrorKind::Inventory)));
    assert_eq!(calls, expected);
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn b02_decision_failure_short_circuits() {
    let mut s = no_action();
    s.decision = Err(error(AnalysisErrorKind::InvalidContext));
    let expected = vec![progress(&s.file, AnalysisStage::Inspecting), Call::Context(s.file.clone()), progress(&s.file, AnalysisStage::Reconciling), Call::Decision(s.context.as_ref().unwrap().clone())];
    let (r, calls) = run(s);
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidContext)));
    assert_eq!(calls, expected);
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn b03_apply_failure_propagates() {
    let mut s = no_action();
    s.result = Err(error(AnalysisErrorKind::Operation));
    assert_passthrough(s);
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn b04_persistence_error_never_becomes_success() {
    let mut s = no_action();
    s.result = Err(error(AnalysisErrorKind::Inventory));
    assert_passthrough(s);
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn b05_apply_evidence_failure_propagates() {
    let mut s = no_action();
    s.result = Err(error(AnalysisErrorKind::Evidence));
    assert_passthrough(s);
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn b06_no_retry_inside_facade() {
    let mut s = no_action();
    s.result = Err(error(AnalysisErrorKind::Operation));
    let (r, calls) = run(s);
    assert!(r.is_err());
    assert_eq!(calls.iter().filter(|c| matches!(c, Call::Apply(_))).count(), 1);
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn c01_no_action() {
    assert_passthrough(no_action());
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn c02_baseline_established() {
    let d = basic(DecisionOutcome::EstablishBaseline);
    let r = completed(&d, ApplyOutcome::BaselineEstablished);
    assert_passthrough(script(d, r));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn c03_conflict_is_successful_durable_handling() {
    let d = basic(DecisionOutcome::Conflict { requirements: vec![] });
    let r = completed(&d, ApplyOutcome::ConflictRecorded);
    assert_passthrough(script(d, r));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn c04_evidence_request_is_not_error() {
    let d = new_observation();
    let r = completed(&d, ApplyOutcome::InsufficientEvidence { missing: NonEmpty::new(AnalysisRequirement::ObserveRemote, vec![]) });
    assert_passthrough(script(d, r));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn c05_pending_execution_not_applied() {
    assert_passthrough(action_script(ApplyOutcome::PendingExecution { operations: NonEmpty::new(operation_id(), vec![]) }));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn c06_pending_recovery_not_retried() {
    assert_passthrough(action_script(ApplyOutcome::PendingRecovery { operations: NonEmpty::new(operation_id(), vec![]) }));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn c07_pending_reanalysis_not_recursive() {
    assert_passthrough(action_script(ApplyOutcome::Interrupted { completed: vec![], reason: InterruptionReason::StaleRevision }));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn c08_blocked_outcome_preserved() {
    let d = basic(DecisionOutcome::Blocked { reason: drivesync_engine::analyzer::DecisionBlock::UnsupportedKind });
    let r = completed(&d, ApplyOutcome::Blocked { reason: ApplyBlock::UnsupportedKind });
    assert_passthrough(script(d, r));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn c09_duplicate_proof_only_from_apply() {
    let d = basic(DecisionOutcome::NoAction);
    assert_passthrough(script(d, AnalysisResult::Skipped(AnalysisSkipReason::Duplicate)));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn c10_obsolete_proof_only_from_apply() {
    let d = basic(DecisionOutcome::Obsolete);
    assert_passthrough(script(d, AnalysisResult::Skipped(AnalysisSkipReason::Obsolete)));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn c11_verified_applied_result_preserved() {
    let d = mutation(OperationKind::Rename, OperationDirection::Upload);
    let item = d.context.association.items()[0].item.remote.as_ref().unwrap().clone();
    let mut item = item;
    item.name = drivesync_engine::models::MetadataValue::Known("new.txt".into());
    let result = drivesync_engine::operations::ItemResult { resource: drivesync_engine::operations::ResourceRef::Remote { provider_id: item.provider.clone(), reference: item.reference.clone() }, metadata: Some(drivesync_engine::models::FileData::Remote(item)), verification: drivesync_engine::operations::VerificationStatus::Verified };
    let r = completed(&d, ApplyOutcome::Applied { operations: NonEmpty::new(drivesync_engine::analyzer::apply::AppliedOperation { id: operation_id(), result }, vec![]) });
    assert_passthrough(script(d, r));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn d01_mixed_targets_preserved() {
    let d = independent();
    let r = AnalysisResult::Completed(vec![AnalysisItemResult { target: d.items[0].target.clone(), outcome: ApplyOutcome::ConflictRecorded }, AnalysisItemResult { target: d.items[1].target.clone(), outcome: ApplyOutcome::PendingExecution { operations: NonEmpty::new(operation_id(), vec![]) } }]);
    assert_passthrough(script(d, r));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn d02_missing_result_target_rejected() {
    let mut s = no_action();
    s.result = Ok(AnalysisResult::Completed(vec![]));
    let trace = full_trace(&s);
    let (r, calls) = run(s);
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidResult)));
    assert_eq!(calls, trace);
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn d03_foreign_result_target_rejected() {
    let mut s = no_action();
    if let Ok(AnalysisResult::Completed(items)) = &mut s.result { items[0].target = DecisionTarget::Synchronization;
    } let (r, _) = run(s);
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidResult)));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn d04_wrong_result_revision_rejected() {
    let mut s = no_action();
    if let Ok(AnalysisResult::Completed(items)) = &mut s.result { if let DecisionTarget::Item { revision, .. } = &mut items[0].target { *revision += 1;
    } } let (r, _) = run(s);
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidResult)));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn d05_duplicate_result_target_rejected() {
    let mut s = no_action();
    if let Ok(AnalysisResult::Completed(items)) = &mut s.result { items.push(items[0].clone());
    } let (r, _) = run(s);
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidResult)));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn d06_result_order_must_match_decision() {
    let d = independent();
    let r = AnalysisResult::Completed(vec![AnalysisItemResult { target: d.items[1].target.clone(), outcome: ApplyOutcome::PendingExecution { operations: NonEmpty::new(operation_id(), vec![]) } }, AnalysisItemResult { target: d.items[0].target.clone(), outcome: ApplyOutcome::ConflictRecorded }]);
    let (r, _) = run(script(d, r));
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidResult)));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn d07_nonobsolete_decision_cannot_be_skipped_as_obsolete() {
    let d = basic(DecisionOutcome::NoAction);
    let (r, _) = run(script(d, AnalysisResult::Skipped(AnalysisSkipReason::Obsolete)));
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidResult)));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn d08_conflict_cannot_be_pending_execution() {
    let d = basic(DecisionOutcome::Conflict { requirements: vec![] });
    let r = completed(&d, ApplyOutcome::PendingExecution { operations: NonEmpty::new(operation_id(), vec![]) });
    let (r, _) = run(script(d, r));
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidResult)));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn d09_evidence_request_cannot_claim_baseline() {
    let d = new_observation();
    let r = completed(&d, ApplyOutcome::BaselineEstablished);
    let (r, _) = run(script(d, r));
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidResult)));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn e01_feedback_uses_only_entered_stages() {
    let (r, calls) = run(no_action());
    assert!(r.is_ok());
    assert_eq!(phases(&calls), vec![AnalysisStage::Inspecting, AnalysisStage::Reconciling, AnalysisStage::Applying]);
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn e02_no_duplicate_started_feedback() {
    let (r, calls) = run(no_action());
    assert!(r.is_ok());
    assert!(!calls.iter().any(|c| matches!(c, Call::Feedback(AnalysisUpdate { status: AnalysisStatus::Started, .. }))));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn e03_feedback_retains_original_file() {
    let s = no_action();
    let file = s.file.clone();
    let (r, calls) = run(s);
    assert!(r.is_ok());
    for c in calls { if let Call::Feedback(update) = c { assert_eq!(update.file, file);
    } }
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn e04_failure_does_not_report_later_phase() {
    let mut s = no_action();
    s.context = Err(error(AnalysisErrorKind::Inventory));
    let (_, calls) = run(s);
    assert_eq!(phases(&calls), vec![AnalysisStage::Inspecting]);
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn e05_terminal_result_does_not_fabricate_verifying_progress() {
    let (r, calls) = run(action_script(ApplyOutcome::PendingExecution { operations: NonEmpty::new(operation_id(), vec![]) }));
    assert!(r.is_ok());
    assert!(!phases(&calls).contains(&AnalysisStage::Verifying));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn e06_multiple_deliveries_are_not_cached_or_skipped_by_facade() {
    let s = no_action();
    let log = Arc::new(Mutex::new(vec![]));
    let engine = AnalyzerEngine::new(ContextPort { log: log.clone(), result: s.context.clone() }, DecisionPort { log: log.clone(), result: s.decision.clone() }, ApplyPort { log: log.clone(), result: s.result.clone() });
    let feedback = Feedback(log.clone());
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    assert_eq!(rt.block_on(engine.analyze(s.file.clone(), &feedback)), s.result);
    assert_eq!(rt.block_on(engine.analyze(s.file.clone(), &feedback)), s.result);
    let expected = full_trace(&s);
    let actual = log.lock().unwrap();
    assert_eq!(actual.as_slice(), [expected.clone(), expected].concat().as_slice());
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn f01_foreign_decision_target_rejected() {
    let mut s = no_action();
    s.decision.as_mut().unwrap().items[0].target = DecisionTarget::Item { id: SyncItemId("foreign".into()), revision: 7 };
    let (r, calls) = run(s);
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidDecision)));
    assert!(!calls.iter().any(|c| matches!(c, Call::Apply(_))));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn f02_scope_missing_decision_target_rejected() {
    let d = independent();
    let r = AnalysisResult::Completed(vec![]);
    let mut s = script(d, r);
    s.decision.as_mut().unwrap().items.pop();
    let (r, calls) = run(s);
    assert_eq!(r, Err(error(AnalysisErrorKind::InvalidDecision)));
    assert!(!calls.iter().any(|c| matches!(c, Call::Apply(_))));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn f03_pending_reanalysis_preserves_confirmed_partial_effect() {
    let d = multi();
    let mut m = d.context.association.items()[0].item.remote.as_ref().unwrap().clone();
    m.size = drivesync_engine::models::MetadataValue::Known(31);
    let result = drivesync_engine::operations::ItemResult { resource: drivesync_engine::operations::ResourceRef::Remote { provider_id: m.provider.clone(), reference: m.reference.clone() }, metadata: Some(drivesync_engine::models::FileData::Remote(m)), verification: drivesync_engine::operations::VerificationStatus::Verified };
    let r = completed(&d, ApplyOutcome::Interrupted { completed: vec![drivesync_engine::analyzer::apply::AppliedOperation { id: operation_id(), result }], reason: InterruptionReason::SourceChanged });
    assert_passthrough(script(d, r));
}

#[test]
#[ignore = "TDD red: AnalyzerEngine implementation pending"]
fn f04_stale_no_action_can_request_reanalysis() {
    let d = basic(DecisionOutcome::NoAction);
    let r = completed(&d, ApplyOutcome::Interrupted { completed: vec![], reason: InterruptionReason::StaleRevision });
    assert_passthrough(script(d, r));
}
