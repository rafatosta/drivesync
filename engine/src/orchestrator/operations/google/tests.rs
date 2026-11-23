//! Loopback-only provider tests; authorization is synthetic and never stored.
use super::super::tests::{request, setup, Sandbox};
use super::*;
use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
pub(super) struct Endpoint {
    pub(super) base: String,
    pub(super) client: GoogleDriveClient,
    task: tokio::task::JoinHandle<()>,
    pub(super) requests: Arc<Mutex<Vec<String>>>,
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        self.task.abort();
    }
}
pub(super) async fn endpoint(
    handler: impl Fn(&str) -> (u16, String, Vec<(String, String)>) + Send + 'static,
) -> Endpoint {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let client = GoogleDriveClient::with_base_urls(
        format!("{base}/drive/v3"),
        format!("{base}/upload/drive/v3"),
    )
    .unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let task = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 8192];
            loop {
                let count = stream.read(&mut buffer).await.unwrap();
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..count]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]);
                    let length = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|s| s.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let request = String::from_utf8_lossy(&bytes).into_owned();
            captured.lock().unwrap().push(request.clone());
            let (status, body, headers) = handler(&request);
            let mut response = format!(
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n",
                body.len()
            );
            for (name, value) in headers {
                response.push_str(&format!("{name}: {value}\r\n"));
            }
            response.push_str("\r\n");
            response.push_str(&body);
            stream.write_all(response.as_bytes()).await.unwrap();
        }
    });
    Endpoint {
        base,
        client,
        task,
        requests,
    }
}
pub(super) struct Associations;
impl crate::accounts::AccountAssociations for Associations {
    fn lock_for_removal(
        &self,
        _: &AccountId,
    ) -> crate::accounts::AccountsResult<Box<dyn crate::accounts::RemovalGuard>> {
        unreachable!("test performs no account removal")
    }
}
pub(super) fn operator(s: &Sandbox, client: GoogleDriveClient) -> GoogleOperator {
    GoogleOperator::new(
        Accounts::new(crate::accounts::AccountsConfig::new(
            s.0.join("accounts"),
            "test.operations",
            Arc::new(Associations),
        ))
        .unwrap(),
        client,
    )
}
pub(super) fn metadata(name: &str, parent: &str, version: u64, content: &str) -> String {
    serde_json::json!({"id":"remote","name":name,"parents":[parent],"mimeType":"application/octet-stream","version":version.to_string(),"trashed":false,"size":content.len().to_string(),"sha256Checksum":format!("{:x}",Sha256::digest(content.as_bytes())),"capabilities":{"canEdit":true,"canRename":true,"canDownload":true,"canTrash":true,"canMoveItemWithinDrive":true}}).to_string()
}
pub(super) fn root() -> String {
    serde_json::json!({"id":"remote-root","name":"Root","parents":[],"mimeType":FOLDER,"version":"1","trashed":false,"capabilities":{"canAddChildren":true}}).to_string()
}
#[tokio::test]
async fn remote_rename_is_conditional_and_verified_by_readback() {
    let s = Sandbox::new();
    let executor = setup(&s);
    let changed = Arc::new(Mutex::new(false));
    let state = changed.clone();
    let server = endpoint(move |r| {
        if r.starts_with("PATCH ") {
            *state.lock().unwrap() = true;
            return (200, metadata("renamed", "remote-root", 2, "bytes"), vec![]);
        }
        let body = if r.contains("/files/remote-root?") {
            root()
        } else if r.contains("/files?") {
            "{\"files\":[],\"incompleteSearch\":false}".into()
        } else {
            metadata(
                if *state.lock().unwrap() {
                    "renamed"
                } else {
                    "file"
                },
                "remote-root",
                1,
                "bytes",
            )
        };
        (200, body, vec![("ETag".into(), "\"v1\"".into())])
    })
    .await;
    let op = operator(&s, server.client.clone());
    let r = request(&executor, OperationAction::RenameRemote);
    let result = op
        .run(
            &executor.config,
            &r,
            &executor.filesystem,
            &GoogleSession::new("test", "synthetic"),
        )
        .await
        .unwrap();
    assert!(result.verified);
    assert_eq!(
        result.remote.unwrap().name,
        Evidence::Known("renamed".into())
    );
    let calls = server.requests.lock().unwrap();
    let patch = calls.iter().find(|r| r.starts_with("PATCH ")).unwrap();
    assert!(patch.to_ascii_lowercase().contains("if-match: \"v1\""));
    assert!(patch.contains("\"title\":\"renamed\""));
}
#[tokio::test]
async fn conditional_rejection_has_no_unconditional_fallback() {
    let s = Sandbox::new();
    let executor = setup(&s);
    let server = endpoint(|r| {
        if r.starts_with("PATCH ") {
            return (412, "{}".into(), vec![]);
        }
        let body = if r.contains("/files/remote-root?") {
            root()
        } else if r.contains("/files?") {
            "{\"files\":[],\"incompleteSearch\":false}".into()
        } else {
            metadata("file", "remote-root", 1, "bytes")
        };
        (200, body, vec![("ETag".into(), "\"v1\"".into())])
    })
    .await;
    let result = operator(&s, server.client.clone())
        .run(
            &executor.config,
            &request(&executor, OperationAction::RenameRemote),
            &executor.filesystem,
            &GoogleSession::new("test", "synthetic"),
        )
        .await
        .unwrap_err();
    assert_eq!(result.effect, ExecutionEffect::NotApplied);
    assert_eq!(result.code, OperatorErrorKind::PreconditionFailed);
    assert_eq!(
        server
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with("PATCH "))
            .count(),
        1
    );
}
#[tokio::test]
async fn version_change_or_missing_validator_prevents_remote_mutation() {
    for (version, etag) in [(2, true), (1, false)] {
        let s = Sandbox::new();
        let executor = setup(&s);
        let server = endpoint(move |_| {
            (
                200,
                metadata("file", "remote-root", version, "bytes"),
                if etag {
                    vec![("ETag".into(), "\"v1\"".into())]
                } else {
                    vec![]
                },
            )
        })
        .await;
        assert!(operator(&s, server.client.clone())
            .run(
                &executor.config,
                &request(&executor, OperationAction::RenameRemote),
                &executor.filesystem,
                &GoogleSession::new("test", "synthetic")
            )
            .await
            .is_err());
        assert!(server
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|r| r.starts_with("GET ")));
    }
}
#[tokio::test]
async fn download_checks_digest_before_publishing_local_bytes() {
    for valid in [true, false] {
        let s = Sandbox::new();
        let executor = setup(&s);
        let server = endpoint(move |r| {
            let body = if r.contains("alt=media") {
                if valid { "bytes" } else { "wrong" }.into()
            } else {
                metadata("file", "remote-root", 1, "bytes")
            };
            (200, body, vec![])
        })
        .await;
        let result = operator(&s, server.client.clone())
            .run(
                &executor.config,
                &request(&executor, OperationAction::Download),
                &executor.filesystem,
                &GoogleSession::new("test", "synthetic"),
            )
            .await;
        if valid {
            assert!(result.unwrap().verified);
            assert_eq!(
                std::fs::read(executor.config.local_root.join("file")).unwrap(),
                b"bytes"
            );
        } else {
            assert!(result.is_err());
            assert!(!executor.config.local_root.join("file").exists());
        }
    }
}
#[tokio::test]
async fn remote_destination_collision_and_outside_scope_are_rejected() {
    for collision in [true, false] {
        let s = Sandbox::new();
        let executor = setup(&s);
        let server = endpoint(move |r| {
            let body = if r.contains("/files/remote-root?") {
                root()
            } else if r.contains("/files?") {
                serde_json::json!({"files":[{"id":"occupied"}],"incompleteSearch":false})
                    .to_string()
            } else {
                metadata(
                    "file",
                    if collision { "remote-root" } else { "outside" },
                    1,
                    "bytes",
                )
            };
            (200, body, vec![("ETag".into(), "\"v1\"".into())])
        })
        .await;
        assert!(operator(&s, server.client.clone())
            .run(
                &executor.config,
                &request(&executor, OperationAction::RenameRemote),
                &executor.filesystem,
                &GoogleSession::new("test", "synthetic")
            )
            .await
            .is_err());
        assert!(server
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|r| r.starts_with("GET ")));
    }
}
#[test]
fn unknown_provider_and_wrong_account_are_rejected_before_execution() {
    let s = Sandbox::new();
    let executor = setup(&s);
    let accounts = Accounts::new(crate::accounts::AccountsConfig::new(
        s.0.join("accounts"),
        "test.operations",
        Arc::new(Associations),
    ))
    .unwrap();
    let factory = super::super::DefaultOperatorFactory::new(
        accounts,
        executor.inventory.clone(),
        GoogleDriveClient::new(),
    );
    use super::super::interfaces::OperatorFactory;
    let mut config = executor.config.clone();
    config.provider_id = ProviderId("unknown".into());
    assert!(matches!(
        factory.create(config),
        Err(OperatorError {
            code: OperatorErrorKind::UnsupportedProvider,
            ..
        })
    ));
    assert!(matches!(
        factory.create(executor.config.clone()),
        Err(OperatorError {
            code: OperatorErrorKind::PermissionDenied,
            ..
        })
    ));
}

#[tokio::test]
async fn directory_creation_checks_parent_and_records_provider_identity() {
    let s = Sandbox::new();
    let executor = setup(&s);
    std::fs::create_dir(executor.config.local_root.join("file")).unwrap();
    let server=endpoint(|r| {
        let body=if r.starts_with("POST ") {serde_json::json!({"id":"created"}).to_string()}
            else if r.contains("/files/created?") {serde_json::json!({"id":"created","name":"file","parents":["remote-root"],"mimeType":FOLDER,"version":"1","trashed":false,"appProperties":{"drivesyncExecution":"execution"}}).to_string()}
            else if r.contains("/files?") {"{\"files\":[],\"incompleteSearch\":false}".into()}else{root()};
        (200,body,vec![])
    }).await;
    let mut r = request(&executor, OperationAction::CreateRemoteDirectory);
    r.operation.name = Some("file".into());
    r.operation.parent_id = Some(RemoteId("remote-root".into()));
    r.operation.remote_id = None;
    r.identity.remote_id = None;
    r.remote.id = None;
    r.remote.state = FileState::Absent;
    let effect = operator(&s, server.client.clone())
        .run(
            &executor.config,
            &r,
            &executor.filesystem,
            &GoogleSession::new("test", "synthetic"),
        )
        .await
        .unwrap();
    assert_eq!(effect.remote_id, Some(RemoteId("created".into())));
    assert!(effect.verified);
    assert!(server
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.starts_with("POST ") && r.contains(FOLDER)));
}
#[tokio::test]
async fn source_absence_is_rechecked_before_trashing_remote_content() {
    let s = Sandbox::new();
    let executor = setup(&s);
    let r = request(&executor, OperationAction::TrashRemote);
    std::fs::write(executor.config.local_root.join("file"), b"new local edit").unwrap();
    let server = endpoint(|_| {
        (
            200,
            metadata("file", "remote-root", 1, "bytes"),
            vec![("ETag".into(), "\"v1\"".into())],
        )
    })
    .await;
    assert!(operator(&s, server.client.clone())
        .run(
            &executor.config,
            &r,
            &executor.filesystem,
            &GoogleSession::new("test", "synthetic")
        )
        .await
        .is_err());
    assert!(server
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|r| r.starts_with("GET ")));
    assert_eq!(
        std::fs::read(executor.config.local_root.join("file")).unwrap(),
        b"new local edit"
    );
}
#[tokio::test]
async fn workspace_shortcut_is_an_inert_non_executable_link() {
    let s = Sandbox::new();
    let executor = setup(&s);
    let server=endpoint(|_|(200,serde_json::json!({"id":"remote","name":"Document","parents":["remote-root"],"mimeType":"application/vnd.google-apps.document","version":"1","trashed":false}).to_string(),vec![])).await;
    let mut r = request(&executor, OperationAction::CreateLocalShortcut);
    r.operation.local_path = Some("document.desktop".into());
    let result = operator(&s, server.client.clone())
        .run(
            &executor.config,
            &r,
            &executor.filesystem,
            &GoogleSession::new("test", "synthetic"),
        )
        .await
        .unwrap();
    assert!(result.verified);
    let path = executor.config.local_root.join("document.desktop");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("Type=Link"));
    assert!(!text.contains("Exec="));
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o111,
        0
    );
}
#[tokio::test]
async fn resumable_content_update_uses_validator_and_bounded_chunks() {
    let s = Sandbox::new();
    let executor = setup(&s);
    std::fs::write(executor.config.local_root.join("file"), b"replacement").unwrap();
    let server_address = Arc::new(Mutex::new(String::new()));
    let location = server_address.clone();
    let uploaded = Arc::new(Mutex::new(false));
    let state = uploaded.clone();
    let server = endpoint(move |r| {
        if r.starts_with("PUT ") && r.contains("uploadType=resumable") {
            return (
                200,
                "".into(),
                vec![(
                    "Location".into(),
                    format!(
                        "{}/upload/drive/v2/files/remote?upload_id=session",
                        location.lock().unwrap()
                    ),
                )],
            );
        }
        if r.starts_with("PUT ") {
            *state.lock().unwrap() = true;
            return (
                200,
                metadata("file", "remote-root", 2, "replacement"),
                vec![],
            );
        }
        let complete = *state.lock().unwrap();
        (
            200,
            metadata(
                "file",
                "remote-root",
                if complete { 2 } else { 1 },
                if complete { "replacement" } else { "old" },
            ),
            vec![("ETag".into(), "\"v1\"".into())],
        )
    })
    .await;
    *server_address.lock().unwrap() = server.base.clone();
    let probe = operator(&s, server.client.clone());
    let result = probe
        .run(
            &executor.config,
            &request(&executor, OperationAction::UpdateRemoteContent),
            &executor.filesystem,
            &GoogleSession::new("test", "synthetic"),
        )
        .await
        .unwrap();
    assert!(result.verified);
    let calls = server.requests.lock().unwrap();
    let put = calls
        .iter()
        .find(|r| r.starts_with("PUT ") && r.contains("upload_id=session"))
        .unwrap();
    assert!(put.to_ascii_lowercase().contains("if-match: \"v1\""));
    assert!(put.contains("bytes 0-10/11"));
    assert!(put.ends_with("replacement"));
}

#[tokio::test]
async fn metadata_for_another_identity_is_rejected() {
    let sandbox = Sandbox::new();
    let server = endpoint(|_| (200, metadata("file", "remote-root", 1, "bytes"), vec![])).await;
    let error = operator(&sandbox, server.client.clone())
        .snapshot(
            &GoogleSession::new("test", "synthetic"),
            &RemoteId("different-item".into()),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, OperatorErrorKind::PreconditionFailed);
}

#[tokio::test]
async fn remote_move_and_trash_use_conditional_metadata_mutations() {
    for action in [OperationAction::MoveRemote, OperationAction::TrashRemote] {
        let sandbox = Sandbox::new();
        let executor = setup(&sandbox);
        let changed = Arc::new(Mutex::new(false));
        let state = changed.clone();
        let server=endpoint(move |r| {
            if r.starts_with("PATCH ") {*state.lock().unwrap()=true;return (200,"{}".into(),vec![]);}
            let complete=*state.lock().unwrap();
            let body=if r.contains("/files/old-parent?"){serde_json::json!({"id":"old-parent","parents":["remote-root"],"mimeType":FOLDER,"version":"1","trashed":false}).to_string()}else if r.contains("/files/remote-root?"){root()}else if r.contains("/files?"){"{\"files\":[],\"incompleteSearch\":false}".into()}
                else {let mut item:serde_json::Value=serde_json::from_str(&metadata("file",if action==OperationAction::MoveRemote && !complete {"old-parent"}else{"remote-root"},if complete {2}else{1},"bytes")).unwrap();if complete && action==OperationAction::TrashRemote {item["trashed"]=serde_json::Value::Bool(true);}item.to_string()};
            (200,body,vec![("ETag".into(),"\"v1\"".into())])
        }).await;
        let mut r = request(&executor, action);
        r.operation.parent_id = Some(RemoteId("remote-root".into()));
        let effect = operator(&sandbox, server.client.clone())
            .run(
                &executor.config,
                &r,
                &executor.filesystem,
                &GoogleSession::new("test", "synthetic"),
            )
            .await
            .unwrap();
        assert!(effect.verified);
        let calls = server.requests.lock().unwrap();
        let patch = calls.iter().find(|r| r.starts_with("PATCH ")).unwrap();
        assert!(patch.to_ascii_lowercase().contains("if-match: \"v1\""));
        if action == OperationAction::MoveRemote {
            assert!(patch.contains("addParents=remote-root"));
            assert!(patch.contains("removeParents=old-parent"));
        } else {
            assert!(patch.contains("\"labels\":{\"trashed\":true}"));
        }
    }
}

#[tokio::test]
async fn adoption_revalidates_versions_and_collisions_using_reads_only() {
    for scenario in ["stable", "changed", "duplicate"] {
        let sandbox = Sandbox::new();
        let executor = setup(&sandbox);
        std::fs::write(executor.config.local_root.join("file"), b"bytes").unwrap();
        let reads = Arc::new(Mutex::new(0));
        let state = reads.clone();
        let server = endpoint(move |request| {
            let body = if request.contains("/files/remote-root?") {
                let mut value: serde_json::Value = serde_json::from_str(&root()).unwrap();
                value["capabilities"]["canListChildren"] = true.into();
                value.to_string()
            } else if request.contains("/files?") {
                if scenario == "duplicate" {
                    r#"{"files":[{"id":"remote"},{"id":"collision"}],"incompleteSearch":false}"#
                        .into()
                } else {
                    r#"{"files":[{"id":"remote"}],"incompleteSearch":false}"#.into()
                }
            } else {
                let mut count = state.lock().unwrap();
                *count += 1;
                metadata(
                    "file",
                    "remote-root",
                    if scenario == "changed" && *count >= 3 {
                        2
                    } else {
                        1
                    },
                    "bytes",
                )
            };
            (200, body, Vec::new())
        })
        .await;
        let mut r = request(&executor, OperationAction::Adopt);
        r.local = executor
            .filesystem
            .observe(std::path::Path::new("file"))
            .unwrap();
        r.identity.local_identity = r.local.identity.clone();
        let result = operator(&sandbox, server.client.clone())
            .run(
                &executor.config,
                &r,
                &executor.filesystem,
                &GoogleSession::new("test", "synthetic"),
            )
            .await;
        if scenario == "stable" {
            assert!(result.unwrap().verified);
        } else {
            assert_eq!(
                result.unwrap_err().code,
                OperatorErrorKind::PreconditionFailed
            );
        }
        assert!(server
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|request| request.starts_with("GET ")));
        assert_eq!(
            std::fs::read(executor.config.local_root.join("file")).unwrap(),
            b"bytes"
        );
    }
}

struct MemoryJournal(std::sync::Mutex<RecoveryRecord>);
impl ExecutionJournal for MemoryJournal {
    fn record(&self) -> RecoveryRecord {
        self.0.lock().unwrap().clone()
    }
    fn checkpoint(&self, checkpoint: RecoveryCheckpoint) -> Result<(), OperatorError> {
        let mut record = self.0.lock().unwrap();
        match checkpoint {
            RecoveryCheckpoint::LocalStage(identity) => record.stage_identity = Some(identity),
            RecoveryCheckpoint::Transfer(transfer) => record.transfer = Some(transfer),
            RecoveryCheckpoint::RemoteAncestry(ancestors) => {
                record.remote_ancestry = Some(ancestors)
            }
        }
        Ok(())
    }
}
impl GoogleOperator {
    /// Component tests provide a controlled journal; production always persists its checkpoints.
    async fn run(
        &self,
        config: &OperatorConfig,
        r: &ExecuteOperationRequest,
        fs: &Filesystem,
        session: &GoogleSession,
    ) -> Result<AppliedEffect, OperatorError> {
        let journal = MemoryJournal(std::sync::Mutex::new(super::super::recovery::record(
            config,
            r,
            fs,
            Some(RemoteId("created".into())),
        )?));
        self.run_with_journal(config, r, fs, session, Some(&journal))
            .await
    }
}
