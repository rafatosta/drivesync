//! Loopback failure after a real archived conflict replacement; recovery must not replay it.
use super::recovery_tests::{reopen, use_backend};
use super::tests::{endpoint, metadata, root};
use super::*;
use crate::{
    conflicts::ConflictChoice,
    inventory::models::*,
    orchestrator::operations::{
        interfaces::Operator,
        tests::{request, setup, Sandbox},
    },
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
#[tokio::test]
async fn lost_conflict_replacement_receipt_preserves_both_versions_and_recovers_without_replay() {
    let s = Sandbox::new();
    let mut executor = setup(&s);
    std::fs::write(executor.config.local_root.join("file"), b"local").unwrap();
    let uploaded = Arc::new(AtomicBool::new(false));
    let done = uploaded.clone();
    let address = Arc::new(Mutex::new(String::new()));
    let location = address.clone();
    let server = endpoint(move |r| {
        if r.contains("alt=media") {
            return (200, "remote".into(), vec![]);
        }
        if r.starts_with("PUT ") && r.contains("uploadType=resumable") {
            return (
                200,
                String::new(),
                vec![(
                    "Location".into(),
                    format!(
                        "{}/upload/drive/v2/files/remote?upload_id=conflict",
                        location.lock().unwrap()
                    ),
                )],
            );
        }
        if r.starts_with("PUT ") {
            done.store(true, Ordering::SeqCst);
        }
        let body = if r.contains("/files/remote-root?") {
            root()
        } else {
            let mut v: serde_json::Value = serde_json::from_str(&metadata(
                "file",
                "remote-root",
                if done.load(Ordering::SeqCst) { 2 } else { 1 },
                if done.load(Ordering::SeqCst) {
                    "local"
                } else {
                    "remote"
                },
            ))
            .unwrap();
            if done.load(Ordering::SeqCst) {
                v["appProperties"] = serde_json::json!({"drivesyncExecution":"execution"});
            }
            v.to_string()
        };
        (200, body, vec![("ETag".into(), "\"v1\"".into())])
    })
    .await;
    *address.lock().unwrap() = server.base.clone();
    use_backend(&s, &mut executor, server.client.clone(), true);
    let mut r = request(&executor, OperationAction::UpdateRemoteContent);
    r.operation.name = Some("file".into());
    r.operation.conflict_id = Some(Id("conflict-test".into()));
    r.remote =
        normalize(&serde_json::from_str(&metadata("file", "remote-root", 1, "remote")).unwrap());
    let conflict = ConflictState {
        id: Id("conflict-test".into()),
        item_id: r.operation.item_ref.clone(),
        local: r.local.clone(),
        remote: r.remote.clone(),
        observed_revision: 0,
        reason: "Content".into(),
        resolved: false,
        resolution: Some(Box::new(ConflictResolution {
            choice: ConflictChoice::KeepLocal,
            expected_local: r.local.clone(),
            expected_remote: r.remote.clone(),
        })),
        updated_at: super::super::now(),
    };
    let write = executor.inventory.write(InventoryWrite {
        operation_id: Id("conflict-test".into()),
        item_id: r.operation.item_ref.clone(),
        identity: r.identity.clone(),
        expected_revision: Some(0),
        effect: PersistedEffect::ConflictUpdated(Some(conflict)),
        recorded_at: super::super::now(),
    });
    assert_eq!(write.status, WriteStatus::Written);
    r.expected_revision = write.revision;
    let inbox = activity::ActivityInbox::for_config(&executor.config);
    assert_eq!(
        inbox.observe(executor.execute(r)).await.status,
        OperationResultStatus::Unconfirmed
    );
    let records = inbox.take_updates();
    for size in [5, 6] {
        assert!(records.iter().any(|record| record.phase == Phase::ArchiveConflict
            && record.state == State::Finished(Outcome::Complete)
            && matches!(record.progress, Some(Progress::Bytes(bytes)) if bytes.written == Some(size))));
    }
    assert!(records
        .iter()
        .any(|record| record.phase == Phase::VerifyArchive
            && record.state == State::Finished(Outcome::Complete)));
    assert!(records.iter().any(|record| record.phase == Phase::Upload
        && record.state == State::Finished(Outcome::Complete)));
    assert_eq!(
        records.last().unwrap().state,
        State::Finished(Outcome::Unconfirmed)
    );
    assert!(uploaded.load(Ordering::SeqCst));
    let archive = executor
        .config
        .local_root
        .join(".drivesync/conflicts/conflict-test");
    assert_eq!(std::fs::read(archive.join("local.bin")).unwrap(), b"local");
    assert_eq!(
        std::fs::read(archive.join("remote.bin")).unwrap(),
        b"remote"
    );
    let reopened = reopen(&s, executor);
    let result = reopened.recover().await.unwrap();
    assert_eq!(result[0].assessment.outcome, ExecutionEffect::Applied);
    let row = reopened
        .inventory
        .enumerate(InventoryScopeQuery {
            account_id: reopened.config.account_id.clone(),
            sync_root_id: reopened.config.sync_root_id.clone(),
        })
        .unwrap()
        .remove(0);
    assert!(row.prior_conflict.unwrap().resolved);
    assert_eq!(row.baseline.state, BaselineState::Synced);
    assert_eq!(
        server
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with("PUT ") && !r.contains("uploadType=resumable"))
            .count(),
        1
    );
}

#[tokio::test]
async fn missing_archive_checkpoint_proves_no_dispatch_but_missing_committed_archive_blocks_recovery(
) {
    let s = Sandbox::new();
    let executor = setup(&s);
    std::fs::write(executor.config.local_root.join("file"), b"local").unwrap();
    let server = endpoint(|_| panic!("archive barrier must be checked before provider I/O")).await;
    let google = super::tests::operator(&s, server.client.clone());
    let mut r = request(&executor, OperationAction::UpdateRemoteContent);
    r.operation.conflict_id = Some(Id("conflict-test".into()));
    let mut record =
        super::super::recovery::record(&executor.config, &r, &executor.filesystem, None).unwrap();
    let session = GoogleSession::new("test", "synthetic");
    assert_eq!(
        google
            .probe_recovery(&executor.config, &record, &executor.filesystem, &session)
            .await
            .outcome,
        ExecutionEffect::NotApplied
    );
    record.conflict_archive = r.operation.conflict_id;
    assert_eq!(
        google
            .probe_recovery(&executor.config, &record, &executor.filesystem, &session)
            .await
            .outcome,
        ExecutionEffect::Unknown
    );
    assert_eq!(
        std::fs::read(executor.config.local_root.join("file")).unwrap(),
        b"local"
    );
}
