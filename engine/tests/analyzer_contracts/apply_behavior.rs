//! Behavioral RED tests for ApplyEngine, using scripted persistence and Operator.
use super::{apply_fixtures::*, apply_mocks::*};
use drivesync_engine::{
    analyzer::{AnalysisDecision, AnalysisErrorKind, AnalysisItemResult, AnalysisResult, AnalysisSkipReason, ApplyOutcome, ContextAssociation, DecisionBlock, DecisionOutcome, NonEmpty},
    analyzer::apply::{ApplyBlock, DeliveryState, OperationReceipt, OperationState, RecoveryEvidence, ReanalysisReason, TargetReceipt},
    inventory::{AnalysisIntent, AnalysisRequirement::*, ItemKind, OperationDirection::{Upload, Download}, OperationKind::{Create, UpdateContent, Rename, Move, Trash, Delete}},
    operations::*,
};
fn execs(s: &Shared) -> Vec<(ExecutionId, OperationCommand)> {
    calls(s).into_iter().filter_map(|c| match c { Call::Execute(id, cmd) => Some((id, cmd)), _ => None }).collect()
}
fn no_effect(s: &Shared) { assert!(execs(s).is_empty()); }
fn no_confirm(s: &Shared) { assert!(!calls(s).iter().any(|c| matches!(c, Call::Confirm(..)))); }
fn finished(s: &Shared) { assert!(matches!(calls(s).last(), Some(Call::Finish(_)))); }
fn rejected(d: AnalysisDecision, s: &Shared) {
    assert_eq!(run(d, s).unwrap_err().kind, AnalysisErrorKind::InvalidDecision);
    no_effect(s);
    assert!(!calls(s).iter().any(|c| matches!(c, Call::Begin(_) | Call::Reserve(_) | Call::Record(_) | Call::Finish(_))));
}
fn simple(d: AnalysisDecision, expected: ApplyOutcome) {
    let s = setup(&d);
    assert_eq!(outcome(run(d, &s).unwrap()), expected);
    no_effect(&s);
    finished(&s);
}
fn planned(d: AnalysisDecision, check: impl FnOnce(&OperationCommand) -> bool) {
    let s = setup(&d);
    let _ = run(d, &s).unwrap();
    let reserved = calls(&s).into_iter().find_map(|c| match c { Call::Reserve(r) => Some(r), _ => None }).expect("expected reserved operation");
    assert!(check(&reserved.intent.command));
    assert!(reserved.intent.validate_source());
}
fn pending(d: AnalysisDecision, fault: OperatorFailure, recovery: bool) {
    let s = setup(&d);
    s.lock().unwrap().effect = Err(fault);
    let out = outcome(run(d, &s).unwrap());
    if recovery { assert!(matches!(out, ApplyOutcome::PendingRecovery { .. })); }
    else { assert!(matches!(out, ApplyOutcome::PendingExecution { .. })); }
    assert_eq!(execs(&s).len(), 1);
    no_confirm(&s);
    finished(&s);
}
fn saved(d: &AnalysisDecision, state: OperationState, attempts: u32) -> OperationReceipt {
    let item = &d.context.association.items()[0].item;
    let local = item.local.as_ref().unwrap().clone();
    let remote = item.remote.as_ref().unwrap().clone();
    let id = ExecutionId::new("original-execution".into()).unwrap();
    OperationReceipt {
        intent: ExecutionIntent {
            context: ExecutionContext { id, source: Some(SourceEvidence::Local { metadata: Box::new(local.clone()), sha256: None }) },
            scope: snapshot(d).scope,
            command: OperationCommand::UpdateContent {
                target: ResourceRef::Remote { provider_id: remote.provider.clone(), reference: remote.reference.clone() },
                source: ContentSource::LocalFile { relative_path: local.relative_path, local_id: local.local_id },
                preconditions: Preconditions { expected_version: Some("7".into()), expected_revision: None },
            },
        },
        state, attempts,
    }
}
fn resume(s: &Shared, d: &AnalysisDecision, receipt: OperationReceipt) {
    let mut state = s.lock().unwrap();
    state.delivery = DeliveryState::InProgress(vec![TargetReceipt { target: d.items[0].target.clone(), outcome: None, operations: vec![receipt.clone()] }]);
    state.reserved = Some(receipt);
}
fn apply_fixtures_result(local: bool) -> ItemResult { applied_result(local, true) }
fn existing_remote_result(d: &AnalysisDecision, name: Option<&str>, parent: Option<&str>) -> ItemResult {
    let mut m = d.context.association.items().iter().last().unwrap().item.remote.as_ref().unwrap().clone();
    m.version = drivesync_engine::models::MetadataValue::Known("10".into());
    if let Some(name) = name { m.name = drivesync_engine::models::MetadataValue::Known(name.into()); }
    if let Some(parent) = parent { m.parent_ids = drivesync_engine::models::MetadataValue::Known(vec![drivesync_engine::models::RemoteFileId(parent.into())]); }
    ItemResult { resource: ResourceRef::Remote { provider_id: m.provider.clone(), reference: m.reference.clone() }, metadata: Some(drivesync_engine::models::FileData::Remote(m)), verification: VerificationStatus::Verified }
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn a01_valid_no_action() {
    simple(basic(DecisionOutcome::NoAction), ApplyOutcome::NoAction);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn a02_duplicate_targets_rejected() {
    let mut d = basic(DecisionOutcome::NoAction);
    d.items.push(d.items[0].clone());
    let s = setup(&d);
    rejected(d, &s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn a03_foreign_target_rejected() {
    let mut d = basic(DecisionOutcome::NoAction);
    d.items[0].target = drivesync_engine::analyzer::DecisionTarget::Synchronization;
    let s = setup(&d);
    rejected(d, &s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn a04_stale_revision_reanalysis() {
    let d = mutation(Rename, Upload);
    let s = setup(&d);
    s.lock().unwrap().snapshot.current_item.as_mut().unwrap().revision += 1;
    let out = outcome(run(d, &s).unwrap());
    assert!(matches!(out, ApplyOutcome::PendingReanalysis { reason: ReanalysisReason::StaleRevision, .. }));
    no_effect(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn a05_empty_decision_rejected() {
    let mut d = basic(DecisionOutcome::NoAction);
    d.items.clear();
    let s = setup(&d);
    rejected(d, &s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn a06_ambiguous_action_rejected() {
    let mut d = new_observation();
    d.context.association = ContextAssociation::Ambiguous(vec![super::decision_fixtures::known_subject("1")]);
    d.items[0].outcome = action(Upload, Create);
    let s = setup(&d);
    rejected(d, &s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn a07_wrong_sync_rejected() {
    let d = basic(DecisionOutcome::NoAction);
    let s = setup(&d);
    s.lock().unwrap().snapshot.scope.sync_id.0 = "other".into();
    assert!(run(d, &s).is_err());
    no_effect(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn a08_duplicate_intents_rejected() {
    let mut d = mutation(Rename, Upload);
    let i = AnalysisIntent { direction: Upload, kind: Rename };
    d.items[0].outcome = DecisionOutcome::ActionSelected { intents: NonEmpty::new(i.clone(), vec![i]) };
    let s = setup(&d);
    rejected(d, &s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn b01_no_action_records() {
    let d = basic(DecisionOutcome::NoAction);
    let s = setup(&d);
    assert_eq!(outcome(run(d, &s).unwrap()), ApplyOutcome::NoAction);
    assert!(calls(&s).iter().any(|c| matches!(c, Call::Record(_))));
    no_effect(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn b02_baseline_requires_verification() {
    let d = basic(DecisionOutcome::EstablishBaseline);
    let s = setup(&d);
    assert_eq!(outcome(run(d, &s).unwrap()), ApplyOutcome::BaselineEstablished);
    let log = calls(&s);
    assert!(log.iter().filter(|c| matches!(c, Call::VerifySource(_))).count() >= 2);
    assert!(log.iter().any(|c| matches!(c, Call::Confirm(..))));
    no_effect(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn b03_conflict_records_without_effect() {
    simple(basic(DecisionOutcome::Conflict { requirements: vec![] }), ApplyOutcome::ConflictRecorded);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn b04_evidence_request_persisted() {
    simple(basic(need(ObserveRemote)), ApplyOutcome::EvidenceRequested { requirements: NonEmpty::new(ObserveRemote, vec![]) });
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn b05_unsupported_kind_blocked() {
    simple(basic(DecisionOutcome::Blocked { reason: DecisionBlock::UnsupportedKind }), ApplyOutcome::Blocked { reason: ApplyBlock::UnsupportedKind });
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn b06_obsolete_never_mutates() {
    let mut d = basic(DecisionOutcome::Obsolete);
    let s = setup(&d);
    let result = run(d, &s).unwrap();
    assert_eq!(result, AnalysisResult::Skipped(AnalysisSkipReason::Obsolete));
    no_effect(&s);
    no_confirm(&s);
    finished(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn b07_authorization_request_persisted() {
    simple(basic(need(AuthorizeDeletion)), ApplyOutcome::EvidenceRequested { requirements: NonEmpty::new(AuthorizeDeletion, vec![]) });
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn b08_new_observation_survives_pending_evidence() {
    let d = new_observation();
    let file = d.context.file.clone();
    let s = setup(&d);
    assert!(matches!(outcome(run(d, &s).unwrap()), ApplyOutcome::EvidenceRequested { .. }));
    assert!(calls(&s).iter().any(|c| matches!(c, Call::Begin(f) if f == &file)));
    no_effect(&s);
    finished(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn c01_upload_command() {
    planned(creation(true, ItemKind::File), |c| matches!(c, OperationCommand::Upload { source: ResourceRef::Local { .. }, destination: DirectoryRef::Remote { .. }, .. }));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn c02_download_command() {
    planned(creation(false, ItemKind::File), |c| matches!(c, OperationCommand::Download { source: ResourceRef::Remote { .. }, .. }));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn c03_remote_directory_command() {
    planned(creation(true, ItemKind::Directory), |c| matches!(c, OperationCommand::CreateDirectory { parent: DirectoryRef::Remote { .. }, .. }));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn c04_local_directory_command() {
    planned(creation(false, ItemKind::Directory), |c| matches!(c, OperationCommand::CreateDirectory { parent: DirectoryRef::LocalRoot | DirectoryRef::Local { .. }, .. }));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn c05_content_update_command() {
    planned(mutation(UpdateContent, Upload), |c| matches!(c, OperationCommand::UpdateContent { target: ResourceRef::Remote { .. }, source: ContentSource::LocalFile { .. }, .. }));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn c06_rename_preserves_identity() {
    planned(mutation(Rename, Upload), |c| matches!(c, OperationCommand::Rename { target: ResourceRef::Remote { reference, .. }, new_name: ItemName::Remote(name), .. } if reference.file_id.0 == "remote-1" && name == "new.txt"));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn c07_move_preserves_identity() {
    planned(mutation(Move, Upload), |c| matches!(c, OperationCommand::Move { target: ResourceRef::Remote { reference, .. }, destination: DirectoryRef::Remote { reference: parent, .. }, .. } if reference.file_id.0 == "remote-1" && parent.file_id.0 == "remote-b"));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn c08_authorized_trash_command() {
    planned(mutation(Trash, Upload), |c| matches!(c, OperationCommand::Trash { target: ResourceRef::Remote { .. }, .. }));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn c09_missing_parent_blocks_dispatch() {
    let d = creation(true, ItemKind::File);
    let s = setup(&d);
    s.lock().unwrap().snapshot.related_parents.clear();
    assert!(matches!(outcome(run(d, &s).unwrap()), ApplyOutcome::EvidenceRequested { .. }));
    no_effect(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn c10_occupied_destination_reanalysis() {
    let d = creation(true, ItemKind::File);
    let s = setup(&d);
    s.lock().unwrap().verification_error = Some(failure(OperatorError::DestinationOccupied, ExecutionEffect::NotApplied));
    assert!(matches!(outcome(run(d, &s).unwrap()), ApplyOutcome::PendingReanalysis { reason: ReanalysisReason::DestinationOccupied, .. }));
    no_effect(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn c11_missing_guard_blocks_mutation() {
    let mut d = mutation(Rename, Upload);
    if let ContextAssociation::Scope(items) = &mut d.context.association { for m in [&mut items[0].item.remote, &mut items[0].item.remote_latest].into_iter().flatten() { m.version = drivesync_engine::models::MetadataValue::NotCollected;
    } } let s = setup(&d);
    assert!(matches!(outcome(run(d, &s).unwrap()), ApplyOutcome::EvidenceRequested { .. }));
    no_effect(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn c12_permanent_delete_rejected() {
    let mut d = mutation(Trash, Upload);
    d.items[0].outcome = action(Upload, Delete);
    let s = setup(&d);
    rejected(d, &s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn d01_reservation_and_claim_precede_execution() {
    let d = creation(true, ItemKind::File);
    let s = setup(&d);
    let _ = run(d, &s).unwrap();
    let log = calls(&s);
    let r = log.iter().position(|c| matches!(c, Call::Reserve(_))).unwrap();
    let cl = log.iter().position(|c| matches!(c, Call::Claim(_))).unwrap();
    let e = log.iter().position(|c| matches!(c, Call::Execute(..))).unwrap();
    assert!(r < cl && cl < e);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn d02_upload_result_is_recorded() {
    let d = creation(true, ItemKind::File);
    let s = setup(&d);
    let result = apply_fixtures_result(false);
    s.lock().unwrap().effect = Ok(result.clone());
    let _ = run(d, &s).unwrap();
    assert!(calls(&s).iter().any(|c| matches!(c, Call::Checkpoint(r) if r.state == OperationState::Verified(result.clone()))));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn d03_download_result_is_recorded() {
    let d = creation(false, ItemKind::File);
    let s = setup(&d);
    let result = apply_fixtures_result(true);
    s.lock().unwrap().effect = Ok(result.clone());
    let _ = run(d, &s).unwrap();
    assert!(calls(&s).iter().any(|c| matches!(c, Call::Checkpoint(r) if r.state == OperationState::Verified(result.clone()))));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn d04_rename_uses_observed_result() {
    let d = mutation(Rename, Upload);
    let s = setup(&d);
    let result = existing_remote_result(&d, Some("new.txt"), None);
    s.lock().unwrap().effect = Ok(result.clone());
    let _ = run(d, &s).unwrap();
    assert!(calls(&s).iter().any(|c| matches!(c, Call::Checkpoint(r) if r.state == OperationState::Verified(result.clone()))));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn d05_move_uses_observed_parent() {
    let d = mutation(Move, Upload);
    let s = setup(&d);
    let result = existing_remote_result(&d, None, Some("remote-b"));
    s.lock().unwrap().effect = Ok(result.clone());
    let _ = run(d, &s).unwrap();
    assert!(calls(&s).iter().any(|c| matches!(c, Call::Checkpoint(r) if r.state == OperationState::Verified(result.clone()))));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn d06_new_identity_is_not_old_identity() {
    let d = creation(true, ItemKind::File);
    let s = setup(&d);
    let result = apply_fixtures_result(false);
    s.lock().unwrap().effect = Ok(result.clone());
    let _ = run(d, &s).unwrap();
    assert!(calls(&s).iter().any(|c| matches!(c, Call::Checkpoint(r) if r.state == OperationState::Verified(result.clone()))));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn d07_unverified_result_requires_recovery() {
    let d = creation(true, ItemKind::File);
    let s = setup(&d);
    s.lock().unwrap().effect = Ok(applied_result(false, false));
    assert!(matches!(outcome(run(d, &s).unwrap()), ApplyOutcome::PendingRecovery { .. }));
    no_confirm(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn d08_integrity_unavailable_prevents_baseline() {
    let d = creation(true, ItemKind::File);
    let s = setup(&d);
    s.lock().unwrap().integrity = IntegrityStatus::NotAvailable;
    let out = outcome(run(d, &s).unwrap());
    assert!(!matches!(out, ApplyOutcome::Applied { .. } | ApplyOutcome::BaselineEstablished));
    no_confirm(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn d09_changed_source_reanalysis() {
    let d = creation(true, ItemKind::File);
    let s = setup(&d);
    s.lock().unwrap().verification_error = Some(failure(OperatorError::PreconditionFailed, ExecutionEffect::NotApplied));
    assert!(matches!(outcome(run(d, &s).unwrap()), ApplyOutcome::PendingReanalysis { reason: ReanalysisReason::SourceChanged, .. }));
    no_effect(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn d10_converged_baseline_confirmed() {
    let d = basic(DecisionOutcome::EstablishBaseline);
    let s = setup(&d);
    let out = outcome(run(d, &s).unwrap());
    assert_eq!(out, ApplyOutcome::BaselineEstablished);
    assert!(calls(&s).iter().any(|c| matches!(c, Call::Confirm(..))));
    finished(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn e01_failed_reservation_no_execution() {
    let d = creation(true, ItemKind::File);
    let s = setup(&d);
    s.lock().unwrap().fault = Some(Fault::Reserve);
    assert!(run(d, &s).is_err());
    no_effect(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn e02_network_no_effect_queues_retry() {
    pending(creation(true, ItemKind::File), failure(OperatorError::Network, ExecutionEffect::NotApplied), false);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn e03_failed_applied_effect_requires_recovery() {
    pending(creation(true, ItemKind::File), failure(OperatorError::Network, ExecutionEffect::Applied), true);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn e04_unknown_effect_requires_recovery() {
    pending(creation(true, ItemKind::File), failure(OperatorError::IndeterminateEffect, ExecutionEffect::Unknown), true);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn e05_cancelled_before_effect_remains_pending() {
    pending(creation(true, ItemKind::File), failure(OperatorError::Cancelled, ExecutionEffect::NotApplied), false);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn e06_cancelled_uncertain_effect_requires_recovery() {
    pending(creation(true, ItemKind::File), failure(OperatorError::Cancelled, ExecutionEffect::Unknown), true);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn e07_checkpoint_failure_does_not_ack() {
    let d = creation(true, ItemKind::File);
    let s = setup(&d);
    s.lock().unwrap().fail_after_execute = true;
    assert!(run(d, &s).is_err());
    assert_eq!(execs(&s).len(), 1);
    assert!(!calls(&s).iter().any(|c| matches!(c, Call::Finish(_))));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn e08_confirmation_failure_does_not_ack() {
    let d = basic(DecisionOutcome::EstablishBaseline);
    let s = setup(&d);
    s.lock().unwrap().fault = Some(Fault::Confirm);
    assert!(run(d, &s).is_err());
    no_effect(&s);
    assert!(!calls(&s).iter().any(|c| matches!(c, Call::Finish(_))));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn e09_permission_denied_blocked() {
    let d = creation(true, ItemKind::File);
    let s = setup(&d);
    s.lock().unwrap().effect = Err(failure(OperatorError::PermissionDenied, ExecutionEffect::NotApplied));
    assert_eq!(outcome(run(d, &s).unwrap()), ApplyOutcome::Blocked { reason: ApplyBlock::PermissionDenied });
    no_confirm(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn e10_unsupported_operation_not_retried() {
    let d = creation(true, ItemKind::File);
    let s = setup(&d);
    s.lock().unwrap().effect = Err(failure(OperatorError::UnsupportedOperation, ExecutionEffect::NotApplied));
    assert_eq!(outcome(run(d, &s).unwrap()), ApplyOutcome::Blocked { reason: ApplyBlock::UnsupportedOperation });
    assert_eq!(execs(&s).len(), 1);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn f01_completed_delivery_skipped() {
    let d = basic(DecisionOutcome::NoAction);
    let s = setup(&d);
    s.lock().unwrap().delivery = DeliveryState::Completed(AnalysisResult::Completed(vec![AnalysisItemResult { target: d.items[0].target.clone(), outcome: ApplyOutcome::NoAction }]));
    assert_eq!(run(d, &s).unwrap(), AnalysisResult::Skipped(AnalysisSkipReason::Duplicate));
    no_effect(&s);
    assert_eq!(calls(&s), vec![Call::Delivery]);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn f02_partial_delivery_resumes() {
    let d = mutation(UpdateContent, Upload);
    let s = setup(&d);
    let receipt = saved(&d, OperationState::Reserved, 0);
    let id = receipt.intent.context.id.clone();
    resume(&s, &d, receipt);
    s.lock().unwrap().effect = Ok(existing_remote_result(&d, None, None));
    let _ = run(d, &s).unwrap();
    assert_eq!(execs(&s)[0].0, id);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn f03_retry_preserves_execution_id() {
    let d = mutation(UpdateContent, Upload);
    let s = setup(&d);
    let receipt = saved(&d, OperationState::RetryPending(failure(OperatorError::Network, ExecutionEffect::NotApplied)), 1);
    let id = receipt.intent.context.id.clone();
    resume(&s, &d, receipt);
    s.lock().unwrap().effect = Ok(existing_remote_result(&d, None, None));
    let _ = run(d, &s).unwrap();
    assert_eq!(execs(&s)[0].0, id);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn f04_execution_identity_mismatch_rejected() {
    let d = mutation(UpdateContent, Upload);
    let s = setup(&d);
    let mut receipt = saved(&d, OperationState::Reserved, 0);
    receipt.intent.command = OperationCommand::CreateDirectory { parent: DirectoryRef::LocalRoot, name: ItemName::Local("unrelated".into()) };
    resume(&s, &d, receipt);
    assert!(run(d, &s).is_err());
    no_effect(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn f05_stale_claim_no_effect() {
    let d = mutation(UpdateContent, Upload);
    let s = setup(&d);
    s.lock().unwrap().fault = Some(Fault::Claim);
    assert!(run(d, &s).is_err());
    no_effect(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn f06_guard_claim_failure_no_duplicate() {
    let d = mutation(UpdateContent, Upload);
    let s = setup(&d);
    s.lock().unwrap().fault = Some(Fault::Claim);
    let _ = run(d.clone(), &s);
    let _ = run(d, &s);
    no_effect(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn f07_verified_checkpoint_not_executed_again() {
    let d = mutation(UpdateContent, Upload);
    let s = setup(&d);
    let receipt = saved(&d, OperationState::Verified(existing_remote_result(&d, None, None)), 1);
    resume(&s, &d, receipt);
    let _ = run(d, &s).unwrap();
    no_effect(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn f08_new_observation_invalidates_guard() {
    let d = mutation(UpdateContent, Upload);
    let s = setup(&d);
    s.lock().unwrap().snapshot.current_item.as_mut().unwrap().revision = 8;
    assert!(matches!(outcome(run(d, &s).unwrap()), ApplyOutcome::PendingReanalysis { .. }));
    no_effect(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn g01_multiple_intents_execute_first_only() {
    let d = multi();
    let s = setup(&d);
    s.lock().unwrap().effect = Ok(existing_remote_result(&d, None, None));
    let out = outcome(run(d, &s).unwrap());
    assert!(matches!(out, ApplyOutcome::PendingReanalysis { reason: ReanalysisReason::PriorStepCompleted, .. }));
    assert_eq!(execs(&s).len(), 1);
    assert!(matches!(execs(&s)[0].1, OperationCommand::UpdateContent { .. }));
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn g02_failed_first_never_dispatches_dependent() {
    pending(multi(), failure(OperatorError::Network, ExecutionEffect::NotApplied), false);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn g03_partial_completion_is_preserved() {
    let d = multi();
    let s = setup(&d);
    let result = existing_remote_result(&d, None, None);
    s.lock().unwrap().effect = Ok(result.clone());
    let out = outcome(run(d, &s).unwrap());
    assert!(matches!(out, ApplyOutcome::PendingReanalysis { completed, .. } if completed.len() == 1 && completed[0].result == result));
    no_confirm(&s);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn g04_prior_step_requires_renewed_analysis() {
    let d = multi();
    let s = setup(&d);
    s.lock().unwrap().effect = Ok(existing_remote_result(&d, None, None));
    assert!(matches!(outcome(run(d, &s).unwrap()), ApplyOutcome::PendingReanalysis { reason: ReanalysisReason::PriorStepCompleted, .. }));
    assert_eq!(execs(&s).len(), 1);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn g05_independent_targets_return_separately() {
    let d = independent();
    let s = setup(&d);
    let result = run(d.clone(), &s).unwrap();
    let AnalysisResult::Completed(items) = result else { panic!("expected targets") };
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].target, d.items[0].target);
    assert_eq!(items[1].target, d.items[1].target);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn g06_conflict_target_never_executes() {
    let d = independent();
    let s = setup(&d);
    let _ = run(d, &s).unwrap();
    assert!(execs(&s).iter().all(|(_, c)| matches!(c, OperationCommand::UpdateContent { target: ResourceRef::Remote { reference, .. }, .. } if reference.file_id.0 == "remote-b")));
    assert_eq!(execs(&s).len(), 1);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn g07_mixed_result_preserves_conflict() {
    let d = independent();
    let s = setup(&d);
    let AnalysisResult::Completed(items) = run(d, &s).unwrap() else { panic!("expected mixed outcomes") };
    assert_eq!(items[0].outcome, ApplyOutcome::ConflictRecorded);
    assert_ne!(items[1].outcome, ApplyOutcome::ConflictRecorded);
}

#[test]
#[ignore = "TDD red: ApplyEngine implementation pending"]
fn g08_finish_failure_requires_redelivery() {
    let d = basic(DecisionOutcome::NoAction);
    let s = setup(&d);
    s.lock().unwrap().fault = Some(Fault::Finish);
    assert!(run(d, &s).is_err());
    no_effect(&s);
}
