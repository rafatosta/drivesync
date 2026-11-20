//! Behavior-first tests for ContextBuilder. Run the RED suite with --ignored.
use super::support::{file, item_context, unknown_evidence};
use drivesync_engine::{
    analyzer::{
        context::{ContextBuilder, ContextCandidate, ContextInventory, ContextInventorySnapshot, RevisionEvidence},
        AnalysisContext, AnalysisError, AnalysisErrorKind, ContextAssociation, ContextService,
        ObservationFreshness,
    },
    inventory::{BaselineSide, EvidenceRelation, PresenceState, SyncBaseline, SyncIdentity},
    models::*,
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

pub(super) fn path(value: &str) -> LocalRelativePath {
    LocalRelativePath::new(value.as_bytes().to_vec()).unwrap()
}

pub(super) fn remote_ref(account: &str, id: &str) -> RemoteFileRef {
    RemoteFileRef { account_id: AccountId(account.into()), file_id: RemoteFileId(id.into()) }
}

pub(super) fn local_metadata(id: &str, location: &str) -> LocalFile {
    use MetadataValue::{Known, NotApplicable, NotCollected};
    LocalFile {
        local_id: Some(LocalFileId(id.into())),
        relative_path: path(location),
        kind: Known(LocalEntryKind::Regular),
        mode: Known(LocalFileMode { raw_mode: 0o100640, permission_bits: 0o640 }),
        owner: LocalOwner { uid: Known(1000), gid: Known(1000) },
        times: LocalTimes { modified_at: Known(LocalTimestamp { unix_seconds: 2, nanoseconds: 0 }), changed_at: Known(LocalTimestamp { unix_seconds: 2, nanoseconds: 0 }), created_at: Known(LocalTimestamp { unix_seconds: 1, nanoseconds: 0 }) },
        storage: LocalStorage { size_bytes: Known(23), hard_link_count: Known(1), inode: Known(123), filesystem_device: Known(DeviceNumber { major: 8, minor: 2 }) },
        flags: LocalFileFlags { statx_attributes: NotCollected, statx_attributes_mask: NotCollected },
        symlink: LocalSymlink { target: NotApplicable },
    }
}

pub(super) fn remote_metadata(account: &str, id: &str, name: &str) -> RemoteFile {
    use MetadataValue::{Known, NotCollected};
    RemoteFile {
        reference: remote_ref(account, id), provider: ProviderId("google".into()),
        name: Known(name.into()), kind: Known(RemoteEntryKind::File),
        mime_type: NotCollected, parent_ids: Known(vec![RemoteFileId("root".into())]),
        trashed: Known(false), version: Known("7".into()), created_time: NotCollected,
        modified_time: NotCollected, size: Known(23), md5_checksum: NotCollected,
        sha256_checksum: NotCollected, provider_data: NotCollected,
    }
}

pub(super) fn remote_input() -> File {
    let remote = remote_metadata("account-1", "remote-1", "file.txt");
    let mut input = file(FileEventAction::SnapshotObserved, FileScope::Remote(remote.reference.clone()));
    input.event.remote = Some(RemoteObservation { source: RemoteObservationSource::Capture, removed: None, time: None, change_type: None, drive_id: None });
    input.coverage = ObservationCoverage::Partial;
    input.event.current = Some(FileLocation::Remote { reference: remote.reference.clone(), parent_id: Some(RemoteFileId("root".into())), name: Some("file.txt".into()) });
    input.data = Some(FileData::Remote(remote));
    input
}

pub(super) fn local_input(action: FileEventAction) -> File {
    let location = path("folder/file.txt");
    let mut input = file(action, FileScope::Local(location.clone()));
    if action == FileEventAction::Deleted {
        input.event.previous = Some(FileLocation::Local(location));
        input.event.local_presence = Some(LocalPresenceEvidence::AbsentConfirmed);
    } else {
        input.event.current = Some(FileLocation::Local(location));
        input.event.local_presence = Some(LocalPresenceEvidence::Present);
        input.data = Some(FileData::Local(local_metadata("local-1", "folder/file.txt")));
        if matches!(action, FileEventAction::Renamed | FileEventAction::Moved) {
            let previous = if action == FileEventAction::Renamed { "folder/old.txt" } else { "old/file.txt" };
            input.event.previous = Some(FileLocation::Local(path(previous)));
        }
    }
    input
}

pub(super) fn scope_input() -> File {
    file(FileEventAction::RecheckRequired, FileScope::WholeSync)
}

fn snapshot() -> ContextInventorySnapshot {
    ContextInventorySnapshot {
        identity: SyncIdentity { sync_id: SyncId("sync-1".into()), account_id: AccountId("account-1".into()), remote_root_id: "root".into() },
        candidates: vec![],
    }
}

fn candidate(id: &str, local_id: &str, remote_id: &str, location: &str) -> ContextCandidate {
    let mut subject = item_context(7);
    subject.item.id.0 = id.into();
    let local = local_metadata(local_id, location);
    let remote = remote_metadata("account-1", remote_id, location.rsplit('/').next().unwrap());
    subject.item.local_file_id = local.local_id.clone();
    subject.item.remote_ref = Some(remote.reference.clone());
    subject.item.local = Some(local.clone());
    subject.item.local_latest = Some(local);
    subject.item.remote = Some(remote.clone());
    subject.item.remote_latest = Some(remote);
    subject.item.local_presence = PresenceState::Present;
    subject.item.remote_presence = PresenceState::Present;
    ContextCandidate {
        item: subject.item,
        local_causal_order: Some(5),
        remote_causal_order: Some(5),
        evidence: Some(RevisionEvidence { revision: 7, evidence: subject.evidence }),
    }
}

fn known() -> ContextCandidate {
    candidate("known", "local-1", "remote-1", "folder/file.txt")
}

fn build(input: File, snapshot: Result<ContextInventorySnapshot, AnalysisError>) -> (Result<AnalysisContext, AnalysisError>, Vec<File>) {
    let reads = Arc::new(Mutex::new(vec![]));
    let service = ContextBuilder::new(FakeInventory { snapshot, reads: reads.clone() }, Box::new(UnavailableEvidence));
    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let result = runtime.block_on(service.build(input));
    let calls = reads.lock().unwrap().clone();
    (result, calls)
}

fn result(input: File, candidates: Vec<ContextCandidate>) -> AnalysisContext {
    input.validate().expect("valid detector fixture");
    let mut facts = snapshot();
    facts.candidates = candidates;
    build(input, Ok(facts)).0.unwrap()
}

fn matched(out: &AnalysisContext) -> &drivesync_engine::analyzer::AnalysisItemContext {
    match &out.association {
        ContextAssociation::Matched(item) => item,
        _ => panic!("expected exact identity match"),
    }
}

fn baseline() -> SyncBaseline {
    let side = BaselineSide {
        presence: PresenceState::Present, name: Some("file.txt".into()), local_name_bytes: Some(b"file.txt".to_vec()),
        parent_identity: None, version: Some("6".into()), checksum: None, size: Some(23),
        modified_at: Some(Timestamp { unix_millis: 10 }),
    };
    SyncBaseline { local: side.clone(), remote: side, confirmed_at: Timestamp { unix_millis: 10 } }
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a01_remote_identity_matches() {
    let out = result(remote_input(), vec![known()]);
    assert_eq!(matched(&out).item.id.0, "known");
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a02_local_identity_matches() {
    let out = result(local_input(FileEventAction::Modified), vec![known()]);
    assert_eq!(matched(&out).item.id.0, "known");
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a03_no_inventory_row_is_unmatched() {
    let out = result(remote_input(), vec![]);
    assert_eq!(out.association, ContextAssociation::Unmatched);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a04_name_alone_is_ambiguous() {
    let other = candidate("other", "local-2", "remote-2", "folder/file.txt");
    let out = result(remote_input(), vec![other]);
    assert!(matches!(out.association, ContextAssociation::Ambiguous(ref items) if items.len() == 1));
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a05_multiple_candidates_are_ambiguous() {
    let a = candidate("a", "local-2", "remote-2", "folder/file.txt");
    let b = candidate("b", "local-3", "remote-3", "other/file.txt");
    let out = result(remote_input(), vec![a, b]);
    assert!(matches!(out.association, ContextAssociation::Ambiguous(ref items) if items.len() == 2));
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a06_rename_retains_identity() {
    let prior = candidate("known", "local-1", "remote-1", "folder/old.txt");
    let out = result(local_input(FileEventAction::Renamed), vec![prior]);
    assert_eq!(matched(&out).item.id.0, "known");
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a07_move_retains_identity() {
    let prior = candidate("known", "local-1", "remote-1", "old/file.txt");
    let out = result(local_input(FileEventAction::Moved), vec![prior]);
    assert_eq!(matched(&out).item.id.0, "known");
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn a08_scope_contains_all_subjects() {
    let other = candidate("other", "local-2", "remote-2", "other.txt");
    let out = result(scope_input(), vec![known(), other]);
    assert!(matches!(out.association, ContextAssociation::Scope(ref items) if items.len() == 2));
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b01_preserve_both_known_states() {
    let c = known();
    let expected = c.item.clone();
    let out = result(remote_input(), vec![c]);
    assert_eq!(matched(&out).item, expected);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b02_preserve_confirmed_baseline() {
    let mut c = known();
    c.item.baseline = Some(baseline());
    c.evidence.as_mut().unwrap().evidence.baseline_confirmed = true;
    let out = result(remote_input(), vec![c]);
    assert_eq!(matched(&out).item.baseline, Some(baseline()));
    assert!(matched(&out).evidence.baseline_confirmed);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b03_missing_baseline_stays_missing() {
    let out = result(remote_input(), vec![known()]);
    assert!(matched(&out).item.baseline.is_none());
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b04_unknown_local_presence_stays_unknown() {
    assert_eq!(result(remote_input(), vec![]).observation.local_presence, PresenceState::Unknown);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b05_confirmed_local_absence_is_derived_from_file() {
    let out = result(local_input(FileEventAction::Deleted), vec![]);
    assert_eq!(out.observation.local_presence, PresenceState::AbsentConfirmed);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b06_unknown_remote_presence_stays_unknown() {
    assert_eq!(result(local_input(FileEventAction::Deleted), vec![]).observation.remote_presence, PresenceState::Unknown);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b07_confirmed_remote_absence_remains_inventory_fact() {
    let mut c = known();
    c.item.remote_presence = PresenceState::AbsentConfirmed;
    let out = result(local_input(FileEventAction::Modified), vec![c]);
    assert_eq!(matched(&out).item.remote_presence, PresenceState::AbsentConfirmed);
    assert_eq!(out.observation.remote_presence, PresenceState::Unknown);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b08_partial_coverage_is_not_complete() {
    let out = result(remote_input(), vec![]);
    assert_eq!(out.file.coverage, ObservationCoverage::Partial);
    assert!(!out.observation.evidence.remote_scope_complete);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b09_unmatched_retains_observation_evidence() {
    let out = result(local_input(FileEventAction::Modified), vec![]);
    assert_eq!(out.association, ContextAssociation::Unmatched);
    assert_eq!(out.observation.local_presence, PresenceState::Present);
    assert_eq!(out.observation.remote_presence, PresenceState::Unknown);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn b10_stale_evidence_is_not_attached() {
    let mut c = known();
    c.evidence.as_mut().unwrap().revision = 6;
    c.evidence.as_mut().unwrap().evidence.current_content = EvidenceRelation::Equal;
    let out = result(remote_input(), vec![c]);
    assert_eq!(matched(&out).evidence, unknown_evidence());
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn c01_inventory_failure_propagates() {
    let error = AnalysisError { kind: AnalysisErrorKind::Inventory };
    assert_eq!(build(remote_input(), Err(error.clone())).0, Err(error));
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn c02_empty_success_is_unmatched() {
    assert_eq!(build(remote_input(), Ok(snapshot())).0.unwrap().association, ContextAssociation::Unmatched);
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
fn c04_other_account_is_rejected() {
    let mut facts = snapshot();
    let mut c = known();
    c.item.remote_ref = Some(remote_ref("other", "remote-1"));
    c.item.remote = Some(remote_metadata("other", "remote-1", "file.txt"));
    c.item.remote_latest = c.item.remote.clone();
    facts.candidates.push(c);
    assert!(build(remote_input(), Ok(facts)).0.is_err());
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn c05_original_payload_preserved() {
    let input = remote_input();
    assert!(input.data.is_some());
    assert_eq!(result(input.clone(), vec![]).file, input);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn c06_only_read_port_is_used() {
    let input = remote_input();
    let (out, reads) = build(input.clone(), Ok(snapshot()));
    assert!(out.is_ok());
    assert_eq!(reads, vec![input]);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d01_same_path_different_identity_is_ambiguous() {
    let other = candidate("other", "local-2", "remote-2", "folder/file.txt");
    let out = result(local_input(FileEventAction::Modified), vec![other]);
    assert!(matches!(out.association, ContextAssociation::Ambiguous(ref items) if items.len() == 1));
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d02_identity_and_destination_disagree() {
    let other = candidate("other", "local-2", "remote-2", "folder/file.txt");
    let out = result(local_input(FileEventAction::Modified), vec![known(), other]);
    assert_eq!(matched(&out).item.id.0, "known");
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d03_future_evidence_is_discarded() {
    let mut c = known();
    c.evidence.as_mut().unwrap().revision = 8;
    c.evidence.as_mut().unwrap().evidence.current_content = EvidenceRelation::Equal;
    let out = result(remote_input(), vec![c]);
    assert_eq!(matched(&out).evidence, unknown_evidence());
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d04_missing_evidence_stays_unknown() {
    let mut c = known();
    c.evidence = None;
    let out = result(remote_input(), vec![c]);
    assert_eq!(matched(&out).evidence, unknown_evidence());
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d05_older_file_preserves_both_states() {
    let mut c = known();
    c.local_causal_order = Some(9);
    let input = local_input(FileEventAction::Modified);
    let expected = c.item.clone();
    let out = result(input.clone(), vec![c]);
    assert_eq!(out.file, input);
    assert_eq!(matched(&out).item, expected);
    assert_eq!(matched(&out).freshness, ObservationFreshness::Older);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d06_missing_causal_order_is_incomparable() {
    let mut input = local_input(FileEventAction::Modified);
    input.causal_order = None;
    let out = result(input, vec![known()]);
    assert_eq!(matched(&out).freshness, ObservationFreshness::Incomparable);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d07_empty_scope_is_scope() {
    assert_eq!(result(scope_input(), vec![]).association, ContextAssociation::Scope(vec![]));
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d08_invalid_file_rejected_before_read() {
    let mut input = remote_input();
    input.id.0.clear();
    assert!(input.validate().is_err());
    let (out, reads) = build(input, Ok(snapshot()));
    assert!(out.is_err());
    assert!(reads.is_empty());
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d09_identical_duplicate_candidates_deduplicated() {
    let c = known();
    let out = result(remote_input(), vec![c.clone(), c]);
    assert_eq!(matched(&out).item.id.0, "known");
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d10_complete_coverage_does_not_prove_opposite_absence() {
    let out = result(local_input(FileEventAction::Created), vec![]);
    assert_eq!(out.file.coverage, ObservationCoverage::Complete);
    assert_eq!(out.observation.remote_presence, PresenceState::Unknown);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d11_remote_input_other_account_rejected() {
    let mut input = remote_input();
    let remote = remote_metadata("other", "remote-1", "file.txt");
    input.scope = FileScope::Remote(remote.reference.clone());
    input.event.current = Some(FileLocation::Remote { reference: remote.reference.clone(), parent_id: Some(RemoteFileId("root".into())), name: Some("file.txt".into()) });
    input.data = Some(FileData::Remote(remote));
    assert!(input.validate().is_ok());
    assert!(build(input, Ok(snapshot())).0.is_err());
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d12_removed_from_scope_is_not_absence() {
    let mut input = file(FileEventAction::RemovedFromScope, FileScope::Remote(remote_ref("account-1", "remote-1")));
    input.event.remote = Some(RemoteObservation { source: RemoteObservationSource::Changes, removed: Some(true), time: None, change_type: None, drive_id: None });
    let out = result(input, vec![]);
    assert_eq!(out.observation.remote_presence, PresenceState::Unknown);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d13_unrelated_candidate_is_not_matched() {
    let other = candidate("other", "local-2", "remote-2", "different.txt");
    assert_eq!(result(remote_input(), vec![other]).association, ContextAssociation::Unmatched);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d14_contradictory_duplicate_is_rejected() {
    let c = known();
    let mut other = c.clone();
    other.item.revision += 1;
    let mut facts = snapshot();
    facts.candidates = vec![c, other];
    assert!(build(remote_input(), Ok(facts)).0.is_err());
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d15_remote_numeric_version_precedes_causal_order() {
    let mut c = known();
    c.item.remote.as_mut().unwrap().version = MetadataValue::Known("10".into());
    c.item.remote_latest = c.item.remote.clone();
    let out = result(remote_input(), vec![c]);
    assert_eq!(matched(&out).freshness, ObservationFreshness::Older);
}

#[test]
#[ignore = "TDD red: ContextBuilder implementation pending"]
fn d16_newer_local_observation_is_not_applied_to_snapshot() {
    let c = known();
    let expected = c.item.clone();
    let out = result(local_input(FileEventAction::Modified), vec![c]);
    assert_eq!(matched(&out).freshness, ObservationFreshness::Newer);
    assert_eq!(matched(&out).item, expected);
}

use drivesync_engine::analyzer::context::{ContextEvidenceReader, EvidenceFuture, ContextScope, CurrentSide, LocalDestination, RemoteDestination};
struct UnavailableEvidence;
impl ContextEvidenceReader for UnavailableEvidence {
    fn scope<'a>(&'a self, _: &'a SyncId) -> EvidenceFuture<'a, MetadataValue<ContextScope>> { Box::pin(async { Ok(MetadataValue::Unavailable) }) }
    fn local<'a>(&'a self, _: &'a ContextScope, _: &'a LocalRelativePath) -> EvidenceFuture<'a, CurrentSide<LocalFile>> { Box::pin(async { Err(AnalysisError { kind: AnalysisErrorKind::Evidence }) }) }
    fn remote<'a>(&'a self, _: &'a ContextScope, _: &'a RemoteFileRef) -> EvidenceFuture<'a, CurrentSide<RemoteFile>> { Box::pin(async { Err(AnalysisError { kind: AnalysisErrorKind::Evidence }) }) }
    fn local_destination<'a>(&'a self, _: &'a ContextScope, _: &'a ObservedParent, _: &'a [u8]) -> EvidenceFuture<'a, MetadataValue<LocalDestination>> { Box::pin(async { Ok(MetadataValue::Unavailable) }) }
    fn remote_destination<'a>(&'a self, _: &'a ContextScope, _: &'a RemoteFileRef, _: &'a str) -> EvidenceFuture<'a, MetadataValue<RemoteDestination>> { Box::pin(async { Ok(MetadataValue::Unavailable) }) }
    fn local_digest<'a>(&'a self, _: &'a ContextScope, _: &'a LocalFile) -> EvidenceFuture<'a, MetadataValue<ContentDigest>> { Box::pin(async { Ok(MetadataValue::Unavailable) }) }
    fn remote_digest<'a>(&'a self, _: &'a ContextScope, _: &'a RemoteFile) -> EvidenceFuture<'a, MetadataValue<ContentDigest>> { Box::pin(async { Ok(MetadataValue::Unavailable) }) }
}
