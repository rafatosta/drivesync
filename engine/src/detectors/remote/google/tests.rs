//! Deterministic provider, sink and checkpoint faults exercise the real Google producer.
use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
struct FakeApi {
    replies: Mutex<VecDeque<RemoteResult<Response>>>,
    calls: Mutex<Vec<String>>,
}
impl FakeApi {
    fn new(replies: Vec<RemoteResult<Response>>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            calls: Mutex::new(Vec::new()),
        })
    }
}
impl Api for FakeApi {
    fn call(&self, request: Request, cancel: &Cancellation) -> RemoteResult<Response> {
        cancel.check()?;
        self.calls.lock().unwrap().push(match request {
            Request::Start => "start".into(),
            Request::Get(id) => format!("get:{id}"),
            Request::List { parent, token } => format!("list:{parent}:{token:?}"),
            Request::Changes(token) => format!("changes:{token}"),
        });
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected provider request")
    }
}
#[derive(Default)]
struct Memory {
    progress: Mutex<Option<Vec<u8>>>,
    files: Mutex<Vec<File>>,
    fail_emit: AtomicUsize,
    fail_save: AtomicUsize,
    saves: AtomicUsize,
}
impl RemoteBackend for Memory {
    fn load(&self) -> RemoteResult<Option<Vec<u8>>> {
        Ok(self.progress.lock().unwrap().clone())
    }
    fn save(&self, previous: Option<&[u8]>, value: &[u8]) -> RemoteResult<()> {
        let call = self.saves.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_save.load(Ordering::SeqCst) == call {
            return Err(RemoteFailure::new(RemoteError::CheckpointUnavailable));
        }
        let mut progress = self.progress.lock().unwrap();
        assert_eq!(progress.as_deref(), previous);
        *progress = Some(value.to_vec());
        Ok(())
    }
    fn emit(&self, file: &File) -> RemoteResult<bool> {
        if self.fail_emit.load(Ordering::SeqCst) > 0 {
            self.fail_emit.fetch_sub(1, Ordering::SeqCst);
            return Err(RemoteFailure::new(RemoteError::SinkUnavailable));
        }
        file.validate().unwrap();
        let mut files = self.files.lock().unwrap();
        if let Some(old) = files.iter().find(|old| old.id == file.id) {
            assert_eq!(old, file);
            return Ok(false);
        }
        files.push(file.clone());
        Ok(true)
    }
}
fn raw(id: &str, name: &str, parent: &str, directory: bool) -> GoogleFile {
    serde_json::from_value(serde_json::json!({"id":id,"name":name,"parents":[parent],"mimeType":if directory {"application/vnd.google-apps.folder"} else {"text/plain"},"trashed":false,"version":"1","md5Checksum":"hash"})).unwrap()
}
fn root() -> GoogleFile {
    raw("root-id", "Root", "outside", true)
}
fn list(files: Vec<GoogleFile>, next: Option<&str>) -> RemoteResult<Response> {
    Ok(Response::List(GoogleFileList {
        files,
        next_page_token: next.map(str::to_owned),
        incomplete_search: Some(false),
    }))
}
fn changes(
    files: Vec<GoogleFile>,
    next: Option<&str>,
    terminal: Option<&str>,
) -> RemoteResult<Response> {
    use crate::providers::google::{FileId, GoogleChange};
    Ok(Response::Changes(GoogleChangeList {
        changes: files
            .into_iter()
            .map(|file| GoogleChange {
                file_id: Some(FileId(file.id.as_ref().unwrap().0.clone())),
                removed: Some(false),
                file: Some(file),
                ..Default::default()
            })
            .collect(),
        next_page_token: next.map(str::to_owned),
        new_start_page_token: terminal.map(str::to_owned),
    }))
}
fn fixture(
    replies: Vec<RemoteResult<Response>>,
) -> (GoogleRemoteDetector, Arc<FakeApi>, Arc<Memory>) {
    let api = FakeApi::new(replies);
    let backend = Arc::new(Memory::default());
    (
        GoogleRemoteDetector {
            scope: RemoteDetectorScope {
                account_id: AccountId("account".into()),
                sync_id: SyncId("sync".into()),
                remote_root_id: "root-id".into(),
            },
            api: api.clone(),
            backend: backend.clone(),
        },
        api,
        backend,
    )
}
fn initial() -> Vec<RemoteResult<Response>> {
    vec![
        Ok(Response::Start("cursor0".into())),
        Ok(Response::File(Box::new(root()))),
        list(vec![raw("f", "old", "root-id", false)], None),
        changes(vec![], None, Some("cursor1")),
    ]
}
#[test]
fn cursor_precedes_tree_and_pagination_is_not_truncated() {
    let mut replies = initial();
    replies[2] = list(vec![raw("d", "Directory", "root-id", true)], Some("tree2"));
    replies.insert(3, list(vec![raw("f", "File", "root-id", false)], None));
    replies.insert(4, list(vec![raw("child", "Child", "d", false)], None));
    let (source, api, sink) = fixture(replies);
    let first = source
        .poll(RemotePollMode::Initial, 2, &Cancellation::default())
        .unwrap();
    assert!(first.has_more);
    assert_eq!(first.coverage, RemoteCoverage::Partial);
    let result = source
        .poll(RemotePollMode::Incremental, 10, &Cancellation::default())
        .unwrap();
    assert!(!result.has_more);
    assert_eq!(sink.files.lock().unwrap().len(), 4);
    assert_eq!(
        *api.calls.lock().unwrap(),
        [
            "start",
            "get:root-id",
            "list:root-id:None",
            "list:root-id:Some(\"tree2\")",
            "list:d:None",
            "changes:cursor0"
        ]
    );
}
#[test]
fn sink_failure_and_checkpoint_failure_replay_identical_objects() {
    let (source, api, sink) = fixture(initial());
    sink.fail_emit.store(1, Ordering::SeqCst);
    assert_eq!(
        source
            .poll(RemotePollMode::Initial, 10, &Cancellation::default())
            .unwrap_err()
            .kind,
        RemoteError::SinkUnavailable
    );
    let staged: State = serde_json::from_slice(&sink.load().unwrap().unwrap()).unwrap();
    let original = staged.pending.unwrap().files[0].clone();
    assert!(staged.progress.cursor.is_none());
    // The next save fails after the staged occurrence has been accepted.
    sink.fail_save.store(2, Ordering::SeqCst);
    assert_eq!(
        source
            .poll(RemotePollMode::Incremental, 10, &Cancellation::default())
            .unwrap_err()
            .kind,
        RemoteError::CheckpointUnavailable
    );
    assert_eq!(sink.files.lock().unwrap()[0], original);
    source
        .poll(RemotePollMode::Incremental, 10, &Cancellation::default())
        .unwrap();
    assert_eq!(
        sink.files
            .lock()
            .unwrap()
            .iter()
            .filter(|file| file.id == original.id)
            .count(),
        1
    );
    assert_eq!(
        api.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| *call == "start")
            .count(),
        1
    );
}
#[test]
fn matching_identity_correlates_rename_move_and_content_while_removed_is_recheck() {
    let mut replies = initial();
    let mut renamed = raw("f", "renamed", "root-id", false);
    renamed.version = Some("2".into());
    let mut modified = renamed.clone();
    modified.md5_checksum = Some("new-hash".into());
    replies.push(changes(vec![renamed.clone()], Some("page2"), None));
    replies.push(changes(vec![modified.clone()], Some("page3"), None));
    replies.push(Ok(Response::Changes(serde_json::from_value(serde_json::json!({"changes":[{"fileId":"f","removed":true}],"newStartPageToken":"end"})).unwrap())));
    let (source, _, sink) = fixture(replies);
    source
        .poll(RemotePollMode::Initial, 10, &Cancellation::default())
        .unwrap();
    source
        .poll(RemotePollMode::Incremental, 10, &Cancellation::default())
        .unwrap();
    let files = sink.files.lock().unwrap();
    assert_eq!(files[2].event.kind, FileEventKind::Renamed);
    assert_ne!(files[2].event.previous, files[2].event.current);
    assert_eq!(files[3].event.kind, FileEventKind::Modified);
    assert_eq!(files[4].event.kind, FileEventKind::RecheckRequired);
    assert!(files[4].data.is_none());
    assert!(!files
        .iter()
        .any(|file| file.event.kind == FileEventKind::Deleted));
}
#[test]
fn partial_native_metadata_preserves_absence_and_filters_private_fields() {
    let mut native: GoogleFile = serde_json::from_value(serde_json::json!({"id":"native","name":"Doc","mimeType":"application/vnd.google-apps.document","resourceKey":"private-key","description":"private-description","owners":[{"emailAddress":"private-email"}]})).unwrap();
    let file = adapt(&native, &AccountId("account".into())).unwrap();
    assert_eq!(file.size, MetadataValue::NotCollected);
    assert_eq!(file.md5_checksum, MetadataValue::NotCollected);
    let MetadataValue::Known(payload) = file.provider_data else {
        panic!()
    };
    let payload = String::from_utf8(payload.payload).unwrap();
    assert!(!payload.contains("private"));
    assert!(!payload.contains("resourceKey"));
    native.id = None;
    assert!(adapt(&native, &AccountId("account".into())).is_err());
}
#[test]
fn malformed_page_never_commits_its_cursor_and_expiry_requires_explicit_recovery() {
    let mut replies = initial();
    replies.push(Err(RemoteFailure::new(RemoteError::CursorExpired)));
    replies.extend(initial());
    let (source, _, sink) = fixture(replies);
    source
        .poll(RemotePollMode::Initial, 10, &Cancellation::default())
        .unwrap();
    let before: State = serde_json::from_slice(&sink.load().unwrap().unwrap()).unwrap();
    assert_eq!(
        source
            .poll(RemotePollMode::Incremental, 10, &Cancellation::default())
            .unwrap_err()
            .kind,
        RemoteError::CursorExpired
    );
    let after: State = serde_json::from_slice(&sink.load().unwrap().unwrap()).unwrap();
    assert_eq!(before.progress.cursor, after.progress.cursor);
    source
        .poll(RemotePollMode::Recovery, 10, &Cancellation::default())
        .unwrap();
    assert!(sink
        .files
        .lock()
        .unwrap()
        .iter()
        .any(|file| file.event.uncertainty == Some(ObservationUncertainty::CoverageIncomplete)));
}
#[test]
fn unrelated_changes_are_filtered_by_ancestry_and_unknown_scope_is_not_deleted() {
    let mut replies = initial();
    replies.push(changes(
        vec![raw("unrelated", "Outside", "outside-root", false)],
        None,
        Some("end"),
    ));
    let mut outside = raw("outside-root", "OutsideRoot", "", true);
    outside.parents = Some(vec![]);
    replies.push(Ok(Response::File(Box::new(outside))));
    let (source, _, sink) = fixture(replies);
    source
        .poll(RemotePollMode::Initial, 10, &Cancellation::default())
        .unwrap();
    source
        .poll(RemotePollMode::Incremental, 10, &Cancellation::default())
        .unwrap();
    assert_eq!(sink.files.lock().unwrap().len(), 2);
}
#[test]
fn stop_token_prevents_network_or_checkpoint_mutation() {
    let (source, api, sink) = fixture(vec![]);
    let cancel = Cancellation::default();
    cancel.cancel();
    assert_eq!(
        source
            .poll(RemotePollMode::Initial, 10, &cancel)
            .unwrap_err()
            .kind,
        RemoteError::Cancelled
    );
    assert!(api.calls.lock().unwrap().is_empty());
    assert!(sink.load().unwrap().is_none());
}

#[test]
fn creation_requires_creation_time_evidence_and_compound_changes_stay_uncertain() {
    let mut file = raw("new", "New", "root-id", false);
    assert_eq!(
        classify(None, &file, false, Some(1000)).0,
        FileEventKind::RecheckRequired
    );
    file.created_time = Some("1970-01-01T00:00:02Z".into());
    assert_eq!(
        classify(None, &file, false, Some(1000)).0,
        FileEventKind::Created
    );
    assert_eq!(
        classify(None, &file, false, Some(3000)).0,
        FileEventKind::RecheckRequired
    );
    let previous = file.clone();
    file.name = Some("Renamed".into());
    file.parents = Some(vec![crate::providers::google::FileId("another".into())]);
    assert_eq!(
        classify(Some(&previous), &file, false, Some(1000)),
        (
            FileEventKind::RecheckRequired,
            Some(ObservationUncertainty::ConflictingSequence)
        )
    );
}
#[test]
fn inaccessible_scope_emits_one_durable_warning_without_advancing_progress() {
    let (source, _, sink) = fixture(vec![
        Err(RemoteFailure::new(RemoteError::PermissionDenied)),
        Err(RemoteFailure::new(RemoteError::PermissionDenied)),
    ]);
    for _ in 0..2 {
        assert_eq!(
            source
                .poll(RemotePollMode::Initial, 10, &Cancellation::default())
                .unwrap_err()
                .kind,
            RemoteError::PermissionDenied
        );
    }
    let files = sink.files.lock().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].event.kind, FileEventKind::ScopeUnavailable);
    assert_eq!(files[0].coverage, ObservationCoverage::Unavailable);
    let progress: State = serde_json::from_slice(&sink.load().unwrap().unwrap()).unwrap();
    assert!(progress.progress.cursor.is_none());
}
#[test]
fn incomplete_changes_page_preserves_cursor_and_requests_recheck() {
    let mut replies = initial();
    replies.push(Ok(Response::Changes(GoogleChangeList::default())));
    let (source, _, sink) = fixture(replies);
    source
        .poll(RemotePollMode::Initial, 10, &Cancellation::default())
        .unwrap();
    assert_eq!(
        source
            .poll(RemotePollMode::Incremental, 10, &Cancellation::default())
            .unwrap_err()
            .kind,
        RemoteError::PageIncomplete
    );
    let progress: State = serde_json::from_slice(&sink.load().unwrap().unwrap()).unwrap();
    assert_eq!(progress.progress.cursor.as_deref(), Some("cursor1"));
    assert_eq!(
        sink.files.lock().unwrap().last().unwrap().event.uncertainty,
        Some(ObservationUncertainty::CoverageIncomplete)
    );
}
