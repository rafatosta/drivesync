//! Behavioral RED specification. Every case calls the concrete pure DecisionEngine.
use super::decision_fixtures::*;
use drivesync_engine::{
    analyzer::{
        decision::{DecisionEngine, DecisionPolicy, DeletionPolicy},
        AnalysisContext, AnalysisDecision, AnalysisErrorKind, ContextAssociation, DecisionBlock,
        DecisionOutcome, DecisionService, DecisionTarget, NonEmpty, ObservationFreshness,
    },
    inventory::{
        AnalysisDimension, AnalysisFindingStatus, AnalysisIntent, AnalysisReason, AnalysisRequirement,
        EvidenceRelation::{Equal, Different, Unknown}, ItemKind, OperationDirection::{Upload, Download},
        OperationKind::{Create, UpdateContent, Rename, Move, Trash}, PresenceState, SyncItemId,
    },
    models::{FileEventAction, MetadataValue, ObservationCoverage},
};
use AnalysisRequirement::*;

fn allowed() -> DecisionPolicy {
    DecisionPolicy { local_deletion: DeletionPolicy::AuthorizePropagation, remote_deletion: DeletionPolicy::AuthorizePropagation }
}
fn act(direction: drivesync_engine::inventory::OperationDirection, kind: drivesync_engine::inventory::OperationKind) -> AnalysisIntent {
    AnalysisIntent { direction, kind }
}
fn actions(values: Vec<AnalysisIntent>) -> DecisionOutcome {
    DecisionOutcome::ActionSelected { intents: NonEmpty::from_vec(values).unwrap() }
}
fn need(values: Vec<AnalysisRequirement>) -> DecisionOutcome {
    DecisionOutcome::NeedsEvidence { requirements: NonEmpty::from_vec(values).unwrap() }
}
fn conflict() -> DecisionOutcome {
    DecisionOutcome::Conflict { requirements: vec![] }
}
fn run(input: AnalysisContext, policy: DecisionPolicy) -> AnalysisDecision {
    input.file.validate().expect("decision fixture must contain a valid File");
    let out = DecisionEngine::new(policy).decide(input.clone()).unwrap();
    assert_eq!(out.context, input);
    out
}
fn expect(input: AnalysisContext, outcome: DecisionOutcome) -> AnalysisDecision {
    expect_policy(input, outcome, DecisionPolicy::default())
}
fn expect_policy(input: AnalysisContext, outcome: DecisionOutcome, policy: DecisionPolicy) -> AnalysisDecision {
    let target = match &input.association {
        ContextAssociation::Matched(item) => DecisionTarget::Item { id: item.item.id.clone(), revision: item.item.revision },
        ContextAssociation::Unmatched | ContextAssociation::Ambiguous(_) => DecisionTarget::Observation(input.file.id.clone()),
        ContextAssociation::Scope(items) if items.is_empty() => DecisionTarget::Synchronization,
        ContextAssociation::Scope(items) if items.len() == 1 => DecisionTarget::Item { id: items[0].item.id.clone(), revision: items[0].item.revision },
        _ => panic!("use the multi-target assertion for scope"),
    };
    let out = run(input, policy);
    assert_eq!(out.items.len(), 1);
    assert_eq!(out.items[0].target, target);
    assert_eq!(out.items[0].outcome, outcome);
    out
}
fn finding(out: &AnalysisDecision, dimension: AnalysisDimension, status: AnalysisFindingStatus, reason: AnalysisReason) {
    assert!(out.items[0].findings.iter().any(|f| f.dimension == dimension && f.status == status && f.reason == reason));
}
fn subject() -> drivesync_engine::analyzer::AnalysisItemContext {
    known_subject("1")
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn a01_matched_targets_exact_revision() {
    expect(matched(subject(), ObservationFreshness::Equal), DecisionOutcome::NoAction);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn a02_ambiguous_needs_identity() {
    let mut input = unmatched(true);
    input.association = ContextAssociation::Ambiguous(vec![subject()]);
    expect(input, need(vec![VerifyIdentity]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn a03_unmatched_targets_original_observation() {
    expect(unmatched(true), need(vec![ObserveRemote]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn a04_older_is_obsolete() {
    expect(matched(subject(), ObservationFreshness::Older), DecisionOutcome::Obsolete);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn a05_incomparable_requires_reobservation() {
    expect(matched(subject(), ObservationFreshness::Incomparable), need(vec![ObserveLocal]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn a06_empty_scope_needs_both_sides() {
    expect(scope(vec![]), need(vec![ObserveLocal, ObserveRemote]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn a07_scope_targets_sorted_and_unique() {
    let out = run(scope(vec![known_subject("b"), known_subject("a")]), DecisionPolicy::default());
    assert_eq!(out.items.len(), 2);
    for (entry, id) in out.items.iter().zip(["a", "b"]) { assert_eq!(entry.target, DecisionTarget::Item { id: SyncItemId(id.into()), revision: 7 });
    assert_eq!(entry.outcome, DecisionOutcome::NoAction);
    }
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn a08_invalid_file_rejected() {
    let mut input = unmatched(true);
    input.file.id.0.clear();
    let err = DecisionEngine::new(DecisionPolicy::default()).decide(input).unwrap_err();
    assert_eq!(err.kind, AnalysisErrorKind::InvalidContext);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn b01_local_only_verified_destination_creates_remote() {
    expect(scope(vec![one_sided(true, ItemKind::File)]), actions(vec![act(Upload, Create)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn b02_remote_only_verified_destination_creates_local() {
    expect(scope(vec![one_sided(false, ItemKind::File)]), actions(vec![act(Download, Create)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn b03_equivalent_without_baseline_establishes_it() {
    let mut s = subject();
    clear_baseline(&mut s);
    expect(scope(vec![s]), DecisionOutcome::EstablishBaseline);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn b04_linked_unequal_without_baseline_conflicts() {
    let mut s = subject();
    clear_baseline(&mut s);
    content(&mut s, Unknown, Unknown, Different);
    expect(scope(vec![s]), conflict());
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn b05_local_presence_unknown() {
    let mut s = one_sided(false, ItemKind::File);
    s.item.local_presence = PresenceState::Unknown;
    expect(scope(vec![s]), need(vec![ObserveLocal]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn b06_remote_presence_unknown() {
    let mut s = one_sided(true, ItemKind::File);
    s.item.remote_presence = PresenceState::Unknown;
    expect(scope(vec![s]), need(vec![ObserveRemote]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn b07_same_name_does_not_establish_identity() {
    let mut input = unmatched(false);
    input.association = ContextAssociation::Ambiguous(vec![known_subject("other")]);
    expect(input, need(vec![VerifyIdentity]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn b08_new_local_directory_creates_remote_directory() {
    expect(scope(vec![one_sided(true, ItemKind::Directory)]), actions(vec![act(Upload, Create)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn b09_new_remote_directory_creates_local_directory() {
    expect(scope(vec![one_sided(false, ItemKind::Directory)]), actions(vec![act(Download, Create)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn b10_unsupported_kind_is_blocked() {
    let mut s = subject();
    s.item.kind = ItemKind::Symlink;
    for m in [&mut s.item.local, &mut s.item.local_latest].into_iter().flatten() { m.kind = MetadataValue::Known(drivesync_engine::models::LocalEntryKind::Symlink);
    m.mode = MetadataValue::Known(drivesync_engine::models::LocalFileMode { raw_mode: 0o120777, permission_bits: 0o777 });
    m.symlink.target = MetadataValue::Known(b"target".to_vec());
    } expect(scope(vec![s]), DecisionOutcome::Blocked { reason: DecisionBlock::UnsupportedKind });
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn c01_unchanged_content() {
    expect(scope(vec![subject()]), DecisionOutcome::NoAction);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn c02_local_content_changed() {
    let mut s = subject();
    content(&mut s, Different, Equal, Different);
    let out = expect(scope(vec![s]), actions(vec![act(Upload, UpdateContent)]));
    finding(&out, AnalysisDimension::Content, AnalysisFindingStatus::Compatible, AnalysisReason::UnilateralChange);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn c03_remote_content_changed() {
    let mut s = subject();
    content(&mut s, Equal, Different, Different);
    expect(scope(vec![s]), actions(vec![act(Download, UpdateContent)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn c04_content_converged() {
    let mut s = subject();
    content(&mut s, Different, Different, Equal);
    expect(scope(vec![s]), DecisionOutcome::EstablishBaseline);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn c05_content_diverged() {
    let mut s = subject();
    content(&mut s, Different, Different, Different);
    let out = expect(scope(vec![s]), conflict());
    finding(&out, AnalysisDimension::Content, AnalysisFindingStatus::Divergent, AnalysisReason::ConcurrentChanges);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn c06_content_evidence_unknown() {
    let mut s = subject();
    content(&mut s, Unknown, Equal, Unknown);
    expect(scope(vec![s]), need(vec![VerifyContent]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn c07_equal_size_does_not_prove_equal_content() {
    let mut s = subject();
    content(&mut s, Unknown, Unknown, Unknown);
    assert_eq!(s.item.local.as_ref().unwrap().storage.size_bytes, s.item.remote.as_ref().unwrap().size);
    expect(scope(vec![s]), need(vec![VerifyContent]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn c08_remote_metadata_version_is_not_content_change() {
    let mut s = subject();
    rename(&mut s, None, Some("renamed.txt"));
    expect(scope(vec![s]), actions(vec![act(Download, Rename)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn d01_local_rename() {
    let mut s = subject();
    rename(&mut s, Some("new.txt"), None);
    expect(scope(vec![s]), actions(vec![act(Upload, Rename)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn d02_remote_rename() {
    let mut s = subject();
    rename(&mut s, None, Some("new.txt"));
    expect(scope(vec![s]), actions(vec![act(Download, Rename)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn d03_converged_rename() {
    let mut s = subject();
    rename(&mut s, Some("new.txt"), Some("new.txt"));
    expect(scope(vec![s]), DecisionOutcome::EstablishBaseline);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn d04_divergent_rename() {
    let mut s = subject();
    rename(&mut s, Some("local.txt"), Some("remote.txt"));
    let out = expect(scope(vec![s]), conflict());
    finding(&out, AnalysisDimension::Name, AnalysisFindingStatus::Divergent, AnalysisReason::ConcurrentChanges);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn d05_local_move() {
    let mut s = subject();
    move_to(&mut s, Some("parent-b"), None, Different);
    expect(scope(vec![s]), actions(vec![act(Upload, Move)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn d06_remote_move() {
    let mut s = subject();
    move_to(&mut s, None, Some("remote-b"), Different);
    expect(scope(vec![s]), actions(vec![act(Download, Move)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn d07_converged_move() {
    let mut s = subject();
    move_to(&mut s, Some("parent-b"), Some("remote-b"), Equal);
    expect(scope(vec![s]), DecisionOutcome::EstablishBaseline);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn d08_divergent_move() {
    let mut s = subject();
    move_to(&mut s, Some("parent-b"), Some("remote-c"), Different);
    expect(scope(vec![s]), conflict());
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn d09_unknown_parent_needs_evidence() {
    let mut s = subject();
    s.evidence.local_parent = None;
    s.evidence.current_parents = Unknown;
    expect(scope(vec![s]), need(vec![VerifyParent]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn d10_destination_candidate_does_not_authorize_mutation() {
    let mut input = unmatched(true);
    input.association = ContextAssociation::Ambiguous(vec![known_subject("1"), known_subject("2")]);
    expect(input, need(vec![VerifyIdentity]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn e01_authorized_local_absence_trashes_remote() {
    let mut s = subject();
    absent(&mut s, true);
    expect_policy(scope(vec![s]), actions(vec![act(Upload, Trash)]), allowed());
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn e02_authorized_remote_absence_trashes_local() {
    let mut s = subject();
    absent(&mut s, false);
    expect_policy(scope(vec![s]), actions(vec![act(Download, Trash)]), allowed());
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn e03_local_absence_remote_edit_conflicts() {
    let mut s = subject();
    content(&mut s, Equal, Different, Different);
    absent(&mut s, true);
    let out = expect_policy(scope(vec![s]), conflict(), allowed());
    finding(&out, AnalysisDimension::Presence, AnalysisFindingStatus::Divergent, AnalysisReason::DeleteVsChange);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn e04_remote_absence_local_edit_conflicts() {
    let mut s = subject();
    content(&mut s, Different, Equal, Different);
    absent(&mut s, false);
    expect_policy(scope(vec![s]), conflict(), allowed());
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn e05_both_absent_no_physical_action() {
    let mut s = subject();
    absent(&mut s, true);
    absent(&mut s, false);
    expect(scope(vec![s]), DecisionOutcome::NoAction);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn e06_deleted_label_without_absence() {
    expect_policy(deleted_signal(), need(vec![ObserveLocal]), allowed());
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn e07_removed_from_scope_without_absence() {
    expect_policy(remote_removal_signal(), need(vec![ObserveRemote]), allowed());
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn e08_unavailable_scope_cannot_delete() {
    let mut input = deleted_signal();
    input.file.event.action = FileEventAction::ScopeUnavailable;
    input.file.coverage = ObservationCoverage::Unavailable;
    expect_policy(input, need(vec![ObserveLocal]), allowed());
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn f01_local_content_remote_name() {
    let mut s = subject();
    content(&mut s, Different, Equal, Different);
    rename(&mut s, None, Some("remote.txt"));
    expect(scope(vec![s]), actions(vec![act(Upload, UpdateContent), act(Download, Rename)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn f02_local_name_remote_content() {
    let mut s = subject();
    content(&mut s, Equal, Different, Different);
    rename(&mut s, Some("local.txt"), None);
    expect(scope(vec![s]), actions(vec![act(Download, UpdateContent), act(Upload, Rename)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn f03_local_name_remote_location() {
    let mut s = subject();
    rename(&mut s, Some("new.txt"), None);
    move_to(&mut s, None, Some("remote-b"), Different);
    expect(scope(vec![s]), actions(vec![act(Upload, Rename), act(Download, Move)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn f04_local_content_and_name() {
    let mut s = subject();
    content(&mut s, Different, Equal, Different);
    rename(&mut s, Some("new.txt"), None);
    expect(scope(vec![s]), actions(vec![act(Upload, UpdateContent), act(Upload, Rename)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn f05_one_conflict_blocks_other_intents() {
    let mut s = subject();
    content(&mut s, Different, Equal, Different);
    rename(&mut s, Some("local.txt"), Some("remote.txt"));
    expect(scope(vec![s]), conflict());
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn f06_unknown_dimension_blocks_known_change() {
    let mut s = subject();
    rename(&mut s, Some("new.txt"), None);
    content(&mut s, Unknown, Equal, Unknown);
    expect(scope(vec![s]), need(vec![VerifyContent]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn f07_conflict_does_not_block_independent_subject() {
    let mut a = known_subject("a");
    rename(&mut a, Some("local.txt"), Some("remote.txt"));
    let mut b = known_subject("b");
    content(&mut b, Different, Equal, Different);
    let out = run(scope(vec![b, a]), DecisionPolicy::default());
    assert_eq!(out.items.len(), 2);
    assert_eq!(out.items[0].outcome, conflict());
    assert_eq!(out.items[1].outcome, actions(vec![act(Upload, UpdateContent)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn f08_identical_input_and_policy_are_deterministic() {
    let mut s = subject();
    rename(&mut s, Some("new.txt"), None);
    let input = scope(vec![s]);
    let first = run(input.clone(), DecisionPolicy::default());
    let second = run(input, DecisionPolicy::default());
    assert_eq!(first, second);
    assert_eq!(first.items[0].outcome, actions(vec![act(Upload, Rename)]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn g01_default_requires_remote_deletion_authorization() {
    let mut s = subject();
    absent(&mut s, true);
    expect(scope(vec![s]), need(vec![AuthorizeDeletion]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn g02_default_requires_local_deletion_authorization() {
    let mut s = subject();
    absent(&mut s, false);
    expect(scope(vec![s]), need(vec![AuthorizeDeletion]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn g03_unmatched_remote_does_not_imply_free_local_destination() {
    expect(unmatched(false), need(vec![ObserveLocal]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn g04_duplicate_targets_are_invalid() {
    let s = subject();
    let err = DecisionEngine::new(DecisionPolicy::default()).decide(scope(vec![s.clone(), s])).unwrap_err();
    assert_eq!(err.kind, AnalysisErrorKind::InvalidContext);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn g05_scope_association_mismatch_is_invalid() {
    let mut input = unmatched(true);
    input.association = ContextAssociation::Scope(vec![subject()]);
    let err = DecisionEngine::new(DecisionPolicy::default()).decide(input).unwrap_err();
    assert_eq!(err.kind, AnalysisErrorKind::InvalidContext);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn g06_contradictory_baseline_evidence_is_invalid() {
    let mut s = subject();
    s.item.baseline = None;
    let err = DecisionEngine::new(DecisionPolicy::default()).decide(scope(vec![s])).unwrap_err();
    assert_eq!(err.kind, AnalysisErrorKind::InvalidContext);
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn g07_new_local_payload_invalidates_old_content_measurement() {
    let mut input = matched(subject(), ObservationFreshness::Newer);
    if let Some(drivesync_engine::models::FileData::Local(m)) = input.file.data.as_mut() { m.storage.size_bytes = MetadataValue::Known(31);
    } expect(input, need(vec![VerifyContent]));
}

#[test]
#[ignore = "TDD red: DecisionEngine implementation pending"]
fn g08_deletion_with_unknown_survivor_cannot_execute() {
    let mut s = subject();
    absent(&mut s, true);
    s.evidence.remote_content = Unknown;
    expect_policy(scope(vec![s]), need(vec![VerifySurvivingState]), allowed());
}
