//! Exercises the public Operator with real temporary files/SQLite and the real
//! Google provider against a stateful loopback HTTP server. No personal accounts.
use super::*;
use crate::inventory::*;
use md5::{Digest, Md5};
use serde_json::{json, Value};
use sha2::Sha256;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};

#[derive(Default)]
struct Faults {
    truncate_download: bool,
    corrupt_download: bool,
    change_version: bool,
    fail_chunk: bool,
    lose_final: bool,
    expire: bool,
    lose_start: bool,
    occupied: bool,
    omit_sha: bool,
    corrupt_ack: bool,
    no_download: bool,
    metadata_failure: Option<u16>,
}
struct Session {
    id: String,
    name: String,
    bytes: Vec<u8>,
}
struct State {
    source: Vec<u8>,
    faults: Faults,
    version: String,
    files: HashMap<String, Value>,
    sessions: HashMap<String, Session>,
    generated: usize,
    started: usize,
    chunks: Vec<u64>,
    requests: Vec<(String, String)>,
}
struct Server {
    base: String,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    fn new(bytes: Vec<u8>, faults: Faults) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State {
            source: bytes,
            faults,
            version: "1".into(),
            files: HashMap::new(),
            sessions: HashMap::new(),
            generated: 0,
            started: 0,
            chunks: vec![],
            requests: vec![],
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let data = state.clone();
        let uri = base.clone();
        let thread = std::thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => handle(stream, &uri, &data),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            base,
            state,
            stop,
            thread: Some(thread),
        }
    }
    fn client(&self) -> g::GoogleDriveClient {
        g::GoogleDriveClient::with_base_urls(
            format!("{}/drive/v3", self.base),
            format!("{}/upload/drive/v3", self.base),
        )
        .unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}
fn blob(id: &str, name: &str, bytes: &[u8], version: &str, omit_sha: bool) -> Value {
    let mut result = json!({"id":id,"name":name,"mimeType":"application/octet-stream","parents":["root"],"trashed":false,"version":version,"size":bytes.len().to_string(),"md5Checksum":format!("{:x}",Md5::digest(bytes)),"capabilities":{"canDownload":true}});
    if !omit_sha {
        result["sha256Checksum"] = json!(format!("{:x}", Sha256::digest(bytes)));
    }
    result
}
fn respond(
    stream: &mut TcpStream,
    code: u16,
    headers: &[(&str, String)],
    body: &[u8],
    length: Option<usize>,
) {
    let mut response = format!(
        "HTTP/1.1 {code} Test\r\nConnection: close\r\nContent-Length: {}\r\n",
        length.unwrap_or(body.len())
    );
    for (k, v) in headers {
        response.push_str(&format!("{k}: {v}\r\n"));
    }
    response.push_str("\r\n");
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(body);
}
fn handle(mut stream: TcpStream, base: &str, data: &Arc<Mutex<State>>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut request = vec![];
    let mut buffer = [0; 65536];
    let (end, length) = loop {
        let n = match stream.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        request.extend_from_slice(&buffer[..n]);
        if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
            let text = String::from_utf8_lossy(&request[..end]);
            let length = text
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|n| n.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            if request.len() >= end + 4 + length {
                break (end, length);
            }
        }
    };
    let header = String::from_utf8_lossy(&request[..end]);
    let first = header.lines().next().unwrap();
    let parts: Vec<_> = first.split_whitespace().collect();
    let method = parts[0];
    let url = reqwest::Url::parse(&format!("{base}{}", parts[1])).unwrap();
    let path = url.path();
    let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
    let bytes = &request[end + 4..end + 4 + length];
    let mut state = data.lock().unwrap();
    state.requests.push((method.into(), parts[1].into()));
    if method == "GET" {
        if let Some(code) = state.faults.metadata_failure.take() {
            respond(
                &mut stream,
                code,
                &[],
                br#"{"error":{"code":503,"message":"private-server-detail"}}"#,
                None,
            );
            return;
        }
    }
    let mut code = 200;
    let mut headers = vec![];
    let response = if method == "GET" && path.ends_with("/generateIds") {
        state.generated += 1;
        json!({"ids":[format!("generated-{}",state.generated)]})
    } else if method == "GET" && path == "/drive/v3/files" {
        let files = if state.faults.occupied {
            vec![json!({"id":"foreign"})]
        } else {
            state
                .files
                .values()
                .filter(|f| {
                    f["name"].as_str().is_some_and(|name| {
                        query
                            .get("q")
                            .is_some_and(|q| q.contains(&format!("name='{name}'")))
                    })
                })
                .map(|f| json!({"id":f["id"]}))
                .collect()
        };
        json!({"files":files,"incompleteSearch":false})
    } else if method == "GET" && path.ends_with("/root") {
        json!({"id":"root","name":"root","mimeType":FOLDER,"trashed":false,"version":"1","capabilities":{"canAddChildren":true}})
    } else if method == "GET" && path.ends_with("/source") {
        if query.get("alt").map(String::as_str) == Some("media") {
            let mut body = state.source.clone();
            if state.faults.corrupt_download && !body.is_empty() {
                body[0] ^= 1;
            }
            let length = body.len();
            if state.faults.truncate_download {
                state.faults.truncate_download = false;
                body.truncate(body.len() / 2);
            }
            if state.faults.change_version {
                state.version = "2".into();
            }
            respond(&mut stream, 200, &[], &body, Some(length));
            return;
        }
        let mut file = blob(
            "source",
            "source.bin",
            &state.source,
            &state.version,
            state.faults.omit_sha,
        );
        if state.faults.no_download {
            file["capabilities"]["canDownload"] = json!(false);
        }
        file
    } else if method == "GET" && path.starts_with("/drive/v3/files/") {
        match state.files.get(path.rsplit('/').next().unwrap()) {
            Some(file) => file.clone(),
            None => {
                code = 404;
                json!({"error":{"code":404}})
            }
        }
    } else if method == "POST" && path == "/upload/drive/v3/files" {
        let metadata: Value = serde_json::from_slice(bytes).unwrap();
        let id = metadata["id"].as_str().unwrap().to_owned();
        if state.files.contains_key(&id) {
            code = 409;
            json!({"error":{"code":409}})
        } else {
            state.started += 1;
            let token = format!("session-{}", state.started);
            state.sessions.insert(
                token.clone(),
                Session {
                    id,
                    name: metadata["name"].as_str().unwrap().into(),
                    bytes: vec![],
                },
            );
            headers.push((
                "Location",
                format!("{base}/upload/drive/v3/files?upload_id={token}"),
            ));
            if state.faults.lose_start {
                state.faults.lose_start = false;
                return;
            }
            json!({})
        }
    } else if method == "PUT" && path == "/upload/drive/v3/files" {
        let token = query.get("upload_id").unwrap();
        if !state.sessions.contains_key(token) {
            code = 404;
            json!({"error":{"code":404}})
        } else {
            let range = header
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-range:")
                        .map(|s| s.trim().to_owned())
                })
                .unwrap();
            let total: usize = range.rsplit('/').next().unwrap().parse().unwrap();
            let is_chunk = !range.starts_with("bytes */") || total == 0;
            let session = state.sessions.get_mut(token).unwrap();
            if is_chunk {
                let offset = if bytes.is_empty() {
                    0
                } else {
                    range
                        .strip_prefix("bytes ")
                        .unwrap()
                        .split('-')
                        .next()
                        .unwrap()
                        .parse::<usize>()
                        .unwrap()
                };
                if offset != session.bytes.len() {
                    code = 400;
                    json!({"error":{"code":400}})
                } else {
                    session.bytes.extend_from_slice(bytes);
                    let id = session.id.clone();
                    let name = session.name.clone();
                    let content = session.bytes.clone();
                    state.chunks.push(offset as u64);
                    if content.len() == total {
                        let file = blob(&id, &name, &content, "1", state.faults.omit_sha);
                        state.files.insert(id, file.clone());
                        if state.faults.lose_final {
                            state.faults.lose_final = false;
                            return;
                        }
                        file
                    } else {
                        if state.faults.fail_chunk {
                            state.faults.fail_chunk = false;
                            if state.faults.expire {
                                state.sessions.remove(token);
                            }
                            return;
                        }
                        code = 308;
                        let end = if state.faults.corrupt_ack {
                            total + 1
                        } else {
                            content.len() - 1
                        };
                        headers.push(("Range", format!("bytes=0-{end}")));
                        json!({})
                    }
                }
            } else {
                let offset = session.bytes.len();
                let id = session.id.clone();
                if offset == total {
                    state.files.get(&id).cloned().unwrap_or_else(|| json!({}))
                } else {
                    code = 308;
                    if offset > 0 {
                        headers.push(("Range", format!("bytes=0-{}", offset - 1)));
                    }
                    json!({})
                }
            }
        }
    } else {
        code = 404;
        json!({"error":{"code":404}})
    };
    respond(
        &mut stream,
        code,
        &headers,
        &serde_json::to_vec(&response).unwrap(),
        None,
    );
}

struct Fixture {
    root: PathBuf,
    inventory: Arc<InventoryManager>,
    server: Server,
}
impl Fixture {
    fn new(bytes: Vec<u8>, faults: Faults) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "operator-transfer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let inventory = Arc::new(
            InventoryManager::open(OpenInventoryRequest {
                local_root: root.clone(),
                expected: SyncIdentity {
                    sync_id: SyncId("sync".into()),
                    account_id: AccountId("account".into()),
                    remote_root_id: "root".into(),
                },
                create_if_missing: true,
            })
            .unwrap()
            .inventory,
        );
        Self {
            root,
            inventory,
            server: Server::new(bytes, faults),
        }
    }
    fn scope(&self) -> OperatorScope {
        OperatorScope {
            sync_id: SyncId("sync".into()),
            account_id: AccountId("account".into()),
            local_root: self.root.clone(),
            remote_root_id: RemoteFileId("root".into()),
        }
    }
    fn operator(&self) -> GoogleOperator {
        GoogleOperator::testing(self.scope(), self.inventory.clone(), self.server.client())
    }
    fn download(&self, key: &str) -> DownloadRequest {
        let state = self.server.state.lock().unwrap();
        let bytes = &state.source;
        let remote = RemoteFile {
            reference: RemoteFileRef {
                account_id: AccountId("account".into()),
                file_id: RemoteFileId("source".into()),
            },
            provider: ProviderId("google".into()),
            name: MetadataValue::Known("source.bin".into()),
            kind: MetadataValue::Known(RemoteEntryKind::File),
            mime_type: MetadataValue::Known("application/octet-stream".into()),
            parent_ids: MetadataValue::Known(vec![RemoteFileId("root".into())]),
            trashed: MetadataValue::Known(false),
            version: MetadataValue::Known(state.version.clone()),
            created_time: MetadataValue::NotCollected,
            modified_time: MetadataValue::NotCollected,
            size: MetadataValue::Known(bytes.len() as u64),
            md5_checksum: MetadataValue::Known(format!("{:x}", Md5::digest(bytes))),
            sha256_checksum: if state.faults.omit_sha {
                MetadataValue::NotCollected
            } else {
                MetadataValue::Known(format!("{:x}", Sha256::digest(bytes)))
            },
            provider_data: MetadataValue::NotCollected,
        };
        DownloadRequest {
            execution: ExecutionContext {
                id: ExecutionId::new(key.into()).unwrap(),
                source: Some(SourceEvidence::Remote {
                    metadata: Box::new(remote.clone()),
                    revision: None,
                }),
            },
            source: ResourceRef::Remote {
                provider_id: ProviderId("google".into()),
                reference: remote.reference,
            },
            destination: DirectoryRef::LocalRoot,
            name: ItemName::Local("download.bin".into()),
            options: ExecutionOptions::default(),
        }
    }
    fn upload(&self, key: &str) -> UploadRequest {
        let source = fs::observe(
            &std::fs::File::open(self.root.join("source.bin")).unwrap(),
            LocalRelativePath::new(b"source.bin".to_vec()).unwrap(),
        )
        .unwrap();
        UploadRequest {
            execution: ExecutionContext {
                id: ExecutionId::new(key.into()).unwrap(),
                source: Some(SourceEvidence::Local {
                    metadata: Box::new(source.clone()),
                    sha256: None,
                }),
            },
            source: ResourceRef::Local {
                relative_path: source.relative_path,
                local_id: source.local_id,
            },
            destination: DirectoryRef::Remote {
                provider_id: ProviderId("google".into()),
                reference: RemoteFileRef {
                    account_id: AccountId("account".into()),
                    file_id: RemoteFileId("root".into()),
                },
            },
            name: ItemName::Remote("upload.bin".into()),
            options: ExecutionOptions::default(),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

#[tokio::test]
async fn downloads_empty_binary_and_large_blobs_and_retry_does_not_redownload() {
    for bytes in [
        vec![],
        b"binary\0\xff".to_vec(),
        vec![7; 2 * 1024 * 1024 + 3],
    ] {
        let f = Fixture::new(bytes.clone(), Faults::default());
        let op = f.operator();
        let request = f.download("download");
        let result = op.download(request.clone()).await.unwrap();
        assert_eq!(result.bytes_transferred, bytes.len() as u64);
        assert_eq!(result.integrity, IntegrityStatus::Verified);
        assert_eq!(std::fs::read(f.root.join("download.bin")).unwrap(), bytes);
        let count = f.server.state.lock().unwrap().requests.len();
        assert_eq!(op.download(request).await.unwrap().bytes_transferred, 0);
        assert_eq!(f.server.state.lock().unwrap().requests.len(), count);
    }
}
#[tokio::test]
async fn interrupted_download_never_publishes_partial_bytes_and_recovers_on_retry() {
    let f = Fixture::new(
        vec![8; 150000],
        Faults {
            truncate_download: true,
            ..Default::default()
        },
    );
    let request = f.download("interrupted");
    assert!(f.operator().download(request.clone()).await.is_err());
    assert!(!f.root.join("download.bin").exists());
    f.operator().download(request).await.unwrap();
    assert_eq!(
        std::fs::read(f.root.join("download.bin")).unwrap(),
        vec![8; 150000]
    );
}
#[tokio::test]
async fn corrupt_or_changed_remote_content_cannot_publish() {
    for faults in [
        Faults {
            corrupt_download: true,
            ..Default::default()
        },
        Faults {
            change_version: true,
            ..Default::default()
        },
        Faults {
            no_download: true,
            ..Default::default()
        },
    ] {
        let f = Fixture::new(b"original".to_vec(), faults);
        assert!(f.operator().download(f.download("guarded")).await.is_err());
        assert!(!f.root.join("download.bin").exists());
    }
}
#[tokio::test]
async fn download_never_replaces_preexisting_files_or_symlinks_and_supports_md5() {
    let f = Fixture::new(
        b"abc".to_vec(),
        Faults {
            omit_sha: true,
            ..Default::default()
        },
    );
    std::fs::write(f.root.join("download.bin"), b"keep").unwrap();
    assert_eq!(
        f.operator()
            .download(f.download("occupied"))
            .await
            .unwrap_err()
            .kind,
        OperatorError::DestinationOccupied
    );
    assert_eq!(std::fs::read(f.root.join("download.bin")).unwrap(), b"keep");
    let mut request = f.download("md5");
    request.name = ItemName::Local("another.bin".into());
    f.operator().download(request).await.unwrap();
    assert_eq!(std::fs::read(f.root.join("another.bin")).unwrap(), b"abc");
}
#[tokio::test]
async fn uploads_empty_binary_and_multichunk_blobs_once_using_reserved_identity() {
    for bytes in [vec![], b"abc\0\xff".to_vec(), vec![9; 2 * 1024 * 1024 + 3]] {
        let f = Fixture::new(vec![], Faults::default());
        std::fs::write(f.root.join("source.bin"), &bytes).unwrap();
        let request = f.upload("upload");
        let op = f.operator();
        let result = op.upload(request.clone()).await.unwrap();
        assert_eq!(result.integrity, IntegrityStatus::Verified);
        assert_eq!(result.bytes_transferred, bytes.len() as u64);
        let retried = op.upload(request).await.unwrap();
        assert_eq!(retried.bytes_transferred, 0);
        assert_eq!(result.item.resource, retried.item.resource);
        let state = f.server.state.lock().unwrap();
        assert_eq!(state.generated, 1);
        assert_eq!(state.files.len(), 1);
        assert_eq!(
            state
                .sessions
                .values()
                .find(|s| s.id == "generated-1")
                .unwrap()
                .bytes,
            bytes
        );
    }
}
#[tokio::test]
async fn interrupted_uploads_resume_at_server_offset_and_expired_sessions_keep_the_id() {
    for expire in [false, true] {
        let bytes = vec![5; 2 * 1024 * 1024 + 3];
        let f = Fixture::new(
            vec![],
            Faults {
                fail_chunk: true,
                expire,
                ..Default::default()
            },
        );
        std::fs::write(f.root.join("source.bin"), &bytes).unwrap();
        let request = f.upload("resumed");
        assert!(f.operator().upload(request.clone()).await.is_err());
        let result = f.operator().upload(request).await.unwrap();
        assert_eq!(result.integrity, IntegrityStatus::Verified);
        let state = f.server.state.lock().unwrap();
        assert_eq!(state.generated, 1);
        assert_eq!(state.files.len(), 1);
        assert_eq!(state.started, if expire { 2 } else { 1 });
        assert_eq!(state.chunks[1], if expire { 0 } else { 1024 * 1024 });
    }
}
#[tokio::test]
async fn lost_final_and_initial_session_responses_never_create_a_second_identity() {
    for faults in [
        Faults {
            lose_final: true,
            ..Default::default()
        },
        Faults {
            lose_start: true,
            ..Default::default()
        },
    ] {
        let f = Fixture::new(vec![], faults);
        std::fs::write(f.root.join("source.bin"), b"abc").unwrap();
        let request = f.upload("lost");
        assert!(f.operator().upload(request.clone()).await.is_err());
        f.operator().upload(request).await.unwrap();
        let state = f.server.state.lock().unwrap();
        assert_eq!(state.generated, 1);
        assert_eq!(state.files.len(), 1);
    }
}
#[tokio::test]
async fn occupied_remote_destination_and_mutated_snapshots_preserve_user_bytes() {
    let f = Fixture::new(
        vec![],
        Faults {
            occupied: true,
            ..Default::default()
        },
    );
    std::fs::write(f.root.join("source.bin"), b"abc").unwrap();
    assert_eq!(
        f.operator()
            .upload(f.upload("occupied"))
            .await
            .unwrap_err()
            .kind,
        OperatorError::DestinationOccupied
    );
    assert_eq!(f.server.state.lock().unwrap().started, 0);
    let f = Fixture::new(
        vec![],
        Faults {
            fail_chunk: true,
            ..Default::default()
        },
    );
    std::fs::write(f.root.join("source.bin"), vec![1; 2 * 1024 * 1024]).unwrap();
    let request = f.upload("tampered");
    assert!(f.operator().upload(request.clone()).await.is_err());
    std::fs::write(
        f.root.join(".drivesync/transfers/tampered.part"),
        b"tampered",
    )
    .unwrap();
    let count = f.server.state.lock().unwrap().chunks.len();
    assert!(f.operator().upload(request).await.is_err());
    assert_eq!(f.server.state.lock().unwrap().chunks.len(), count);
}

struct Cancel(AtomicBool);
impl CancellationSignal for Cancel {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
struct Reporter {
    callback: Box<dyn Fn(ExecutionProgress) + Send + Sync>,
}
impl ProgressReporter for Reporter {
    fn try_report(&self, value: ExecutionProgress) -> ProgressDelivery {
        (self.callback)(value);
        ProgressDelivery::Dropped
    }
}
#[tokio::test]
async fn cancellation_preserves_unpublished_downloads_and_upload_session_for_retry() {
    for upload in [false, true] {
        let f = Fixture::new(vec![2; 2 * 1024 * 1024 + 3], Faults::default());
        let cancel = Arc::new(Cancel(AtomicBool::new(false)));
        let stopped = cancel.clone();
        let reporter = Arc::new(Reporter {
            callback: Box::new(move |progress| {
                if progress.phase == ExecutionPhase::Transferring
                    && progress.bytes_transferred.unwrap_or(0) > 0
                {
                    stopped.0.store(true, Ordering::SeqCst);
                }
            }),
        });
        if upload {
            std::fs::write(f.root.join("source.bin"), vec![2; 2 * 1024 * 1024 + 3]).unwrap();
            let mut request = f.upload("cancel-upload");
            request.options = ExecutionOptions {
                cancellation: Some(cancel.clone()),
                progress: Some(reporter),
            };
            let error = f.operator().upload(request.clone()).await.unwrap_err();
            assert_eq!(error.kind, OperatorError::Cancelled);
            assert_eq!(error.effect, ExecutionEffect::Unknown);
            request.options = ExecutionOptions::default();
            f.operator().upload(request).await.unwrap();
            assert_eq!(f.server.state.lock().unwrap().generated, 1);
        } else {
            let mut request = f.download("cancel-download");
            request.options = ExecutionOptions {
                cancellation: Some(cancel.clone()),
                progress: Some(reporter),
            };
            let error = f.operator().download(request.clone()).await.unwrap_err();
            assert_eq!(error.kind, OperatorError::Cancelled);
            assert_eq!(error.effect, ExecutionEffect::NotApplied);
            assert!(!f.root.join("download.bin").exists());
            request.options = ExecutionOptions::default();
            f.operator().download(request).await.unwrap();
        }
    }
}
#[tokio::test]
async fn cancellation_at_atomic_cutover_does_not_publish_and_retry_uses_verified_stage() {
    let f = Fixture::new(b"abc".to_vec(), Faults::default());
    let cancel = Arc::new(Cancel(AtomicBool::new(false)));
    let stopped = cancel.clone();
    let mut request = f.download("cutover");
    request.options = ExecutionOptions {
        cancellation: Some(cancel),
        progress: Some(Arc::new(Reporter {
            callback: Box::new(move |p| {
                if p.phase == ExecutionPhase::Applying {
                    stopped.0.store(true, Ordering::SeqCst);
                }
            }),
        })),
    };
    let error = f.operator().download(request.clone()).await.unwrap_err();
    assert_eq!(error.kind, OperatorError::Cancelled);
    assert_eq!(error.effect, ExecutionEffect::NotApplied);
    assert!(!f.root.join("download.bin").exists());
    request.options = ExecutionOptions::default();
    let result = f.operator().download(request).await.unwrap();
    assert_eq!(result.bytes_transferred, 0);
    assert_eq!(std::fs::read(f.root.join("download.bin")).unwrap(), b"abc");
}
#[tokio::test]
async fn published_effect_without_database_confirmation_recovers_without_another_transfer() {
    for upload in [false, true] {
        let f = Fixture::new(b"abc".to_vec(), Faults::default());
        let sql = rusqlite::Connection::open(f.root.join(".drivesync/inventory.sqlite3")).unwrap();
        sql.execute_batch("CREATE TRIGGER fail_completed BEFORE UPDATE ON execution_checkpoints WHEN NEW.phase='\"Completed\"' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        if upload {
            std::fs::write(f.root.join("source.bin"), b"abc").unwrap();
            let request = f.upload("confirm-upload");
            let error = f.operator().upload(request.clone()).await.unwrap_err();
            assert_eq!(error.effect, ExecutionEffect::Applied);
            assert_eq!(f.server.state.lock().unwrap().files.len(), 1);
            sql.execute_batch("DROP TRIGGER fail_completed").unwrap();
            let chunks = f.server.state.lock().unwrap().chunks.len();
            assert_eq!(
                f.operator()
                    .upload(request)
                    .await
                    .unwrap()
                    .bytes_transferred,
                0
            );
            assert_eq!(f.server.state.lock().unwrap().chunks.len(), chunks);
        } else {
            let request = f.download("confirm-download");
            let error = f.operator().download(request.clone()).await.unwrap_err();
            assert_eq!(error.effect, ExecutionEffect::Applied);
            assert_eq!(std::fs::read(f.root.join("download.bin")).unwrap(), b"abc");
            sql.execute_batch("DROP TRIGGER fail_completed").unwrap();
            let count = f.server.state.lock().unwrap().requests.len();
            assert_eq!(
                f.operator()
                    .download(request)
                    .await
                    .unwrap()
                    .bytes_transferred,
                0
            );
            assert_eq!(f.server.state.lock().unwrap().requests.len(), count);
        }
    }
}
#[tokio::test]
async fn destination_inserted_at_cutover_is_preserved_and_completed_download_is_not_recreated() {
    let f = Fixture::new(b"abc".to_vec(), Faults::default());
    let path = f.root.join("download.bin");
    let mut request = f.download("racing");
    request.options.progress = Some(Arc::new(Reporter {
        callback: Box::new(move |p| {
            if p.phase == ExecutionPhase::Applying {
                std::fs::write(&path, b"foreign").unwrap();
            }
        }),
    }));
    assert_eq!(
        f.operator().download(request).await.unwrap_err().kind,
        OperatorError::DestinationOccupied
    );
    assert_eq!(
        std::fs::read(f.root.join("download.bin")).unwrap(),
        b"foreign"
    );
    let f = Fixture::new(b"abc".to_vec(), Faults::default());
    let request = f.download("completed");
    f.operator().download(request.clone()).await.unwrap();
    std::fs::rename(f.root.join("download.bin"), f.root.join("user-moved.bin")).unwrap();
    assert!(f.operator().download(request).await.is_err());
    assert!(!f.root.join("download.bin").exists());
    assert_eq!(
        std::fs::read(f.root.join("user-moved.bin")).unwrap(),
        b"abc"
    );
}
#[tokio::test]
async fn source_writer_resuming_after_first_chunk_prevents_final_remote_publication() {
    let f = Fixture::new(vec![], Faults::default());
    let path = f.root.join("source.bin");
    std::fs::write(&path, vec![3; 2 * 1024 * 1024 + 3]).unwrap();
    let mut request = f.upload("writer");
    request.options.progress = Some(Arc::new(Reporter {
        callback: Box::new(move |p| {
            if p.phase == ExecutionPhase::Transferring {
                std::fs::write(&path, b"new user content").unwrap();
            }
        }),
    }));
    let error = f.operator().upload(request).await.unwrap_err();
    assert_eq!(error.kind, OperatorError::PreconditionFailed);
    assert!(f.server.state.lock().unwrap().files.is_empty());
    assert_eq!(
        std::fs::read(f.root.join("source.bin")).unwrap(),
        b"new user content"
    );
}
#[tokio::test]
async fn forged_offsets_and_foreign_accounts_never_send_additional_content() {
    let f = Fixture::new(
        vec![],
        Faults {
            corrupt_ack: true,
            ..Default::default()
        },
    );
    std::fs::write(f.root.join("source.bin"), vec![3; 2 * 1024 * 1024 + 3]).unwrap();
    assert!(f.operator().upload(f.upload("offset")).await.is_err());
    assert_eq!(f.server.state.lock().unwrap().chunks.len(), 1);
    let f = Fixture::new(b"abc".to_vec(), Faults::default());
    let mut request = f.download("foreign");
    let ResourceRef::Remote { reference, .. } = &mut request.source else {
        unreachable!()
    };
    reference.account_id = AccountId("other".into());
    assert!(f.operator().download(request).await.is_err());
    assert!(f.server.state.lock().unwrap().requests.is_empty());
}
#[tokio::test]
async fn download_parent_symlink_and_reserved_paths_are_rejected_before_http() {
    for name in ["escape", ".drivesync"] {
        let f = Fixture::new(b"abc".to_vec(), Faults::default());
        if name == "escape" {
            std::os::unix::fs::symlink("/tmp", f.root.join(name)).unwrap();
        }
        let mut request = f.download("scope");
        request.destination = DirectoryRef::Local {
            relative_path: LocalRelativePath::new(name.as_bytes().to_vec()).unwrap(),
            local_id: None,
        };
        assert!(f.operator().download(request).await.is_err());
        assert!(f.server.state.lock().unwrap().requests.is_empty());
    }
}
#[tokio::test]
async fn permanent_delete_refuses_without_journal_or_http_and_unimplemented_mutations_are_explicit()
{
    let f = Fixture::new(vec![], Faults::default());
    let target = ResourceRef::Local {
        relative_path: LocalRelativePath::new(b"missing".to_vec()).unwrap(),
        local_id: None,
    };
    let context = ExecutionContext {
        id: ExecutionId::new("delete".into()).unwrap(),
        source: None,
    };
    let error = f
        .operator()
        .delete(DeleteRequest {
            execution: context,
            target,
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.kind, OperatorError::PermanentDeletionProhibited);
    assert_eq!(error.effect, ExecutionEffect::NotApplied);
    assert!(f.server.state.lock().unwrap().requests.is_empty());
    let sql = rusqlite::Connection::open(f.root.join(".drivesync/inventory.sqlite3")).unwrap();
    assert_eq!(
        sql.query_row("SELECT count(*) FROM execution_intents", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn sessions_completed_before_response_loss_are_not_recreated_after_user_moves_remote_file() {
    let f = Fixture::new(
        vec![],
        Faults {
            lose_final: true,
            ..Default::default()
        },
    );
    std::fs::write(f.root.join("source.bin"), b"abc").unwrap();
    let request = f.upload("moved");
    assert!(f.operator().upload(request.clone()).await.is_err());
    f.server
        .state
        .lock()
        .unwrap()
        .files
        .get_mut("generated-1")
        .unwrap()["name"] = json!("user-renamed.bin");
    assert!(f.operator().upload(request).await.is_err());
    assert_eq!(f.server.state.lock().unwrap().started, 1);
    assert_eq!(f.server.state.lock().unwrap().generated, 1);
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    struct Wake(std::thread::Thread);
    impl std::task::Wake for Wake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = std::task::Waker::from(Arc::new(Wake(std::thread::current())));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => return value,
            std::task::Poll::Pending => std::thread::park(),
        }
    }
}
#[test]
fn transfer_futures_work_without_a_caller_tokio_runtime() {
    let f = Fixture::new(b"abc".to_vec(), Faults::default());
    let operator = f.operator();
    let result = block_on(operator.download(f.download("plain-future"))).unwrap();
    assert_eq!(result.bytes_transferred, 3);
}

/// Invoked only by the parent process crash test below. In normal test runs this
/// is inert. No desktop accounts, environment credentials or real endpoints.
#[test]
fn crash_transfer_child() {
    use std::os::unix::ffi::OsStringExt;
    let Some(path) = std::env::var_os("DRIVESYNC_OPERATOR_CRASH_INTENT") else {
        return;
    };
    let intent: ExecutionIntent = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let base = std::env::var("DRIVESYNC_OPERATOR_CRASH_BASE").unwrap();
    let phase = std::env::var("DRIVESYNC_OPERATOR_CRASH_PHASE").unwrap();
    let scope = OperatorScope {
        sync_id: intent.scope.sync_id.clone(),
        account_id: intent.scope.account_id.clone(),
        local_root: PathBuf::from(std::ffi::OsString::from_vec(
            intent.scope.local_root.clone(),
        )),
        remote_root_id: intent.scope.remote_root_id.clone(),
    };
    let inventory = Arc::new(
        InventoryManager::open(OpenInventoryRequest {
            local_root: scope.local_root.clone(),
            expected: SyncIdentity {
                sync_id: scope.sync_id.clone(),
                account_id: scope.account_id.clone(),
                remote_root_id: scope.remote_root_id.0.clone(),
            },
            create_if_missing: false,
        })
        .unwrap()
        .inventory,
    );
    let client = g::GoogleDriveClient::with_base_urls(
        format!("{base}/drive/v3"),
        format!("{base}/upload/drive/v3"),
    )
    .unwrap();
    let operator = GoogleOperator::testing(scope, inventory, client);
    let options = ExecutionOptions {
        cancellation: None,
        progress: Some(Arc::new(Reporter {
            callback: Box::new(move |p| {
                let stop = match phase.as_str() {
                    "before-publish" => p.phase == ExecutionPhase::Applying,
                    "after-publish" => p.phase == ExecutionPhase::Finalizing,
                    _ => {
                        p.phase == ExecutionPhase::Transferring
                            && p.bytes_transferred.unwrap_or(0) > 0
                    }
                };
                if stop {
                    std::process::exit(73);
                }
            }),
        })),
    };
    let result = match intent.command {
        OperationCommand::Download {
            source,
            destination,
            name,
        } => block_on(operator.download(DownloadRequest {
            execution: intent.context,
            source,
            destination,
            name,
            options,
        })),
        OperationCommand::Upload {
            source,
            destination,
            name,
        } => block_on(operator.upload(UploadRequest {
            execution: intent.context,
            source,
            destination,
            name,
            options,
        })),
        _ => panic!("unexpected synthetic crash command"),
    };
    panic!("crash injection was not reached: {result:?}");
}
#[test]
fn process_exit_at_transfer_and_publication_boundaries_recovers_the_same_execution() {
    for (upload, phase) in [
        (false, "before-publish"),
        (false, "after-publish"),
        (true, "after-chunk"),
        (true, "after-publish"),
    ] {
        let bytes = vec![6; 2 * 1024 * 1024 + 3];
        let f = Fixture::new(bytes.clone(), Faults::default());
        std::fs::write(f.root.join("source.bin"), &bytes).unwrap();
        let download = f.download("crash");
        let request = f.upload("crash");
        let intent = ExecutionIntent {
            scope: ExecutionScope::from(&f.scope()),
            context: if upload {
                request.execution.clone()
            } else {
                download.execution.clone()
            },
            command: if upload {
                OperationCommand::Upload {
                    source: request.source.clone(),
                    destination: request.destination.clone(),
                    name: request.name.clone(),
                }
            } else {
                OperationCommand::Download {
                    source: download.source.clone(),
                    destination: download.destination.clone(),
                    name: download.name.clone(),
                }
            },
        };
        let file = f.root.join(".drivesync/synthetic-crash-intent.json");
        std::fs::write(&file, serde_json::to_vec(&intent).unwrap()).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "operations::google::tests::crash_transfer_child",
                "--test-threads=1",
            ])
            .env("DRIVESYNC_OPERATOR_CRASH_INTENT", &file)
            .env("DRIVESYNC_OPERATOR_CRASH_BASE", &f.server.base)
            .env("DRIVESYNC_OPERATOR_CRASH_PHASE", phase)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(73), "{upload} {phase}");
        let operator = f.operator();
        if upload {
            let result = block_on(operator.upload(request)).unwrap();
            assert_eq!(result.integrity, IntegrityStatus::Verified);
            let state = f.server.state.lock().unwrap();
            assert_eq!(state.generated, 1);
            assert_eq!(state.files.len(), 1);
            if phase == "after-chunk" {
                assert_eq!(state.chunks[1], 1024 * 1024);
            } else {
                assert_eq!(result.bytes_transferred, 0);
            }
        } else {
            let result = block_on(operator.download(download)).unwrap();
            assert_eq!(result.bytes_transferred, 0);
            assert_eq!(std::fs::read(f.root.join("download.bin")).unwrap(), bytes);
        }
    }
}

#[tokio::test]
async fn network_unavailability_is_bounded_and_exact_retry_preserves_source_and_identity() {
    for upload in [false, true] {
        let f = Fixture::new(
            b"abc".to_vec(),
            Faults {
                metadata_failure: Some(503),
                ..Default::default()
            },
        );
        std::fs::write(f.root.join("source.bin"), b"abc").unwrap();
        let download = f.download("offline");
        let request = f.upload("offline");
        let operator = f.operator();
        let error = if upload {
            operator.upload(request.clone()).await.unwrap_err()
        } else {
            operator.download(download.clone()).await.unwrap_err()
        };
        assert_eq!(error.kind, OperatorError::Network);
        assert_eq!(error.effect, ExecutionEffect::NotApplied);
        assert!(!format!("{error:?}").contains("private-server-detail"));
        assert_eq!(f.server.state.lock().unwrap().requests.len(), 1);
        if upload {
            operator.upload(request).await.unwrap();
            assert_eq!(f.server.state.lock().unwrap().generated, 1);
        } else {
            operator.download(download).await.unwrap();
        }
    }
}
#[tokio::test]
async fn md5_only_upload_metadata_is_verified_against_the_snapshot() {
    let f = Fixture::new(
        vec![],
        Faults {
            omit_sha: true,
            ..Default::default()
        },
    );
    std::fs::write(f.root.join("source.bin"), b"abc").unwrap();
    assert_eq!(
        f.operator()
            .upload(f.upload("md5-upload"))
            .await
            .unwrap()
            .integrity,
        IntegrityStatus::Verified
    );
}

#[tokio::test]
async fn a_distinct_upload_intent_does_not_adopt_or_replace_an_existing_homonym() {
    let f = Fixture::new(vec![], Faults::default());
    std::fs::write(f.root.join("source.bin"), b"abc").unwrap();
    let first = f.operator().upload(f.upload("first-intent")).await.unwrap();
    let error = f
        .operator()
        .upload(f.upload("second-intent"))
        .await
        .unwrap_err();
    assert_eq!(error.kind, OperatorError::DestinationOccupied);
    let state = f.server.state.lock().unwrap();
    assert_eq!(state.generated, 1);
    assert_eq!(state.files.len(), 1);
    assert!(
        matches!(first.item.resource,ResourceRef::Remote {reference,..} if reference.file_id.0=="generated-1")
    );
}
#[tokio::test]
async fn replacing_the_download_parent_at_cutover_is_detected_before_publication() {
    let f = Fixture::new(b"abc".to_vec(), Faults::default());
    std::fs::create_dir(f.root.join("parent")).unwrap();
    let root = f.root.clone();
    let mut request = f.download("parent-swap");
    request.destination = DirectoryRef::Local {
        relative_path: LocalRelativePath::new(b"parent".to_vec()).unwrap(),
        local_id: None,
    };
    request.options.progress = Some(Arc::new(Reporter {
        callback: Box::new(move |p| {
            if p.phase == ExecutionPhase::Applying {
                std::fs::rename(root.join("parent"), root.join("moved-parent")).unwrap();
                std::fs::create_dir(root.join("parent")).unwrap();
            }
        }),
    }));
    let error = f.operator().download(request).await.unwrap_err();
    assert_eq!(error.kind, OperatorError::WrongScope);
    assert_eq!(error.effect, ExecutionEffect::NotApplied);
    assert!(!f.root.join("parent/download.bin").exists());
    assert!(!f.root.join("moved-parent/download.bin").exists());
}

#[tokio::test]
async fn lost_verifying_checkpoint_after_confirmed_upload_reports_applied() {
    let f = Fixture::new(vec![], Faults::default());
    std::fs::write(f.root.join("source.bin"), b"abc").unwrap();
    let request = f.upload("verifying-save");
    let sql = rusqlite::Connection::open(f.root.join(".drivesync/inventory.sqlite3")).unwrap();
    sql.execute_batch("CREATE TRIGGER fail_verifying BEFORE UPDATE ON execution_checkpoints WHEN NEW.phase='\"Verifying\"' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    let error = f.operator().upload(request.clone()).await.unwrap_err();
    assert_eq!(error.effect, ExecutionEffect::Applied);
    assert_eq!(f.server.state.lock().unwrap().files.len(), 1);
    sql.execute_batch("DROP TRIGGER fail_verifying").unwrap();
    assert_eq!(
        f.operator()
            .upload(request)
            .await
            .unwrap()
            .bytes_transferred,
        0
    );
    assert_eq!(f.server.state.lock().unwrap().files.len(), 1);
}
