//! The real Context/Analyzer/Operations pipeline converges an unchanged remote tree.
use super::super::{
    tests::{setup, Sandbox},
    *,
};
use super::*;
use super::{
    recovery_tests::use_backend,
    tests::{endpoint, metadata},
};
use crate::{
    detectors::{
        context::{Context, ContextConfig, PathRoot},
        observation::{
            models::{CaptureEntries, ScopeCapture},
            remote::interfaces::CancellationToken,
            scanner::Scanner,
            *,
        },
    },
    inventory::models::*,
    orchestrator::{Orchestrator, OrchestratorConfig},
    synchronization::*,
};
use std::collections::BTreeMap;

#[tokio::test]
async fn three_level_initial_download_rechecks_children_and_reopens_without_replay() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    let config = executor.config.clone();
    let mut entries = BTreeMap::new();
    for (id, name, parent, content) in [
        ("a", "a", "remote-root", None),
        ("b", "b", "a", None),
        ("c", "c", "b", None),
        ("empty", "empty", "a", Some(String::new())),
        ("large", "large", "c", Some("data".repeat(262144))),
    ] {
        let directory = content.is_none();
        let content = content.unwrap_or_default();
        let file: GoogleFile = serde_json::from_value(serde_json::json!({"id":id,"name":name,"parents":[parent],"mimeType":if directory {FOLDER} else {"application/octet-stream"},"version":"1","trashed":false,"size":content.len().to_string(),"sha256Checksum":format!("{:x}",Sha256::digest(content.as_bytes())),"capabilities":{"canDownload":true}})).unwrap();
        entries.insert(id.to_owned(), (file, content));
    }
    let remote: Vec<_> = entries
        .values()
        .map(|(file, _)| {
            let state = normalize(file);
            RemoteObservation {
                account_id: config.account_id.clone(),
                provider_id: config.provider_id.clone(),
                sync_root_id: config.sync_root_id.clone(),
                state: state.state,
                remote_id: state.id,
                name: state.name,
                parents: state.parents,
                mime_type: state.mime_type,
                size: state.size,
                checksum: state.checksum,
                version: state.version,
                modified_time: state.modified_time,
                trashed: state.trashed,
                capabilities: state.capabilities,
                observed_at: now(),
                coverage: ObservationCoverage::Complete,
                page_token: None,
                event: None,
            }
        })
        .collect();
    let server = endpoint(move |r| {
        let id = r
            .split("/files/")
            .nth(1)
            .unwrap()
            .split('?')
            .next()
            .unwrap();
        let (file, content) = &entries[id];
        (
            200,
            if r.contains("alt=media") {
                content.clone()
            } else {
                serde_json::to_string(file).unwrap()
            },
            vec![],
        )
    })
    .await;
    use_backend(&s, &mut executor, server.client.clone(), false);
    let inventory = executor.inventory.clone();
    let context_config = || ContextConfig {
        inventory_directory: s.0.join("inventory"),
        account_id: config.account_id.clone(),
        provider_id: config.provider_id.clone(),
        sync_root_id: config.sync_root_id.clone(),
        local_root: PathRoot(config.local_root.clone()),
        remote_root_id: config.remote_root_id.clone(),
        local_root_available: true,
        remote_available: true,
        initial_association_authorized: false,
    };
    let worker_config = OrchestratorConfig {
        initial_association_authorized: false,
        roots: StartSynchronizationRequest {
            account_id: config.account_id.clone(),
            local_root: config.local_root.clone(),
            remote_root_id: config.remote_root_id.clone(),
        },
        sync_root_id: config.sync_root_id.clone(),
        provider_id: config.provider_id.clone(),
    };
    let (status, _) = tokio::sync::watch::channel(SynchronizationStatus::new(
        config.account_id.clone(),
        SynchronizationState::Waiting,
    ));
    let (_stop, rx) = tokio::sync::watch::channel(false);
    let mut worker = Orchestrator {
        local_refresh: Arc::new(tokio::sync::Notify::new()),
        config: worker_config,
        operator: Box::new(executor),
        status,
        stop: rx,
        blockers: BTreeMap::new(),
        sources: BTreeMap::new(),
        recovery_failed: false,
    };
    let scanner = Scanner::open(config.local_root.clone(), config.sync_root_id.clone()).unwrap();
    let capture = |generation, entries| {
        Observation::Capture(Box::new(ScopeCapture {
            account_id: config.account_id.clone(),
            provider_id: config.provider_id.clone(),
            sync_root_id: config.sync_root_id.clone(),
            local_root: config.local_root.clone(),
            remote_root_id: config.remote_root_id.clone(),
            generation,
            entries,
            coverage: ObservationCoverage::Complete,
            observed_at: now(),
        }))
    };
    let query = InventoryScopeQuery {
        account_id: config.account_id.clone(),
        sync_root_id: config.sync_root_id.clone(),
    };
    for pass in 0..2 {
        let context = Context::open(context_config()).unwrap();
        context
            .consolidate(capture(1, CaptureEntries::Remote(remote.clone())))
            .unwrap();
        // No new remote capture or external filesystem mutation occurs between levels.
        for generation in 1..=6 {
            let local = scanner.scan(&CancellationToken::new()).unwrap();
            let files = context
                .consolidate(capture(
                    generation,
                    CaptureEntries::Local(local.entries.into_values().collect()),
                ))
                .unwrap();
            let mut published_parent = false;
            for file in files {
                let result = worker.process(file).await;
                published_parent |= result.results.iter().any(|r| {
                    r.action == OperationAction::CreateLocalDirectory
                        && r.status == OperationResultStatus::Succeeded
                });
            }
            if published_parent {
                tokio::time::timeout(
                    std::time::Duration::from_millis(100),
                    worker.local_refresh.notified(),
                )
                .await
                .unwrap();
            }
        }
        let rows = inventory.enumerate(query.clone()).unwrap();
        assert_eq!(rows.len(), 5, "pass {pass}");
        assert!(rows
            .iter()
            .all(|r| r.baseline.state == BaselineState::Synced));
        assert!(rows.iter().all(|r| r.pending_operations.len() == 1
            && r.pending_operations[0].state == OperationState::Confirmed));
        assert_eq!(
            std::fs::read(config.local_root.join("a/empty")).unwrap(),
            b""
        );
        assert_eq!(
            std::fs::read(config.local_root.join("a/b/c/large")).unwrap(),
            "data".repeat(262144).as_bytes()
        );
        if pass == 0 {
            std::fs::write(s.0.join("before.json"), serde_json::to_vec(&rows).unwrap()).unwrap();
        } else {
            let before: Vec<InventorySnapshot> =
                serde_json::from_slice(&std::fs::read(s.0.join("before.json")).unwrap()).unwrap();
            assert_eq!(rows, before);
        }
    }
    assert_eq!(
        server
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.contains("alt=media"))
            .count(),
        2
    );
}

#[tokio::test]
async fn download_failure_never_publishes_or_confirms_and_preserves_concurrent_content() {
    for case in ["collision", "version", "hash", "size"] {
        let s = Sandbox::new();
        let mut executor = setup(&s);
        let destination = executor.config.local_root.join("file");
        let path = destination.clone();
        let downloaded = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = downloaded.clone();
        let server = endpoint(move |r| {
            if r.contains("alt=media") {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
                if case == "collision" {
                    std::fs::write(&path, b"concurrent local content").unwrap();
                }
                return (
                    200,
                    match case {
                        "hash" => "wrong",
                        "size" => "shorter payload",
                        _ => "bytes",
                    }
                    .into(),
                    vec![],
                );
            }
            let changed = case == "version" && flag.load(std::sync::atomic::Ordering::SeqCst);
            (
                200,
                metadata("file", "remote-root", if changed { 2 } else { 1 }, "bytes"),
                vec![],
            )
        })
        .await;
        use_backend(&s, &mut executor, server.client.clone(), false);
        let mut request = super::super::tests::request(&executor, OperationAction::Download);
        request.remote =
            normalize(&serde_json::from_str(&metadata("file", "remote-root", 1, "bytes")).unwrap());
        let result = executor.execute(request).await;
        assert_ne!(result.status, OperationResultStatus::Succeeded, "{case}");
        let rows = executor
            .inventory
            .enumerate(InventoryScopeQuery {
                account_id: executor.config.account_id.clone(),
                sync_root_id: executor.config.sync_root_id.clone(),
            })
            .unwrap();
        assert_eq!(rows[0].baseline.state, BaselineState::NeverSynced, "{case}");
        assert!(rows[0]
            .pending_operations
            .iter()
            .all(|op| op.state != OperationState::Confirmed));
        if case == "collision" {
            assert_eq!(
                std::fs::read(destination).unwrap(),
                b"concurrent local content"
            );
        } else {
            assert!(!destination.exists(), "{case}");
        }
    }
}

#[tokio::test]
async fn full_storage_error_stops_verified_media_before_any_publication() {
    let server = endpoint(|_| (200, "downloaded bytes".into(), vec![])).await;
    let mut stream = server
        .client
        .get_file_bytes(
            &GoogleSession::new("test", "synthetic"),
            &FileId("remote".into()),
            None,
        )
        .await
        .unwrap();
    // /dev/full returns the real OS ENOSPC error without filling a shared filesystem.
    let mut full = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .unwrap();
    let result = receive_verified(
        &mut stream.body,
        &mut full,
        16,
        &format!("{:x}", Sha256::digest(b"downloaded bytes")),
    )
    .await;
    assert_eq!(result.unwrap_err().code, OperatorErrorKind::Unavailable);
}

#[tokio::test]
async fn an_existing_directory_without_confirmed_parent_association_cannot_receive_children() {
    let s = Sandbox::new();
    let executor = setup(&s);
    std::fs::create_dir(executor.config.local_root.join("unassociated")).unwrap();
    let mut r = super::super::tests::request(&executor, OperationAction::CreateLocalDirectory);
    r.local.parent = Evidence::Known("unassociated".into());
    r.local.path = Some("unassociated/file".into());
    r.operation.local_path = r.local.path.clone();
    r.remote.parents = Evidence::Known(vec![RemoteId("unassociated-remote".into())]);
    let result = executor.execute(r).await;
    assert_eq!(result.status, OperationResultStatus::Skipped);
    assert!(result.effect.is_none());
    assert!(!executor
        .config
        .local_root
        .join("unassociated/file")
        .exists());
    assert!(executor
        .inventory
        .enumerate(InventoryScopeQuery {
            account_id: executor.config.account_id.clone(),
            sync_root_id: executor.config.sync_root_id.clone()
        })
        .unwrap()
        .is_empty());
}
