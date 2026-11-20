//! Behavioral TDD specification for ContextBuilder, independent of Decision/Apply.
use super::support::{file, item_context, unknown_evidence, unknown_observation};
use drivesync_engine::{
    analyzer::{
        context::{ContextBuilder, ContextCandidate, ContextInventory, ContextInventorySnapshot, RevisionEvidence},
        AnalysisContext, AnalysisError, AnalysisErrorKind, ContextAssociation, ContextService,
    },
    inventory::{BaselineSide, EvidenceRelation, PresenceState, SyncBaseline, SyncIdentity},
    models::{AccountId, File, FileEventAction, FileLocation, FileScope, LocalFileId, LocalPresenceEvidence,
        LocalRelativePath, ObservationCoverage, RemoteFileId, RemoteFileRef, SyncId, Timestamp},
};
use std::sync::{Arc, Mutex};

struct FakeInventory {
    snapshot: Result<ContextInventorySnapshot, AnalysisError>,
    reads: Arc<Mutex<Vec<File>>>,
}

impl ContextInventory for FakeInventory {
    fn read(&self, input: &File) -> Result<ContextInventorySnapshot, AnalysisError> {
        self.reads.lock().unwrap().push(input.clone());
        self.snapshot.clone()
    }
}

fn remote_ref(account: &str, id: &str) -> RemoteFileRef {
    RemoteFileRef { account_id: AccountId(account.into()), file_id: RemoteFileId(id.into()) }
}

fn remote_input() -> File {
    let mut input = file(FileEventAction::RecheckRequired, FileScope::Remote(remote_ref("account-1", "remote-1")));
    input.coverage = ObservationCoverage::Partial;
    input
}

fn local_input(action: FileEventAction) -> File {
    let path = LocalRelativePath::new(b"folder/file.txt".to_vec()).unwrap();
    let mut input = file(action, FileScope::Local(path.clone()));
    input.coverage = ObservationCoverage::Partial;
    if action == FileEventAction::Deleted {
        input.event.previous = Some(FileLocation::Local(path));
    } else {
        input.event.current = Some(FileLocation::Local(path));
        if matches!(action, FileEventAction::Renamed | FileEventAction::Moved) {
            input.event.previous = Some(FileLocation::Local(LocalRelativePath::new(b"old/file.txt".to_vec()).unwrap()));
        }
    }
    input
}

fn scope_input() -> File {
    file(FileEventAction::RecheckRequired, FileScope::WholeSync)
}

fn snapshot() -> ContextInventorySnapshot {
    ContextInventorySnapshot {
        identity: SyncIdentity { sync_id: SyncId("sync-1".into()), account_id: AccountId("account-1".into()), remote_root_id: "root".into() },
        observation: unknown_observation(),
        candidates: vec![],
    }
}

fn candidate(id: &str, exact_identity: bool) -> ContextCandidate {
    let mut subject = item_context(7);
    subject.item.id.0 = id.into();
    subject.item.remote_ref = Some(remote_ref("account-1", "remote-1"));
    subject.item.local_file_id = Some(LocalFileId("local-1".into()));
    ContextCandidate {
        item: subject.item,
        exact_identity,
        evidence: Some(RevisionEvidence { revision: 7, evidence: subject.evidence }),
    }
}

fn build(input: File, snapshot: Result<ContextInventorySnapshot, AnalysisError>) -> (Result<AnalysisContext, AnalysisError>, Vec<File>) {
    input.validate().expect("behavior fixture must be a valid detector File");
    let reads = Arc::new(Mutex::new(vec![]));
    let service = ContextBuilder::new(FakeInventory { snapshot, reads: reads.clone() });
    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let result = runtime.block_on(service.build(input));
    let calls = reads.lock().unwrap().clone();
    (result, calls)
}

fn result(input: File, snapshot: ContextInventorySnapshot) -> AnalysisContext {
    build(input, Ok(snapshot)).0.unwrap()
}

fn baseline() -> SyncBaseline {
    let side = BaselineSide {
        presence: PresenceState::Present, name: Some("file.txt".into()), local_name_bytes: None,
        parent_identity: None, version: None, checksum: None, size: None, modified_at: None,
    };
    SyncBaseline { local: side.clone(), remote: side, confirmed_at: Timestamp { unix_millis: 10 } }
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a01_remote_identity_matches() {
    let mut facts = snapshot();
    facts.candidates.push(candidate("known", true));
    let out = result(remote_input(), facts);
    assert_eq!(out.association, ContextAssociation::Matched);
    assert_eq!(out.items[0].item.id.0, "known");
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a02_local_identity_matches() {
    let mut facts = snapshot();
    facts.candidates.push(candidate("known", true));
    let out = result(local_input(FileEventAction::Modified), facts);
    assert_eq!(out.association, ContextAssociation::Matched);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a03_no_inventory_row_is_unmatched() {
    let out = result(remote_input(), snapshot());
    assert_eq!(out.association, ContextAssociation::Unmatched);
    assert!(out.items.is_empty());
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a04_name_alone_never_establishes_identity() {
    let mut facts = snapshot();
    let mut possible = candidate("other", false);
    possible.item.remote_ref = Some(remote_ref("account-1", "other"));
    facts.candidates.push(possible);
    let out = result(remote_input(), facts);
    assert_ne!(out.association, ContextAssociation::Matched);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a05_multiple_candidates_are_ambiguous() {
    let mut facts = snapshot();
    facts.candidates = vec![candidate("a", false), candidate("b", false)];
    let out = result(remote_input(), facts);
    assert_eq!(out.association, ContextAssociation::Ambiguous);
    assert_eq!(out.items.len(), 2);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a06_rename_retains_identity() {
    let mut facts = snapshot();
    facts.candidates.push(candidate("known", true));
    let out = result(local_input(FileEventAction::Renamed), facts);
    assert_eq!(out.association, ContextAssociation::Matched);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a07_move_retains_identity() {
    let mut facts = snapshot();
    facts.candidates.push(candidate("known", true));
    let out = result(local_input(FileEventAction::Moved), facts);
    assert_eq!(out.association, ContextAssociation::Matched);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a08_scope_contains_all_subjects() {
    let mut facts = snapshot();
    facts.candidates = vec![candidate("a", false), candidate("b", false)];
    let out = result(scope_input(), facts);
    assert_eq!(out.association, ContextAssociation::Scope);
    assert_eq!(out.items.len(), 2);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b01_preserve_both_known_states() {
    let mut facts = snapshot();
    let mut c = candidate("known", true);
    c.item.local_presence = PresenceState::Present;
    c.item.remote_presence = PresenceState::Present;
    let expected = c.item.clone();
    facts.candidates.push(c);
    assert_eq!(result(remote_input(), facts).items[0].item, expected);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b02_preserve_confirmed_baseline() {
    let mut facts = snapshot();
    let mut c = candidate("known", true);
    c.item.baseline = Some(baseline());
    facts.candidates.push(c);
    assert_eq!(result(remote_input(), facts).items[0].item.baseline, Some(baseline()));
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b03_missing_baseline_stays_missing() {
    let mut facts = snapshot();
    facts.candidates.push(candidate("known", true));
    assert!(result(remote_input(), facts).items[0].item.baseline.is_none());
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b04_unknown_local_presence_stays_unknown() {
    assert_eq!(result(remote_input(), snapshot()).observation.local_presence, PresenceState::Unknown);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b05_confirmed_local_absence_is_preserved() {
    let mut input = local_input(FileEventAction::Deleted);
    input.event.local_presence = Some(LocalPresenceEvidence::AbsentConfirmed);
    let mut facts = snapshot();
    facts.observation.local_presence = PresenceState::AbsentConfirmed;
    assert_eq!(result(input, facts).observation.local_presence, PresenceState::AbsentConfirmed);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b06_unknown_remote_presence_stays_unknown() {
    assert_eq!(result(local_input(FileEventAction::Deleted), snapshot()).observation.remote_presence, PresenceState::Unknown);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b07_confirmed_remote_absence_is_preserved() {
    let mut facts = snapshot();
    facts.observation.remote_presence = PresenceState::AbsentConfirmed;
    assert_eq!(result(local_input(FileEventAction::Modified), facts).observation.remote_presence, PresenceState::AbsentConfirmed);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b08_partial_coverage_is_not_complete() {
    let out = result(remote_input(), snapshot());
    assert_eq!(out.file.coverage, ObservationCoverage::Partial);
    assert!(!out.observation.evidence.remote_scope_complete);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b09_unmatched_retains_observation_evidence() {
    let mut facts = snapshot();
    facts.observation.local_presence = PresenceState::Present;
    let expected = facts.observation.clone();
    let out = result(local_input(FileEventAction::Modified), facts);
    assert_eq!(out.association, ContextAssociation::Unmatched);
    assert_eq!(out.observation, expected);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b10_stale_evidence_is_not_attached() {
    let mut facts = snapshot();
    let mut c = candidate("known", true);
    c.evidence.as_mut().unwrap().revision = 6;
    c.evidence.as_mut().unwrap().evidence.current_content = EvidenceRelation::Equal;
    facts.candidates.push(c);
    assert_eq!(result(remote_input(), facts).items[0].evidence, unknown_evidence());
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn c01_inventory_failure_propagates() {
    let error = AnalysisError { kind: AnalysisErrorKind::Inventory };
    assert_eq!(build(remote_input(), Err(error.clone())).0, Err(error));
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn c02_empty_success_is_not_failure() {
    assert!(build(remote_input(), Ok(snapshot())).0.is_ok());
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn c03_other_sync_is_rejected() {
    let mut facts = snapshot();
    facts.identity.sync_id = SyncId("other".into());
    assert!(build(remote_input(), Ok(facts)).0.is_err());
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn c04_other_account_is_not_associated() {
    let mut facts = snapshot();
    let mut c = candidate("foreign", true);
    c.item.remote_ref = Some(remote_ref("other", "remote-1"));
    facts.candidates.push(c);
    let out = build(remote_input(), Ok(facts)).0;
    assert!(out.is_err() || out.unwrap().items.is_empty());
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn c05_original_file_preserved() {
    let input = remote_input();
    assert_eq!(result(input.clone(), snapshot()).file, input);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn c06_only_read_port_is_used() {
    let input = remote_input();
    let (out, reads) = build(input.clone(), Ok(snapshot()));
    assert!(out.is_ok());
    assert_eq!(reads, vec![input]);
}
