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
    assert_initial_download(
        vec![
            ("a".into(), "remote-root".into(), None),
            ("b".into(), "a".into(), None),
            ("c".into(), "b".into(), None),
            ("empty".into(), "a".into(), Some(String::new())),
            ("large".into(), "c".into(), Some("data".repeat(262144))),
        ],
        6,
    )
    .await;
}

#[tokio::test]
async fn hundred_independent_downloads_converge_and_reopen_without_replay() {
    let entries = (0..100)
        .map(|index| {
            (
                format!("file-{index:03}"),
                "remote-root".to_owned(),
                Some(format!("fixture {index}\n").repeat(4096)),
            )
        })
        .collect();
    assert_initial_download(entries, 101).await;
}

async fn assert_initial_download(fixture: Vec<(String, String, Option<String>)>, round_limit: u64) {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    let config = executor.config.clone();
    let mut entries = BTreeMap::new();
    let mut expected_files = BTreeMap::new();
    let fixture_size = fixture.len();
    let mut paths = BTreeMap::from([("remote-root".to_owned(), std::path::PathBuf::new())]);
    for (id, parent, content) in fixture {
        let path = paths[&parent].join(&id);
        paths.insert(id.clone(), path.clone());
        if let Some(content) = &content {
            expected_files.insert(path, content.clone());
        }
        let directory = content.is_none();
        let content = content.unwrap_or_default();
        let file: GoogleFile = serde_json::from_value(serde_json::json!({"id":id,"name":id,"parents":[parent],"mimeType":if directory {FOLDER} else {"application/octet-stream"},"version":"1","trashed":false,"size":content.len().to_string(),"sha256Checksum":format!("{:x}",Sha256::digest(content.as_bytes())),"capabilities":{"canDownload":true}})).unwrap();
        entries.insert(id, (file, content));
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
        trash_propagation_authorized: false,
        initial_association_authorized: false,
    };
    let worker_config = OrchestratorConfig {
        expected_root_identity: None,
        trash_propagation_authorized: false,
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
        _writer: None,
        capture_epoch: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        local_refresh: Arc::new(tokio::sync::Notify::new()),
        remote_refresh: Arc::new(tokio::sync::Notify::new()),
        config: worker_config,
        operator: Box::new(executor),
        status,
        stop: rx,
        blockers: BTreeMap::new(),
        sources: BTreeMap::new(),
        recovery_failed: false,
        refresh_after: None,
        refresh_wait: None,
    };
    let scanner = Scanner::open(config.local_root.clone(), config.sync_root_id.clone()).unwrap();
    let epoch = worker.capture_epoch.clone();
    let capture = |generation, entries| {
        Observation::Capture(Box::new(ScopeCapture {
            collect_epoch: epoch.load(std::sync::atomic::Ordering::SeqCst),
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
        worker.refresh_after = None;
        worker.refresh_wait = None;
        context
            .consolidate(capture(1, CaptureEntries::Remote(remote.clone())))
            .unwrap();
        // Fresh reads of the unchanged remote tree require no external mutation between levels.
        let _sample = crate::performance::Sample::begin(if pass == 0 {
            "fixture_initial_convergence"
        } else {
            "fixture_reopen_audit"
        });
        for generation in 1..=round_limit {
            let refreshed = crate::performance::Sample::begin("fixture_capture_pair");
            context
                .consolidate(capture(
                    generation + 1,
                    CaptureEntries::Remote(remote.clone()),
                ))
                .unwrap();
            let local = scanner.scan(&CancellationToken::new()).unwrap();
            let files = context
                .consolidate(capture(
                    generation,
                    CaptureEntries::Local(local.entries.into_values().collect()),
                ))
                .unwrap();
            drop(refreshed);
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
            let rows = inventory.enumerate(query.clone()).unwrap();
            if rows.len() == fixture_size
                && rows
                    .iter()
                    .all(|row| row.baseline.state == BaselineState::Synced)
            {
                break;
            }
        }
        let rows = inventory.enumerate(query.clone()).unwrap();
        assert_eq!(rows.len(), fixture_size, "pass {pass}");
        assert!(rows
            .iter()
            .all(|r| r.baseline.state == BaselineState::Synced));
        assert!(rows.iter().all(|r| r.pending_operations.len() == 1
            && r.pending_operations[0].state == OperationState::Confirmed));
        for (path, content) in &expected_files {
            assert_eq!(
                std::fs::read(config.local_root.join(path)).unwrap(),
                content.as_bytes()
            );
        }
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
        expected_files.len()
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

#[tokio::test]
async fn office_edit_without_provider_sha256_replaces_local_bytes_verified_by_observation() {
    use super::super::tests::request;

    let s = Sandbox::new();
    let mut executor = setup(&s);
    let local = executor.config.local_root.join("file");
    std::fs::write(&local, b"old").unwrap();
    let mut office: serde_json::Value =
        serde_json::from_str(&metadata("file", "remote-root", 1, "hello")).unwrap();
    office["mimeType"] =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document".into();
    office["md5Checksum"] = "5d41402abc4b2a76b9719d911017c592".into();
    office["headRevisionId"] = "updated-at-drive".into();
    let observed = normalize(&serde_json::from_value(office.clone()).unwrap());
    office.as_object_mut().unwrap().remove("sha256Checksum");
    let server = endpoint(move |req| {
        (
            200,
            if req.contains("alt=media") {
                "hello".into()
            } else {
                office.to_string()
            },
            vec![],
        )
    })
    .await;
    use_backend(&s, &mut executor, server.client.clone(), false);
    let mut request = request(&executor, OperationAction::UpdateLocalContent);
    request.operation.name = Some("file".into());
    request.operation.parent = Some(Path::new());
    request.remote = observed.clone();
    let result = executor.execute(request).await;
    assert_eq!(
        result.status,
        OperationResultStatus::Succeeded,
        "{result:?}"
    );
    assert_eq!(std::fs::read(local).unwrap(), b"hello");
    assert_eq!(
        result.effect.unwrap().remote.unwrap().checksum,
        observed.checksum
    );
    assert_eq!(
        server
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.contains("alt=media"))
            .count(),
        1
    );
}
