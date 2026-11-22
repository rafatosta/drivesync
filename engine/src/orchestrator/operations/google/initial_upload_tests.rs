//! Initial upload recovery exercises durable checkpoints and actual loopback request sequences.
use super::super::{
    tests::{request, setup, Sandbox},
    *,
};
use super::*;
use super::{
    recovery_tests::reopen,
    tests::{endpoint, metadata, operator, root},
};
use crate::inventory::models::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

struct Backend {
    google: GoogleOperator,
    interrupt: bool,
}
struct Interrupt<'a>(&'a dyn ExecutionJournal);
impl ExecutionJournal for Interrupt<'_> {
    fn record(&self) -> RecoveryRecord {
        self.0.record()
    }
    fn checkpoint(&self, checkpoint: RecoveryCheckpoint) -> Result<(), OperatorError> {
        self.0.checkpoint(checkpoint.clone())?;
        if matches!(checkpoint, RecoveryCheckpoint::Transfer(t) if t.confirmed_offset == 262144) {
            return Err(filesystem::stale().uncertain());
        }
        Ok(())
    }
}
impl RemoteExecutor for Backend {
    fn reserve<'a>(
        &'a self,
        _: &'a OperatorConfig,
        _: &'a ExecuteOperationRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<RemoteId>, OperatorError>> + Send + 'a>,
    > {
        Box::pin(async { Ok(Some(RemoteId("created".into()))) })
    }
    fn execute<'a>(
        &'a self,
        c: &'a OperatorConfig,
        r: &'a ExecuteOperationRequest,
        fs: &'a Filesystem,
        journal: Option<&'a dyn ExecutionJournal>,
    ) -> EffectFuture<'a> {
        Box::pin(async move {
            let wrapped = Interrupt(journal.unwrap());
            self.google
                .run_with_journal(
                    c,
                    r,
                    fs,
                    &GoogleSession::new("test", "synthetic"),
                    if self.interrupt {
                        Some(&wrapped)
                    } else {
                        journal
                    },
                )
                .await
        })
    }
    fn reconcile<'a>(
        &'a self,
        c: &'a OperatorConfig,
        r: &'a RecoveryRecord,
        fs: &'a Filesystem,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = RecoveryAssessment> + Send + 'a>> {
        Box::pin(async move {
            self.google
                .probe_recovery(c, r, fs, &GoogleSession::new("test", "synthetic"))
                .await
        })
    }
    fn resume_upload<'a>(
        &'a self,
        c: &'a OperatorConfig,
        r: &'a RecoveryRecord,
        fs: &'a Filesystem,
        journal: &'a dyn ExecutionJournal,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = RecoveryAssessment> + Send + 'a>> {
        Box::pin(async move {
            self.google
                .continue_upload(c, r, fs, journal, &GoogleSession::new("test", "synthetic"))
                .await
        })
    }
}
fn upload_request(executor: &PersistingOperator) -> ExecuteOperationRequest {
    let mut r = request(executor, OperationAction::Upload);
    r.operation.remote_id = None;
    r.identity.remote_id = None;
    r.remote.id = None;
    r.remote.state = FileState::Absent;
    r.operation.name = Some("file".into());
    r.operation.parent_id = Some(RemoteId("remote-root".into()));
    r
}

#[tokio::test]
async fn upload_reopens_after_first_block_and_never_restarts_a_create() {
    for case in [
        "resume",
        "settling",
        "expired",
        "source-changed",
        "change-during",
        "regressed",
        "lost-final",
        "collision",
    ] {
        let s = Sandbox::new();
        let mut executor = setup(&s);
        let content = "x".repeat(300000);
        std::fs::write(executor.config.local_root.join("file"), &content).unwrap();
        let address = Arc::new(Mutex::new(String::new()));
        let location = address.clone();
        let complete = Arc::new(AtomicBool::new(false));
        let state = complete.clone();
        let interrupted = Arc::new(AtomicBool::new(false));
        let resumed = interrupted.clone();
        let expected = content.clone();
        let source_path = executor.config.local_root.join("file");
        let readbacks = std::sync::atomic::AtomicUsize::new(0);
        let server=endpoint(move |r| {
            let lower=r.to_ascii_lowercase();
            if r.starts_with("POST ") {
                return (200,String::new(),vec![("Location".into(),format!("{}/upload/drive/v3/files?upload_id=session",location.lock().unwrap()))]);
            }
            if r.starts_with("PUT ") {
                if lower.contains("content-range: bytes */") {
                    if case=="expired" {return (404,"{}".into(),vec![]);}
                    return (308,String::new(),vec![("Range".into(),if case=="regressed" {"bytes=0-131071"} else {"bytes=0-262143"}.into())]);
                }
                if lower.contains("content-range: bytes 0-262143/300000") {
                    assert!(r.ends_with(&expected[..262144]));
                    if case == "change-during" { std::fs::write(&source_path, b"mutated while sending").unwrap(); } resumed.store(true,Ordering::SeqCst);
                    return (308,String::new(),vec![("Range".into(),"bytes=0-262143".into())]);
                }
                assert!(lower.contains("content-range: bytes 262144-299999/300000"));
                assert!(r.ends_with(&expected[262144..])); state.store(true,Ordering::SeqCst);
                if case=="lost-final" { return (500,"{}".into(),vec![]); }
            }
            if r.contains("/files/remote-root?") { return (200,root(),vec![]); }
            if r.starts_with("GET ") && r.contains("/files?") {
                let collision=case=="collision" && resumed.load(Ordering::SeqCst);
                return (200,serde_json::json!({"files":if collision {vec![serde_json::json!({"id":"foreign"})]} else {vec![]},"incompleteSearch":false}).to_string(),vec![]);
            }
            if !state.load(Ordering::SeqCst) { return (404,"{}".into(),vec![]); }
            let mut meta:serde_json::Value=serde_json::from_str(&metadata("file","remote-root",1,&expected)).unwrap();
            if case == "settling" && r.starts_with("GET ") && readbacks.fetch_add(1, Ordering::SeqCst) == 0 { meta["sha256Checksum"] = serde_json::Value::Null; }
            meta["id"]="created".into();meta["appProperties"]=serde_json::json!({"drivesyncExecution":"execution"});
            (200,meta.to_string(),vec![])
        }).await;
        *address.lock().unwrap() = server.base.clone();
        executor.remote = Arc::new(Backend {
            google: operator(&s, server.client.clone()),
            interrupt: case != "lost-final",
        });
        let r = upload_request(&executor);
        let result = executor.execute(r).await;
        assert_eq!(result.status, OperationResultStatus::Unconfirmed, "{case}");
        assert!(interrupted.load(Ordering::SeqCst));
        if case == "source-changed" {
            std::fs::write(
                executor.config.local_root.join("file"),
                b"changed and preserved",
            )
            .unwrap();
        }
        let reopened = reopen(&s, executor);
        let recovered = reopened.recover().await.unwrap();
        let rows = reopened
            .inventory
            .enumerate(InventoryScopeQuery {
                account_id: reopened.config.account_id.clone(),
                sync_root_id: reopened.config.sync_root_id.clone(),
            })
            .unwrap();
        if matches!(case, "resume" | "lost-final" | "settling") {
            assert_eq!(
                recovered[0].assessment.outcome,
                ExecutionEffect::Applied,
                "{case}: {:?}; calls {:?}",
                recovered[0].assessment,
                server
                    .requests
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|r| r.lines().take(1).collect::<Vec<_>>())
                    .collect::<Vec<_>>()
            );
            assert_eq!(rows[0].baseline.state, BaselineState::Synced);
            assert_eq!(
                rows[0].pending_operations[0].state,
                OperationState::Confirmed
            );
            assert!(reopened.recover().await.unwrap().is_empty());
        } else {
            assert_eq!(
                recovered[0].assessment.outcome,
                ExecutionEffect::Unknown,
                "{case}"
            );
            assert_eq!(rows[0].baseline.state, BaselineState::NeverSynced);
            assert!(rows[0].pending_operations[0]
                .recovery
                .as_ref()
                .unwrap()
                .transfer
                .is_some());
        }
        let calls = server.requests.lock().unwrap();
        assert_eq!(
            calls.iter().filter(|r| r.starts_with("POST ")).count(),
            1,
            "{case}"
        );
        assert_eq!(
            calls
                .iter()
                .filter(|r| r
                    .to_ascii_lowercase()
                    .contains("content-range: bytes 0-262143/300000"))
                .count(),
            1,
            "{case}"
        );
        assert_eq!(
            calls
                .iter()
                .filter(|r| r
                    .to_ascii_lowercase()
                    .contains("content-range: bytes 262144-299999/300000"))
                .count(),
            usize::from(matches!(case, "resume" | "lost-final" | "settling")),
            "{case}"
        );
    }
}

#[tokio::test]
async fn another_inventory_instance_cannot_dispatch_or_recover_an_active_account() {
    let s = Sandbox::new();
    let executor = setup(&s);
    let lease = executor.inventory.execution_lease().unwrap();
    let other = reopen(&s, executor);
    assert!(other.recover().await.is_err());
    let r = request(&other, OperationAction::CreateLocalDirectory);
    let result = other.execute(r.clone()).await;
    assert_eq!(result.status, OperationResultStatus::Skipped);
    assert!(!other.config.local_root.join("file").exists());
    drop(lease);
    assert_eq!(
        other.execute(r).await.status,
        OperationResultStatus::Succeeded
    );
}

#[tokio::test]
async fn a_remote_collision_prevents_upload_session_creation_and_preserves_the_source() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    std::fs::write(executor.config.local_root.join("file"), b"original source").unwrap();
    let server = endpoint(|r| {
        if r.contains("/files/remote-root?") {
            (200, root(), vec![])
        } else {
            (
                200,
                r#"{"files":[{"id":"foreign"}],"incompleteSearch":false}"#.into(),
                vec![],
            )
        }
    })
    .await;
    executor.remote = Arc::new(Backend {
        google: operator(&s, server.client.clone()),
        interrupt: false,
    });
    let r = upload_request(&executor);
    let result = executor.execute(r).await;
    assert_ne!(result.status, OperationResultStatus::Succeeded);
    assert_eq!(
        std::fs::read(executor.config.local_root.join("file")).unwrap(),
        b"original source"
    );
    assert!(server
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|r| r.starts_with("GET ")));
}
