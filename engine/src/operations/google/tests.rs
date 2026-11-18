//! Exercises the public Operator with real temporary files/SQLite and the real
//! Google provider against a stateful loopback HTTP server. No personal accounts.
use super::*;
use crate::inventory::*;
use md5::{Digest, Md5};
use serde_json::{json, Value};
use sha2::Sha256;
use std::os::unix::ffi::OsStrExt;
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
    lose_metadata: bool,
    stale_metadata: bool,
    race_on_chunk: bool,
    omit_etag: bool,
    occupied: bool,
    omit_sha: bool,
    corrupt_ack: bool,
    no_download: bool,
    metadata_failure: Option<u16>,
    fail_confirmation: bool,
}
struct Session {
    id: String,
    name: String,
    bytes: Vec<u8>,
    version: String,
    parent: String,
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
    let mut result = json!({"id":id,"name":name,"mimeType":"application/octet-stream","parents":["root"],"trashed":false,"version":version,"size":bytes.len().to_string(),"md5Checksum":format!("{:x}",Md5::digest(bytes)),"capabilities":{"canDownload":true,"canEdit":true,"canRename":true,"canMoveItemWithinDrive":true,"canTrash":true,"canUntrash":true}});
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
    } else if method == "PATCH" && path.starts_with("/drive/v2/files/") {
        let id = path.rsplit('/').next().unwrap();
        let tag = header.lines().find_map(|line| {
            line.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("if-match"))
                .map(|(_, v)| v.trim())
        });
        let stale = state.faults.stale_metadata;
        let file = state.files.get_mut(id).unwrap();
        let expected = format!("\"v{}\"", file["version"].as_str().unwrap());
        if stale || tag != Some(expected.as_str()) {
            code = 412;
            json!({"error":{"code":412}})
        } else {
            let mut patch: Value = serde_json::from_slice(bytes).unwrap();
            if let Some(title) = patch.get("title").cloned() {
                patch["name"] = title;
            }
            if let Some(trashed) = patch.get("labels").and_then(|v| v.get("trashed")).cloned() {
                patch["trashed"] = trashed;
            }
            for key in ["name", "trashed"] {
                if let Some(value) = patch.get(key) {
                    file[key] = value.clone();
                }
            }
            if let Some(parent) = query.get("addParents") {
                file["parents"] = json!([parent]);
            }
            let v = file["version"].as_str().unwrap().parse::<u64>().unwrap() + 1;
            file["version"] = json!(v.to_string());
            let result = file.clone();
            if state.faults.lose_metadata {
                state.faults.lose_metadata = false;
                return;
            }
            result
        }
    } else if method == "POST" && path == "/drive/v3/files" {
        let mut metadata: Value = serde_json::from_slice(bytes).unwrap();
        let id = metadata["id"].as_str().unwrap().to_owned();
        metadata["trashed"] = json!(false);
        metadata["version"] = json!("1");
        metadata["capabilities"] = json!({"canAddChildren":true});
        state.files.insert(id, metadata.clone());
        metadata
    } else if (method == "POST" && path == "/upload/drive/v3/files")
        || (method == "PUT" && path.starts_with("/upload/drive/v2/files/"))
    {
        let metadata: Value = serde_json::from_slice(bytes).unwrap();
        let replacing = method == "PUT";
        let id = if replacing {
            path.rsplit('/').next().unwrap().to_owned()
        } else {
            metadata["id"].as_str().unwrap().to_owned()
        };
        let old = state.files.get(&id).cloned();
        let tag = header.lines().find_map(|line| {
            line.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("if-match"))
                .map(|(_, v)| v.trim())
        });
        let etag = old
            .as_ref()
            .map(|f| format!("\"v{}\"", f["version"].as_str().unwrap()));
        if replacing && (state.faults.stale_metadata || tag != etag.as_deref()) {
            code = 412;
            json!({"error":{"code":412}})
        } else if !replacing && old.is_some() {
            code = 409;
            json!({"error":{"code":409}})
        } else {
            state.started += 1;
            let token = format!("session-{}", state.started);
            let (name, version, parent) = if let Some(old) = old {
                (
                    old["name"].as_str().unwrap().to_owned(),
                    (old["version"].as_str().unwrap().parse::<u64>().unwrap() + 1).to_string(),
                    old["parents"][0].as_str().unwrap().to_owned(),
                )
            } else {
                (
                    metadata["name"].as_str().unwrap().to_owned(),
                    "1".into(),
                    metadata["parents"][0].as_str().unwrap().to_owned(),
                )
            };
            state.sessions.insert(
                token.clone(),
                Session {
                    id,
                    name,
                    bytes: vec![],
                    version,
                    parent,
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
        let tag = header.lines().find_map(|line| {
            line.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("if-match"))
                .map(|(_, v)| v.trim())
        });
        if let Some(tag) = tag {
            let id = state.sessions.get(token).map(|s| s.id.clone());
            if let Some(id) = id {
                if state.faults.race_on_chunk {
                    state.faults.race_on_chunk = false;
                    state.files.insert(
                        id.clone(),
                        blob(&id, "target.bin", b"foreign edit", "2", false),
                    );
                }
                let etag = state
                    .files
                    .get(&id)
                    .map(|f| format!("\"v{}\"", f["version"].as_str().unwrap()));
                if etag.as_deref() != Some(tag) {
                    respond(&mut stream, 412, &[], br#"{"error":{"code":412}}"#, None);
                    return;
                }
            }
        }
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
                    let version = session.version.clone();
                    let parent = session.parent.clone();
                    state.chunks.push(offset as u64);
                    if content.len() == total {
                        let mut file = blob(&id, &name, &content, &version, state.faults.omit_sha);
                        file["parents"] = json!([parent]);
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
    if method == "GET" && response.get("version").is_some() && !state.faults.omit_etag {
        headers.push((
            "ETag",
            format!("\"v{}\"", response["version"].as_str().unwrap()),
        ));
    }
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

#[tokio::test]
async fn directory_creation_is_identity_bound_recoverable_and_never_merges_homonyms() {
    let f = Fixture::new(vec![], Faults::default());
    let operator = f.operator();
    let r = CreateDirectoryRequest {
        execution: ExecutionContext {
            id: ExecutionId::new("local-directory".into()).unwrap(),
            source: None,
        },
        parent: DirectoryRef::LocalRoot,
        name: ItemName::Local("folder".into()),
        options: ExecutionOptions::default(),
    };
    let result = operator.create_directory(r.clone()).await.unwrap();
    assert!(f.root.join("folder").is_dir());
    assert_eq!(
        result.resource,
        operator.create_directory(r.clone()).await.unwrap().resource
    );
    let mut other = r.clone();
    other.execution.id = ExecutionId::new("other-directory".into()).unwrap();
    assert_eq!(
        operator.create_directory(other).await.unwrap_err().kind,
        OperatorError::DestinationOccupied
    );
    let remote = CreateDirectoryRequest {
        execution: ExecutionContext {
            id: ExecutionId::new("remote-directory".into()).unwrap(),
            source: None,
        },
        parent: f.upload_parent(),
        name: ItemName::Remote("folder".into()),
        options: ExecutionOptions::default(),
    };
    let result = operator.create_directory(remote.clone()).await.unwrap();
    assert_eq!(
        result.resource,
        operator.create_directory(remote).await.unwrap().resource
    );
    assert_eq!(f.server.state.lock().unwrap().generated, 1);
}
impl Fixture {
    fn upload_parent(&self) -> DirectoryRef {
        DirectoryRef::Remote {
            provider_id: ProviderId("google".into()),
            reference: RemoteFileRef {
                account_id: AccountId("account".into()),
                file_id: RemoteFileId("root".into()),
            },
        }
    }
}

#[derive(Default)]
struct Events(
    Mutex<Vec<crate::analyzer::CreationEvent>>,
    Option<Arc<Mutex<State>>>,
);
impl crate::analyzer::CreationObserver for Events {
    fn notify(&self, event: crate::analyzer::CreationEvent) {
        if event.stage == crate::analyzer::CreationStage::Verifying {
            if let Some(state) = &self.1 {
                let mut state = state.lock().unwrap();
                if state.faults.fail_confirmation {
                    state.faults.fail_confirmation = false;
                    state.faults.metadata_failure = Some(503);
                }
            }
        }
        self.0.lock().unwrap().push(event);
    }
}

/// Exercise Analyzer decisions, real Operator/Provider HTTP, SQLite claims,
/// physical replicas and baseline confirmation together, without personal data.
#[tokio::test]
async fn analyzer_dispatches_download_and_upload_and_confirms_verified_baselines() {
    use crate::analyzer::*;
    for (upload, directory, interrupted) in [
        (false, false, false),
        (true, false, false),
        (false, false, true),
        (true, false, true),
        (false, true, false),
        (true, true, false),
    ] {
        let f = Fixture::new(
            b"verified contents".to_vec(),
            Faults {
                truncate_download: interrupted && !upload,
                lose_final: interrupted && upload,
                ..Default::default()
            },
        );
        let mut data = if upload {
            std::fs::write(f.root.join("source.bin"), b"verified contents").unwrap();
            let Some(SourceEvidence::Local { metadata, .. }) = f.upload("unused").execution.source
            else {
                panic!()
            };
            FileData::Local(*metadata)
        } else {
            let Some(SourceEvidence::Remote { metadata, .. }) =
                f.download("unused").execution.source
            else {
                panic!()
            };
            FileData::Remote(*metadata)
        };
        if directory {
            match &mut data {
                FileData::Local(l) => {
                    std::fs::create_dir(f.root.join("folder")).unwrap();
                    *l = fs::observe(
                        &std::fs::File::open(f.root.join("folder")).unwrap(),
                        LocalRelativePath::new(b"folder".to_vec()).unwrap(),
                    )
                    .unwrap();
                }
                FileData::Remote(r) => {
                    r.reference.file_id = RemoteFileId("directory-source".into());
                    r.name = MetadataValue::Known("folder".into());
                    r.kind = MetadataValue::Known(RemoteEntryKind::Directory);
                    r.mime_type = MetadataValue::Known(FOLDER.into());
                    r.size = MetadataValue::NotApplicable;
                    r.md5_checksum = MetadataValue::NotApplicable;
                    r.sha256_checksum = MetadataValue::NotApplicable;
                    f.server.state.lock().unwrap().files.insert("directory-source".into(),json!({"id":"directory-source","name":"folder","mimeType":FOLDER,"parents":["root"],"trashed":false,"version":"1","capabilities":{"canAddChildren":true}}));
                }
            }
        }
        let at = Timestamp { unix_millis: 100 };
        let location = match &data {
            FileData::Local(l) => FileLocation::Local(l.relative_path.clone()),
            FileData::Remote(r) => FileLocation::Remote {
                reference: r.reference.clone(),
                parent_id: Some(RemoteFileId("root".into())),
                name: if directory {
                    Some("folder".into())
                } else {
                    Some("source.bin".into())
                },
            },
        };
        let file = crate::models::File {
            id: crate::models::FileId("observed".into()),
            sync_id: SyncId("sync".into()),
            observed_at: at,
            scope: match &data {
                FileData::Local(l) => FileScope::Local(l.relative_path.clone()),
                FileData::Remote(r) => FileScope::Remote(r.reference.clone()),
            },
            coverage: ObservationCoverage::Partial,
            event: crate::models::FileEvent {
                action: if upload {
                    FileEventAction::Modified
                } else {
                    FileEventAction::StateChanged
                },
                local: if upload {
                    Some(crate::models::LocalObservation {
                        capture_id: "test".into(),
                        mask: libc::IN_MODIFY,
                        cookie: 0,
                    })
                } else {
                    None
                },
                remote: if upload {
                    None
                } else {
                    Some(crate::models::RemoteObservation {
                        source: RemoteObservationSource::Changes,
                        removed: Some(false),
                        time: None,
                        change_type: None,
                        drive_id: None,
                    })
                },
                uncertainty: None,
                previous: None,
                current: Some(location),
            },
            data: Some(data),
        };
        let proof = ObservationProof {
            source: if upload {
                ObservationSource::LocalEvent
            } else {
                ObservationSource::RemoteChange
            },
            observed_at: at,
            scan_or_generation: None,
            complete_scope: false,
        };
        let item = if upload {
            f.inventory
                .record_local(RecordLocalObservation {
                    selector: ObservationTarget::NewItem,
                    expected_revision: None,
                    observation: crate::inventory::LocalObservation {
                        observed: file,
                        presence: PresenceState::Present,
                        proof,
                    },
                })
                .unwrap()
                .item
        } else {
            f.inventory
                .record_remote(RecordRemoteObservation {
                    selector: ObservationTarget::NewItem,
                    expected_revision: None,
                    observation: crate::inventory::RemoteObservation {
                        observed: file,
                        presence: PresenceState::Present,
                        proof,
                    },
                })
                .unwrap()
                .item
        };
        let events = Arc::new(Events(Mutex::new(vec![]), Some(f.server.state.clone())));
        let analyzer = Analyzer::new(AnalyzerConfig {
            provider: ProviderId("google".into()),
            identity: SyncIdentity {
                sync_id: SyncId("sync".into()),
                account_id: AccountId("account".into()),
                remote_root_id: "root".into(),
            },
            dependencies: AnalyzerDependencies {
                inventory: f.inventory.clone(),
                execution: Some(AnalyzerExecution {
                    operator: Arc::new(f.operator()),
                    options: ExecutionOptions::default(),
                    observer: events.clone(),
                }),
            },
        })
        .unwrap();
        let evidence = AnalysisEvidence {
            baseline_confirmed: false,
            local_scope_complete: upload,
            remote_scope_complete: !upload,
            local_content: EvidenceRelation::Unknown,
            remote_content: EvidenceRelation::Unknown,
            current_content: EvidenceRelation::Unknown,
            local_parent: None,
            current_parents: EvidenceRelation::Unknown,
        };
        analyzer
            .analyze(AnalyzeRequest {
                observations: Vec::new(),
                directive: None,
                sync_id: SyncId("sync".into()),
                item: item.clone(),
                evidence: evidence.clone(),
                recorded_at: at,
            })
            .await
            .unwrap();
        if interrupted {
            let binding = f
                .inventory
                .get_creation(GetCreationRequest {
                    item_id: item.id.clone(),
                })
                .unwrap()
                .unwrap();
            assert_eq!(
                binding.operation.state,
                OperationState::RetryWait,
                "upload={upload}, operation={:?}, events={:?}",
                binding.operation,
                events.0.lock().unwrap()
            );
            assert!(f
                .inventory
                .get_item(GetItemRequest {
                    selector: ItemSelector::Id(item.id.clone())
                })
                .unwrap()
                .item
                .unwrap()
                .baseline
                .is_none());
            let next = binding.operation.next_attempt_at.unwrap();
            assert!(f
                .inventory
                .due_creations(DueCreationsRequest { at, limit: 64 })
                .unwrap()
                .is_empty());
            assert_eq!(
                f.inventory
                    .due_creations(DueCreationsRequest {
                        at: next,
                        limit: 64
                    })
                    .unwrap(),
                vec![item.id.clone()]
            );
            analyzer
                .analyze(AnalyzeRequest {
                    observations: Vec::new(),
                    directive: None,
                    sync_id: SyncId("sync".into()),
                    item: f
                        .inventory
                        .get_item(GetItemRequest {
                            selector: ItemSelector::Id(item.id.clone()),
                        })
                        .unwrap()
                        .item
                        .unwrap(),
                    evidence: evidence.clone(),
                    recorded_at: next,
                })
                .await
                .unwrap();
            assert_eq!(
                f.inventory
                    .get_creation(GetCreationRequest {
                        item_id: item.id.clone()
                    })
                    .unwrap()
                    .unwrap()
                    .intent
                    .context
                    .id,
                binding.intent.context.id
            );
        }
        let current = f
            .inventory
            .get_item(GetItemRequest {
                selector: ItemSelector::Id(item.id.clone()),
            })
            .unwrap()
            .item
            .unwrap();
        assert_eq!(
            current.status,
            SyncStatus::Synced,
            "events: {:?}",
            events.0.lock().unwrap()
        );
        assert!(current.baseline.is_some());
        assert_eq!(
            f.inventory
                .get_creation(GetCreationRequest {
                    item_id: item.id.clone()
                })
                .unwrap()
                .unwrap()
                .operation
                .state,
            OperationState::Completed
        );
        assert_eq!(
            events.0.lock().unwrap().last().unwrap().stage,
            CreationStage::Completed
        );
        let started = f.server.state.lock().unwrap().started;
        analyzer
            .analyze(AnalyzeRequest {
                observations: Vec::new(),
                directive: None,
                sync_id: SyncId("sync".into()),
                item: current.clone(),
                evidence: AnalysisEvidence {
                    baseline_confirmed: true,
                    local_scope_complete: true,
                    remote_scope_complete: true,
                    local_content: if directory {
                        EvidenceRelation::Unknown
                    } else {
                        EvidenceRelation::Equal
                    },
                    remote_content: if directory {
                        EvidenceRelation::Unknown
                    } else {
                        EvidenceRelation::Equal
                    },
                    current_content: if directory {
                        EvidenceRelation::Unknown
                    } else {
                        EvidenceRelation::Equal
                    },
                    local_parent: None,
                    current_parents: EvidenceRelation::Equal,
                },
                recorded_at: Timestamp {
                    unix_millis: current.baseline.as_ref().unwrap().confirmed_at.unix_millis + 1,
                },
            })
            .await
            .unwrap();
        assert_eq!(
            f.inventory
                .get_item(GetItemRequest {
                    selector: ItemSelector::Id(item.id.clone())
                })
                .unwrap()
                .item
                .unwrap()
                .status,
            SyncStatus::Synced
        );
        assert_eq!(f.server.state.lock().unwrap().started, started);

        if upload && !directory && !interrupted {
            for (event_id, name, contents_changed) in [
                ("renamed", "renamed.bin", false),
                ("edited", "renamed.bin", true),
                ("combined", "combined.bin", true),
            ] {
                if event_id == "combined" {
                    std::fs::rename(f.root.join("renamed.bin"), f.root.join(name)).unwrap();
                }
                if contents_changed {
                    std::fs::write(f.root.join(name), format!("updated by analyzer {event_id}"))
                        .unwrap();
                } else {
                    std::fs::rename(f.root.join("source.bin"), f.root.join(name)).unwrap();
                }
                let current = f
                    .inventory
                    .get_item(GetItemRequest {
                        selector: ItemSelector::Id(item.id.clone()),
                    })
                    .unwrap()
                    .item
                    .unwrap();
                let path = LocalRelativePath::new(name.as_bytes().to_vec()).unwrap();
                let metadata = fs::observe(
                    &std::fs::File::open(f.root.join(name)).unwrap(),
                    path.clone(),
                )
                .unwrap();
                let at = Timestamp {
                    unix_millis: current.baseline.as_ref().unwrap().confirmed_at.unix_millis + 1,
                };
                let observed = crate::models::File {
                    id: crate::models::FileId(event_id.into()),
                    sync_id: SyncId("sync".into()),
                    observed_at: at,
                    scope: FileScope::Local(path.clone()),
                    coverage: ObservationCoverage::Partial,
                    event: crate::models::FileEvent {
                        action: FileEventAction::Modified,
                        local: Some(crate::models::LocalObservation {
                            capture_id: "mutation".into(),
                            mask: libc::IN_MODIFY,
                            cookie: 0,
                        }),
                        remote: None,
                        uncertainty: None,
                        previous: None,
                        current: Some(FileLocation::Local(path)),
                    },
                    data: Some(FileData::Local(metadata)),
                };
                let updated = f
                    .inventory
                    .record_local(RecordLocalObservation {
                        selector: ObservationTarget::Existing(item.id.clone()),
                        expected_revision: Some(current.revision),
                        observation: crate::inventory::LocalObservation {
                            observed,
                            presence: PresenceState::Present,
                            proof: ObservationProof {
                                source: ObservationSource::LocalEvent,
                                observed_at: at,
                                scan_or_generation: None,
                                complete_scope: false,
                            },
                        },
                    })
                    .unwrap()
                    .item;
                if event_id == "renamed" {
                    f.server.state.lock().unwrap().faults.fail_confirmation = true;
                }
                analyzer
                    .analyze(AnalyzeRequest {
                        observations: Vec::new(),
                        directive: None,
                        sync_id: SyncId("sync".into()),
                        item: updated,
                        evidence: AnalysisEvidence {
                            baseline_confirmed: true,
                            local_scope_complete: true,
                            remote_scope_complete: true,
                            local_content: if contents_changed {
                                EvidenceRelation::Different
                            } else {
                                EvidenceRelation::Equal
                            },
                            remote_content: EvidenceRelation::Equal,
                            current_content: if contents_changed {
                                EvidenceRelation::Different
                            } else {
                                EvidenceRelation::Equal
                            },
                            local_parent: None,
                            current_parents: EvidenceRelation::Equal,
                        },
                        recorded_at: at,
                    })
                    .await
                    .unwrap();
                let updated = f
                    .inventory
                    .get_item(GetItemRequest {
                        selector: ItemSelector::Id(item.id.clone()),
                    })
                    .unwrap()
                    .item
                    .unwrap();
                let updated = if event_id == "renamed" {
                    let binding = f
                        .inventory
                        .get_creation(GetCreationRequest {
                            item_id: item.id.clone(),
                        })
                        .unwrap()
                        .unwrap();
                    assert_eq!(binding.operation.state, OperationState::RetryWait);
                    // Exercise the real backoff clock instead of passing a future
                    // start timestamp that predates the subsequent confirmation.
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as i64;
                    let delay = binding
                        .operation
                        .next_attempt_at
                        .as_ref()
                        .unwrap()
                        .unix_millis
                        .saturating_sub(now)
                        .max(0) as u64;
                    tokio::time::sleep(Duration::from_millis(delay + 10)).await;
                    let patches = f
                        .server
                        .state
                        .lock()
                        .unwrap()
                        .requests
                        .iter()
                        .filter(|(method, _)| method == "PATCH")
                        .count();
                    analyzer
                        .analyze(AnalyzeRequest {
                            observations: Vec::new(),
                            directive: None,
                            sync_id: SyncId("sync".into()),
                            item: updated,
                            evidence: AnalysisEvidence {
                                baseline_confirmed: true,
                                local_scope_complete: true,
                                remote_scope_complete: true,
                                local_content: EvidenceRelation::Equal,
                                remote_content: EvidenceRelation::Equal,
                                current_content: EvidenceRelation::Equal,
                                local_parent: None,
                                current_parents: EvidenceRelation::Equal,
                            },
                            recorded_at: binding.operation.next_attempt_at.unwrap(),
                        })
                        .await
                        .unwrap();
                    assert_eq!(
                        f.server
                            .state
                            .lock()
                            .unwrap()
                            .requests
                            .iter()
                            .filter(|(method, _)| method == "PATCH")
                            .count(),
                        patches
                    );
                    f.inventory
                        .get_item(GetItemRequest {
                            selector: ItemSelector::Id(item.id.clone()),
                        })
                        .unwrap()
                        .item
                        .unwrap()
                } else {
                    updated
                };
                let updated = if event_id == "combined" {
                    assert_eq!(updated.status, SyncStatus::PendingReconciliation);
                    let previous_baseline = updated.baseline.clone();
                    analyzer
                        .analyze(AnalyzeRequest {
                            observations: Vec::new(),
                            directive: None,
                            sync_id: SyncId("sync".into()),
                            item: updated,
                            evidence: AnalysisEvidence {
                                baseline_confirmed: true,
                                local_scope_complete: true,
                                remote_scope_complete: true,
                                local_content: EvidenceRelation::Different,
                                remote_content: EvidenceRelation::Different,
                                current_content: EvidenceRelation::Equal,
                                local_parent: None,
                                current_parents: EvidenceRelation::Equal,
                            },
                            recorded_at: Timestamp {
                                unix_millis: at.unix_millis + 1,
                            },
                        })
                        .await
                        .unwrap();
                    let updated = f
                        .inventory
                        .get_item(GetItemRequest {
                            selector: ItemSelector::Id(item.id.clone()),
                        })
                        .unwrap()
                        .item
                        .unwrap();
                    assert_ne!(updated.baseline, previous_baseline);
                    updated
                } else {
                    updated
                };
                assert_eq!(
                    updated.status,
                    SyncStatus::Synced,
                    "mutation {event_id}: {:?}",
                    events.0.lock().unwrap()
                );
                let binding = f
                    .inventory
                    .get_creation(GetCreationRequest {
                        item_id: item.id.clone(),
                    })
                    .unwrap()
                    .unwrap();
                assert_eq!(binding.operation.state, OperationState::Completed);
                assert_eq!(
                    binding.operation.kind,
                    if contents_changed && event_id != "combined" {
                        OperationKind::UpdateContent
                    } else {
                        OperationKind::Rename
                    }
                );
            }
        }
        if !upload && !directory {
            assert_eq!(
                std::fs::read(f.root.join("source.bin")).unwrap(),
                b"verified contents"
            );
        }
    }
}

fn context_for(key: &str, metadata: FileData) -> ExecutionContext {
    ExecutionContext {
        id: ExecutionId::new(key.into()).unwrap(),
        source: Some(match metadata {
            FileData::Local(metadata) => SourceEvidence::Local {
                metadata: Box::new(metadata),
                sha256: None,
            },
            FileData::Remote(metadata) => SourceEvidence::Remote {
                metadata: Box::new(metadata),
                revision: None,
            },
        }),
    }
}
fn local_item(f: &Fixture, path: &str) -> ItemResult {
    let path = LocalRelativePath::new(path.as_bytes().to_vec()).unwrap();
    let metadata = fs::observe(
        &std::fs::File::open(f.root.join(std::ffi::OsStr::from_bytes(path.as_bytes()))).unwrap(),
        path.clone(),
    )
    .unwrap();
    ItemResult {
        resource: ResourceRef::Local {
            relative_path: path,
            local_id: metadata.local_id.clone(),
        },
        metadata: Some(FileData::Local(metadata)),
        verification: VerificationStatus::Verified,
    }
}
#[tokio::test]
async fn local_files_and_directories_rename_move_trash_restore_without_replacement() {
    for directory in [false, true] {
        let f = Fixture::new(vec![], Faults::default());
        if directory {
            std::fs::create_dir(f.root.join("original")).unwrap();
            std::fs::write(f.root.join("original/child"), b"keep").unwrap();
        } else {
            std::fs::write(f.root.join("original"), b"keep").unwrap();
        }
        std::fs::create_dir(f.root.join("destination")).unwrap();
        let original = local_item(&f, "original");
        let operator = f.operator();
        let rename = RenameRequest {
            execution: context_for("rename", original.metadata.clone().unwrap()),
            target: original.resource.clone(),
            new_name: ItemName::Local("renamed".into()),
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        };
        let renamed = operator.rename(rename.clone()).await.unwrap();
        assert_eq!(
            operator.rename(rename.clone()).await.unwrap().resource,
            renamed.resource
        );
        let moved = operator
            .move_item(MoveRequest {
                execution: context_for("move", renamed.metadata.clone().unwrap()),
                target: renamed.resource.clone(),
                destination: DirectoryRef::Local {
                    relative_path: LocalRelativePath::new(b"destination".to_vec()).unwrap(),
                    local_id: None,
                },
                preconditions: Preconditions::default(),
                options: ExecutionOptions::default(),
            })
            .await
            .unwrap();
        let trash = TrashRequest {
            execution: context_for("trash", moved.metadata.clone().unwrap()),
            target: moved.resource.clone(),
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        };
        let trashed = operator.trash(trash.clone()).await.unwrap();
        assert!(!f.root.join("destination/renamed").exists());
        assert_eq!(
            operator.trash(trash).await.unwrap().resource,
            trashed.resource
        );
        let restore = RestoreRequest {
            execution: context_for("restore", trashed.metadata.clone().unwrap()),
            target: trashed.resource.clone(),
            destination: None,
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        };
        let restored = operator.restore(restore.clone()).await.unwrap();
        assert_eq!(
            operator.restore(restore).await.unwrap().resource,
            restored.resource
        );
        assert_eq!(restored.resource, moved.resource);
        assert_eq!(
            std::fs::read(f.root.join(if directory {
                "destination/renamed/child"
            } else {
                "destination/renamed"
            }))
            .unwrap(),
            b"keep"
        );
        assert!(operator.rename(rename).await.is_err());
        assert!(f.server.state.lock().unwrap().requests.is_empty());
    }
}
#[tokio::test]
async fn local_mutations_reject_collisions_stale_metadata_and_directory_cycles() {
    let f = Fixture::new(vec![], Faults::default());
    std::fs::write(f.root.join("source"), b"old").unwrap();
    std::fs::write(f.root.join("occupied"), b"other").unwrap();
    let item = local_item(&f, "source");
    let rename = RenameRequest {
        execution: context_for("collision", item.metadata.clone().unwrap()),
        target: item.resource.clone(),
        new_name: ItemName::Local("occupied".into()),
        preconditions: Preconditions::default(),
        options: ExecutionOptions::default(),
    };
    assert_eq!(
        f.operator().rename(rename).await.unwrap_err().kind,
        OperatorError::DestinationOccupied
    );
    std::fs::write(f.root.join("source"), b"changed").unwrap();
    assert_eq!(
        f.operator()
            .trash(TrashRequest {
                execution: context_for("stale", item.metadata.unwrap()),
                target: item.resource,
                preconditions: Preconditions::default(),
                options: ExecutionOptions::default()
            })
            .await
            .unwrap_err()
            .kind,
        OperatorError::PreconditionFailed
    );
    std::fs::create_dir_all(f.root.join("tree/child")).unwrap();
    let item = local_item(&f, "tree");
    assert_eq!(
        f.operator()
            .move_item(MoveRequest {
                execution: context_for("cycle", item.metadata.unwrap()),
                target: item.resource,
                destination: DirectoryRef::Local {
                    relative_path: LocalRelativePath::new(b"tree/child".to_vec()).unwrap(),
                    local_id: None
                },
                preconditions: Preconditions::default(),
                options: ExecutionOptions::default()
            })
            .await
            .unwrap_err()
            .kind,
        OperatorError::InvalidRequest
    );
    assert_eq!(std::fs::read(f.root.join("occupied")).unwrap(), b"other");
}
fn remote_item(f: &Fixture) -> ItemResult {
    let Some(SourceEvidence::Remote { mut metadata, .. }) = f.download("unused").execution.source
    else {
        panic!()
    };
    metadata.reference.file_id = RemoteFileId("target".into());
    metadata.name = MetadataValue::Known("target.bin".into());
    f.server.state.lock().unwrap().files.insert(
        "target".into(),
        blob("target", "target.bin", &[], "1", false),
    );
    ItemResult {
        resource: ResourceRef::Remote {
            provider_id: ProviderId("google".into()),
            reference: metadata.reference.clone(),
        },
        metadata: Some(FileData::Remote(*metadata)),
        verification: VerificationStatus::Verified,
    }
}
#[tokio::test]
async fn remote_metadata_mutations_recover_lost_responses_and_preserve_identity() {
    let f = Fixture::new(
        vec![],
        Faults {
            lose_metadata: true,
            ..Default::default()
        },
    );
    let item = remote_item(&f);
    let r = RenameRequest {
        execution: context_for("remote-rename", item.metadata.unwrap()),
        target: item.resource.clone(),
        new_name: ItemName::Remote("renamed.bin".into()),
        preconditions: Preconditions {
            expected_version: Some("1".into()),
            expected_revision: None,
        },
        options: ExecutionOptions::default(),
    };
    assert_eq!(
        f.operator().rename(r.clone()).await.unwrap_err().kind,
        OperatorError::Network
    );
    let renamed = f.operator().rename(r).await.unwrap();
    assert_eq!(renamed.resource, item.resource);
    let trash = TrashRequest {
        execution: context_for("remote-trash", renamed.metadata.unwrap()),
        target: renamed.resource,
        preconditions: Preconditions::default(),
        options: ExecutionOptions::default(),
    };
    let trashed = f.operator().trash(trash.clone()).await.unwrap();
    assert_eq!(
        f.operator().trash(trash).await.unwrap().resource,
        trashed.resource
    );
    let restored = f
        .operator()
        .restore(RestoreRequest {
            execution: context_for("remote-restore", trashed.metadata.unwrap()),
            target: trashed.resource,
            destination: None,
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    assert_eq!(restored.resource, item.resource);
    assert_eq!(
        f.server
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .filter(|(method, _)| method == "PATCH")
            .count(),
        3
    );
}
#[tokio::test]
async fn remote_metadata_mutation_cannot_override_a_stale_etag() {
    let f = Fixture::new(
        vec![],
        Faults {
            stale_metadata: true,
            ..Default::default()
        },
    );
    let item = remote_item(&f);
    assert_eq!(
        f.operator()
            .rename(RenameRequest {
                execution: context_for("conditional", item.metadata.unwrap()),
                target: item.resource,
                new_name: ItemName::Remote("new".into()),
                preconditions: Preconditions::default(),
                options: ExecutionOptions::default()
            })
            .await
            .unwrap_err()
            .kind,
        OperatorError::PreconditionFailed
    );
    assert_eq!(
        f.server.state.lock().unwrap().files["target"]["name"],
        "target.bin"
    );
}
#[tokio::test]
async fn creates_files_from_empty_inline_local_and_remote_content_in_both_environments() {
    for remote in [false, true] {
        for content_kind in 0..4 {
            let f = Fixture::new(b"contents".to_vec(), Faults::default());
            std::fs::write(f.root.join("source.bin"), b"contents").unwrap();
            let (content, source) = match content_kind {
                0 => (None, None),
                1 => (
                    Some(ContentSource::Bytes {
                        data: b"contents".to_vec(),
                    }),
                    None,
                ),
                2 => {
                    let request = f.upload("unused");
                    let ResourceRef::Local {
                        relative_path,
                        local_id,
                    } = request.source
                    else {
                        panic!()
                    };
                    (
                        Some(ContentSource::LocalFile {
                            relative_path,
                            local_id,
                        }),
                        request.execution.source,
                    )
                }
                _ => {
                    let request = f.download("unused");
                    (
                        Some(ContentSource::RemoteFile {
                            resource: request.source,
                        }),
                        request.execution.source,
                    )
                }
            };
            let request = CreateFileRequest {
                execution: ExecutionContext {
                    id: ExecutionId::new("create-file".into()).unwrap(),
                    source,
                },
                parent: if remote {
                    f.upload_parent()
                } else {
                    DirectoryRef::LocalRoot
                },
                name: if remote {
                    ItemName::Remote("created.bin".into())
                } else {
                    ItemName::Local("created.bin".into())
                },
                content,
                options: ExecutionOptions::default(),
            };
            let result = f.operator().create_file(request.clone()).await.unwrap();
            assert_eq!(
                result.resource,
                f.operator().create_file(request).await.unwrap().resource
            );
            let expected = if content_kind == 0 {
                &b""[..]
            } else {
                &b"contents"[..]
            };
            if remote {
                let state = f.server.state.lock().unwrap();
                assert_eq!(state.generated, 1);
                assert_eq!(state.sessions.values().next().unwrap().bytes, expected);
            } else {
                assert_eq!(std::fs::read(f.root.join("created.bin")).unwrap(), expected);
            }
        }
    }
}
#[tokio::test]
async fn local_content_replacement_preserves_old_bytes_and_never_replays_completed_work() {
    let f = Fixture::new(vec![], Faults::default());
    std::fs::write(f.root.join("target"), b"old").unwrap();
    let item = local_item(&f, "target");
    let Some(FileData::Local(metadata)) = item.metadata else {
        panic!()
    };
    let r = UpdateContentRequest {
        execution: ExecutionContext {
            id: ExecutionId::new("replace".into()).unwrap(),
            source: None,
        },
        target: item.resource,
        source: ContentSource::Bytes {
            data: b"new contents".to_vec(),
        },
        preconditions: Preconditions {
            expected_version: None,
            expected_revision: Some(local_revision(&metadata).unwrap()),
        },
        options: ExecutionOptions::default(),
    };
    let result = f.operator().update_content(r.clone()).await.unwrap();
    assert_eq!(
        std::fs::read(f.root.join("target")).unwrap(),
        b"new contents"
    );
    assert_eq!(
        std::fs::read(f.root.join(".drivesync/transfers/replace.part")).unwrap(),
        b"old"
    );
    assert_eq!(
        f.operator()
            .update_content(r.clone())
            .await
            .unwrap()
            .item
            .resource,
        result.item.resource
    );
    std::fs::write(f.root.join("target"), b"user edit").unwrap();
    assert!(f.operator().update_content(r).await.is_err());
    assert_eq!(std::fs::read(f.root.join("target")).unwrap(), b"user edit");
}
#[tokio::test]
async fn remote_replacement_keeps_target_id_and_recovers_lost_final_response() {
    let f = Fixture::new(
        vec![],
        Faults {
            lose_final: true,
            ..Default::default()
        },
    );
    let item = remote_item(&f);
    let r = UpdateContentRequest {
        execution: ExecutionContext {
            id: ExecutionId::new("replace-remote".into()).unwrap(),
            source: None,
        },
        target: item.resource.clone(),
        source: ContentSource::Bytes {
            data: b"updated bytes".to_vec(),
        },
        preconditions: Preconditions {
            expected_version: Some("1".into()),
            expected_revision: None,
        },
        options: ExecutionOptions::default(),
    };
    assert_eq!(
        f.operator()
            .update_content(r.clone())
            .await
            .unwrap_err()
            .kind,
        OperatorError::Network
    );
    assert_eq!(
        f.operator()
            .update_content(r.clone())
            .await
            .unwrap()
            .item
            .resource,
        item.resource
    );
    assert_eq!(
        f.operator()
            .update_content(r)
            .await
            .unwrap()
            .bytes_transferred,
        0
    );
    let state = f.server.state.lock().unwrap();
    assert_eq!(state.generated, 0);
    assert_eq!(state.started, 1);
    assert_eq!(state.files.len(), 1);
    assert_eq!(
        state.sessions.values().next().unwrap().bytes,
        b"updated bytes"
    );
}
#[tokio::test]
async fn remote_move_preserves_bytes_and_rejects_foreign_accounts_and_collisions() {
    let f = Fixture::new(vec![], Faults::default());
    let item = remote_item(&f);
    f.server.state.lock().unwrap().files.insert("folder".into(),json!({"id":"folder","name":"folder","mimeType":FOLDER,"parents":["root"],"version":"1","trashed":false,"capabilities":{"canAddChildren":true}}));
    let destination = DirectoryRef::Remote {
        provider_id: ProviderId("google".into()),
        reference: RemoteFileRef {
            account_id: AccountId("account".into()),
            file_id: RemoteFileId("folder".into()),
        },
    };
    let r = MoveRequest {
        execution: context_for("move-remote", item.metadata.unwrap()),
        target: item.resource.clone(),
        destination,
        preconditions: Preconditions::default(),
        options: ExecutionOptions::default(),
    };
    let result = f.operator().move_item(r.clone()).await.unwrap();
    assert_eq!(result.resource, item.resource);
    assert_eq!(
        f.operator().move_item(r).await.unwrap().resource,
        item.resource
    );
    assert_eq!(
        f.server.state.lock().unwrap().files["target"]["parents"],
        json!(["folder"])
    );
    assert!(f.server.state.lock().unwrap().sessions.is_empty());
}
#[tokio::test]
async fn replacements_accept_each_content_source_and_resume_expired_sessions() {
    for remote in [false, true] {
        for source_kind in 0..3 {
            let bytes = vec![5; 2 * 1024 * 1024 + 3];
            let f = Fixture::new(
                bytes.clone(),
                Faults {
                    fail_chunk: remote,
                    expire: remote,
                    ..Default::default()
                },
            );
            std::fs::write(f.root.join("source.bin"), &bytes).unwrap();
            std::fs::write(f.root.join("target"), b"old").unwrap();
            let item = if remote {
                remote_item(&f)
            } else {
                local_item(&f, "target")
            };
            // The target is independent of the remote content fixture.
            if remote {
                let mut state = f.server.state.lock().unwrap();
                state.files.insert(
                    "target".into(),
                    blob("target", "target.bin", b"old", "1", false),
                );
            }
            let (source, evidence) = match source_kind {
                0 => (
                    ContentSource::Bytes {
                        data: bytes.clone(),
                    },
                    None,
                ),
                1 => {
                    let u = f.upload("unused");
                    let ResourceRef::Local {
                        relative_path,
                        local_id,
                    } = u.source
                    else {
                        panic!()
                    };
                    (
                        ContentSource::LocalFile {
                            relative_path,
                            local_id,
                        },
                        u.execution.source,
                    )
                }
                _ => {
                    let d = f.download("unused");
                    (
                        ContentSource::RemoteFile { resource: d.source },
                        d.execution.source,
                    )
                }
            };
            let preconditions = if remote {
                Preconditions {
                    expected_version: Some("1".into()),
                    expected_revision: None,
                }
            } else {
                let Some(FileData::Local(m)) = &item.metadata else {
                    panic!()
                };
                Preconditions {
                    expected_version: None,
                    expected_revision: Some(local_revision(m).unwrap()),
                }
            };
            let r = UpdateContentRequest {
                execution: ExecutionContext {
                    id: ExecutionId::new("replacement-sources".into()).unwrap(),
                    source: evidence,
                },
                target: item.resource,
                source,
                preconditions,
                options: ExecutionOptions::default(),
            };
            if remote {
                assert!(f.operator().update_content(r.clone()).await.is_err());
            }
            f.operator().update_content(r.clone()).await.unwrap();
            assert_eq!(
                f.operator()
                    .update_content(r)
                    .await
                    .unwrap()
                    .bytes_transferred,
                0
            );
            if remote {
                let state = f.server.state.lock().unwrap();
                assert_eq!(state.sessions.values().next().unwrap().bytes, bytes);
                assert_eq!(state.started, 2);
            } else {
                assert_eq!(std::fs::read(f.root.join("target")).unwrap(), bytes);
                assert_eq!(
                    std::fs::read(f.root.join(".drivesync/transfers/replacement-sources.part"))
                        .unwrap(),
                    b"old"
                );
            }
        }
    }
}
#[tokio::test]
async fn remote_replacement_rejects_a_concurrent_edit_before_empty_put() {
    let f = Fixture::new(
        vec![],
        Faults {
            race_on_chunk: true,
            ..Default::default()
        },
    );
    let item = remote_item(&f);
    let r = UpdateContentRequest {
        execution: ExecutionContext {
            id: ExecutionId::new("conditional-race".into()).unwrap(),
            source: None,
        },
        target: item.resource,
        source: ContentSource::Bytes { data: vec![] },
        preconditions: Preconditions {
            expected_version: Some("1".into()),
            expected_revision: None,
        },
        options: ExecutionOptions::default(),
    };
    assert_eq!(
        f.operator().update_content(r).await.unwrap_err().kind,
        OperatorError::PreconditionFailed
    );
    let state = f.server.state.lock().unwrap();
    assert_eq!(state.files["target"]["size"], "12");
    assert_eq!(state.files["target"]["version"], "2");
}
#[tokio::test]
async fn metadata_mutations_never_accept_forged_remote_parent_evidence() {
    let f = Fixture::new(vec![], Faults::default());
    let item = remote_item(&f);
    f.server
        .state
        .lock()
        .unwrap()
        .files
        .get_mut("target")
        .unwrap()["parents"] = json!(["outside"]);
    let error = f
        .operator()
        .trash(TrashRequest {
            execution: context_for("forged-parent", item.metadata.unwrap()),
            target: item.resource,
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.kind, OperatorError::WrongScope);
    assert!(!f
        .server
        .state
        .lock()
        .unwrap()
        .requests
        .iter()
        .any(|(m, _)| m == "PATCH"));
}
#[tokio::test]
async fn local_rename_trash_and_replacement_recover_after_confirmation_failure() {
    for action in ["rename", "trash", "replace"] {
        let f = Fixture::new(vec![], Faults::default());
        std::fs::write(f.root.join("original"), b"old bytes").unwrap();
        let item = local_item(&f, "original");
        let sql = rusqlite::Connection::open(f.root.join(".drivesync/inventory.sqlite3")).unwrap();
        sql.execute_batch("CREATE TRIGGER fail_completed BEFORE UPDATE ON execution_checkpoints WHEN NEW.phase='\"Completed\"' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        let operator = f.operator();
        let result = if action == "rename" {
            operator
                .rename(RenameRequest {
                    execution: context_for("recover", item.metadata.clone().unwrap()),
                    target: item.resource.clone(),
                    new_name: ItemName::Local("renamed".into()),
                    preconditions: Preconditions::default(),
                    options: ExecutionOptions::default(),
                })
                .await
                .map(|r| r.resource)
        } else if action == "trash" {
            operator
                .trash(TrashRequest {
                    execution: context_for("recover", item.metadata.clone().unwrap()),
                    target: item.resource.clone(),
                    preconditions: Preconditions::default(),
                    options: ExecutionOptions::default(),
                })
                .await
                .map(|r| r.resource)
        } else {
            let Some(FileData::Local(metadata)) = &item.metadata else {
                panic!()
            };
            operator
                .update_content(UpdateContentRequest {
                    execution: ExecutionContext {
                        id: ExecutionId::new("recover".into()).unwrap(),
                        source: None,
                    },
                    target: item.resource.clone(),
                    source: ContentSource::Bytes {
                        data: b"new bytes".to_vec(),
                    },
                    preconditions: Preconditions {
                        expected_version: None,
                        expected_revision: Some(local_revision(metadata).unwrap()),
                    },
                    options: ExecutionOptions::default(),
                })
                .await
                .map(|r| r.item.resource)
        };
        assert_eq!(result.unwrap_err().effect, ExecutionEffect::Applied);
        sql.execute_batch("DROP TRIGGER fail_completed").unwrap();
        if action == "rename" {
            operator
                .rename(RenameRequest {
                    execution: context_for("recover", item.metadata.clone().unwrap()),
                    target: item.resource.clone(),
                    new_name: ItemName::Local("renamed".into()),
                    preconditions: Preconditions::default(),
                    options: ExecutionOptions::default(),
                })
                .await
                .unwrap();
            assert_eq!(std::fs::read(f.root.join("renamed")).unwrap(), b"old bytes");
        } else if action == "trash" {
            operator
                .trash(TrashRequest {
                    execution: context_for("recover", item.metadata.clone().unwrap()),
                    target: item.resource.clone(),
                    preconditions: Preconditions::default(),
                    options: ExecutionOptions::default(),
                })
                .await
                .unwrap();
            assert_eq!(
                std::fs::read(f.root.join(".drivesync/trash/recover/item")).unwrap(),
                b"old bytes"
            );
        } else {
            let Some(FileData::Local(metadata)) = &item.metadata else {
                panic!()
            };
            operator
                .update_content(UpdateContentRequest {
                    execution: ExecutionContext {
                        id: ExecutionId::new("recover".into()).unwrap(),
                        source: None,
                    },
                    target: item.resource.clone(),
                    source: ContentSource::Bytes {
                        data: b"new bytes".to_vec(),
                    },
                    preconditions: Preconditions {
                        expected_version: None,
                        expected_revision: Some(local_revision(metadata).unwrap()),
                    },
                    options: ExecutionOptions::default(),
                })
                .await
                .unwrap();
            assert_eq!(
                std::fs::read(f.root.join("original")).unwrap(),
                b"new bytes"
            );
            assert_eq!(
                std::fs::read(f.root.join(".drivesync/transfers/recover.part")).unwrap(),
                b"old bytes"
            );
        }
    }
}
#[tokio::test]
async fn mutations_without_a_server_validator_never_fall_back_to_unconditional_writes() {
    for update in [false, true] {
        let f = Fixture::new(
            vec![],
            Faults {
                omit_etag: true,
                ..Default::default()
            },
        );
        let item = remote_item(&f);
        let error = if update {
            f.operator()
                .update_content(UpdateContentRequest {
                    execution: ExecutionContext {
                        id: ExecutionId::new("missing-etag".into()).unwrap(),
                        source: None,
                    },
                    target: item.resource,
                    source: ContentSource::Bytes {
                        data: b"new".to_vec(),
                    },
                    preconditions: Preconditions {
                        expected_version: Some("1".into()),
                        expected_revision: None,
                    },
                    options: ExecutionOptions::default(),
                })
                .await
                .unwrap_err()
        } else {
            f.operator()
                .rename(RenameRequest {
                    execution: context_for("missing-etag", item.metadata.unwrap()),
                    target: item.resource,
                    new_name: ItemName::Remote("new".into()),
                    preconditions: Preconditions::default(),
                    options: ExecutionOptions::default(),
                })
                .await
                .unwrap_err()
        };
        assert_eq!(error.kind, OperatorError::UnsupportedOperation);
        assert!(!f
            .server
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .any(|(method, _)| method == "PATCH" || method == "PUT"));
        assert_eq!(
            f.server.state.lock().unwrap().files["target"]["name"],
            "target.bin"
        );
    }
}
#[tokio::test]
async fn replacement_cannot_turn_a_directory_into_a_file() {
    let f = Fixture::new(vec![], Faults::default());
    std::fs::create_dir(f.root.join("target")).unwrap();
    std::fs::write(f.root.join("target/child"), b"keep").unwrap();
    let item = local_item(&f, "target");
    let Some(FileData::Local(metadata)) = &item.metadata else {
        panic!()
    };
    let error = f
        .operator()
        .update_content(UpdateContentRequest {
            execution: ExecutionContext {
                id: ExecutionId::new("directory-update".into()).unwrap(),
                source: None,
            },
            target: item.resource,
            source: ContentSource::Bytes {
                data: b"bad".to_vec(),
            },
            preconditions: Preconditions {
                expected_version: None,
                expected_revision: Some(local_revision(metadata).unwrap()),
            },
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.kind, OperatorError::InvalidRequest);
    assert_eq!(std::fs::read(f.root.join("target/child")).unwrap(), b"keep");
}
#[tokio::test]
async fn cancelled_retry_preserves_an_already_applied_effect_and_identity() {
    let f = Fixture::new(vec![], Faults::default());
    let mut r = CreateFileRequest {
        execution: ExecutionContext {
            id: ExecutionId::new("cancel-completed".into()).unwrap(),
            source: None,
        },
        parent: DirectoryRef::LocalRoot,
        name: ItemName::Local("created".into()),
        content: None,
        options: ExecutionOptions::default(),
    };
    let item = f.operator().create_file(r.clone()).await.unwrap();
    r.options.cancellation = Some(Arc::new(Cancel(AtomicBool::new(true))));
    let error = f.operator().create_file(r).await.unwrap_err();
    assert_eq!(error.kind, OperatorError::Cancelled);
    assert_eq!(error.effect, ExecutionEffect::Applied);
    assert_eq!(error.resource, Some(item.resource));
    assert!(f.root.join("created").is_file());
    assert!(f.server.state.lock().unwrap().requests.is_empty());
}
#[tokio::test]
async fn metadata_mutations_honor_the_explicit_source_revision() {
    let f = Fixture::new(vec![], Faults::default());
    let item = remote_item(&f);
    let mut execution = context_for("source-revision", item.metadata.unwrap());
    let Some(SourceEvidence::Remote { revision, .. }) = &mut execution.source else {
        panic!()
    };
    *revision = Some("not-current".into());
    let error = f
        .operator()
        .rename(RenameRequest {
            execution,
            target: item.resource,
            new_name: ItemName::Remote("new".into()),
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.kind, OperatorError::PreconditionFailed);
    assert!(!f
        .server
        .state
        .lock()
        .unwrap()
        .requests
        .iter()
        .any(|(method, _)| method == "PATCH"));
}
#[tokio::test]
async fn contradictory_source_content_is_rejected_before_metadata_writes() {
    let f = Fixture::new(vec![], Faults::default());
    let item = remote_item(&f);
    f.server.state.lock().unwrap().files.insert(
        "target".into(),
        blob("target", "target.bin", b"different", "1", false),
    );
    let error = f
        .operator()
        .rename(RenameRequest {
            execution: context_for("source-content", item.metadata.unwrap()),
            target: item.resource,
            new_name: ItemName::Remote("new".into()),
            preconditions: Preconditions::default(),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.kind, OperatorError::PreconditionFailed);
    assert_eq!(error.effect, ExecutionEffect::NotApplied);
    assert!(!f
        .server
        .state
        .lock()
        .unwrap()
        .requests
        .iter()
        .any(|(method, _)| method == "PATCH"));
}

#[tokio::test]
async fn replica_verification_allows_version_advance_only_with_unchanged_content_and_location() {
    let f = Fixture::new(b"unchanged".to_vec(), Faults::default());
    let original = f.download("evidence").execution.source.unwrap();
    f.server.state.lock().unwrap().version = "3".into();
    let op = f.operator();
    let request = VerifySourceRequest {
        source: original.clone(),
        allow_version_advance: false,
        options: ExecutionOptions::default(),
    };
    assert_eq!(
        op.verify_source(request.clone()).await.unwrap_err().kind,
        OperatorError::PreconditionFailed
    );
    let mut request = request;
    request.allow_version_advance = true;
    let verified = op.verify_source(request.clone()).await.unwrap();
    let FileData::Remote(remote) = verified.metadata else {
        panic!()
    };
    assert_eq!(remote.version, MetadataValue::Known("3".into()));
    f.server.state.lock().unwrap().source = b"different".to_vec();
    assert_eq!(
        op.verify_source(request.clone()).await.unwrap_err().kind,
        OperatorError::PreconditionFailed
    );
    f.server.state.lock().unwrap().source = b"unchanged".to_vec();
    let SourceEvidence::Remote { metadata, .. } = &mut request.source else {
        panic!()
    };
    metadata.name = MetadataValue::Known("other.bin".into());
    assert_eq!(
        op.verify_source(request).await.unwrap_err().kind,
        OperatorError::PreconditionFailed
    );
}

#[tokio::test]
async fn directory_verification_preserves_identity_while_children_change() {
    let f = Fixture::new(vec![], Faults::default());
    std::fs::create_dir(f.root.join("folder")).unwrap();
    let path = LocalRelativePath::new(b"folder".to_vec()).unwrap();
    let metadata = fs::observe(&std::fs::File::open(f.root.join("folder")).unwrap(), path).unwrap();
    let request = VerifySourceRequest {
        source: SourceEvidence::Local {
            metadata: Box::new(metadata),
            sha256: None,
        },
        allow_version_advance: false,
        options: ExecutionOptions::default(),
    };
    std::fs::write(f.root.join("folder/child"), b"child").unwrap();
    f.operator().verify_source(request.clone()).await.unwrap();
    std::fs::rename(f.root.join("folder"), f.root.join("old-folder")).unwrap();
    std::fs::create_dir(f.root.join("folder")).unwrap();
    assert_eq!(
        f.operator().verify_source(request).await.unwrap_err().kind,
        OperatorError::PreconditionFailed
    );
}

#[tokio::test]
async fn directory_replica_version_can_advance_without_authorizing_a_move_or_rename() {
    let f = Fixture::new(vec![], Faults::default());
    let op = f.operator();
    let item = op
        .create_directory(CreateDirectoryRequest {
            execution: ExecutionContext {
                id: ExecutionId::new("directory-version".into()).unwrap(),
                source: None,
            },
            parent: DirectoryRef::Remote {
                provider_id: ProviderId("google".into()),
                reference: RemoteFileRef {
                    account_id: AccountId("account".into()),
                    file_id: RemoteFileId("root".into()),
                },
            },
            name: ItemName::Remote("folder".into()),
            options: ExecutionOptions::default(),
        })
        .await
        .unwrap();
    let ResourceRef::Remote { reference, .. } = &item.resource else {
        panic!()
    };
    f.server
        .state
        .lock()
        .unwrap()
        .files
        .get_mut(&reference.file_id.0)
        .unwrap()["version"] = json!("3");
    let request = VerifySourceRequest {
        source: match item.metadata.unwrap() {
            FileData::Remote(metadata) => SourceEvidence::Remote {
                metadata: Box::new(metadata),
                revision: None,
            },
            _ => panic!(),
        },
        allow_version_advance: true,
        options: ExecutionOptions::default(),
    };
    op.verify_source(request.clone()).await.unwrap();
    f.server
        .state
        .lock()
        .unwrap()
        .files
        .get_mut(&reference.file_id.0)
        .unwrap()["name"] = json!("other-folder");
    assert_eq!(
        op.verify_source(request).await.unwrap_err().kind,
        OperatorError::PreconditionFailed
    );
}
