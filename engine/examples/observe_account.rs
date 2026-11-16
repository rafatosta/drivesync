//! Explicitly opted-in live observation harness; never creates user files or executes operations.
//! Supply account/root configuration through environment variables. Uses the same public
//! Orchestrator and account identity as the application. Commands: status, prepare, associate LOCAL_ID REMOTE_ID, quit.
use drivesync_engine::{
    detectors::*, inventory::*, models::*, orchestrator::*, Engine, EngineConfig,
};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
    time::Duration,
};
fn required(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("missing {name}"))
}
fn status(session: &Orchestrator) {
    let s = session.preparation_status();
    println!(
        "STATE {:?} processed={} error={:?}",
        s.state, s.events_processed, s.error
    );
    println!("LOCAL {:?}", session.local_status().map(|s| s.state));
    if let Some(r) = session.remote_diagnostics() {
        println!("REMOTE {:?}", r.status);
    }
    println!("QUEUE {:?}", session.diagnostics().delivery);
    io::stdout().flush().unwrap();
}
#[tokio::main]
async fn main() {
    assert_eq!(
        required("DRIVESYNC_LIVE_OBSERVATION"),
        "1",
        "explicit live observation opt-in required"
    );
    let account_id = AccountId(required("DRIVESYNC_LIVE_ACCOUNT"));
    let connector = Engine::open(EngineConfig {
        data_directory: PathBuf::from(required("DRIVESYNC_LIVE_ACCOUNTS")),
        application_id: required("DRIVESYNC_LIVE_APPLICATION"),
        google_oauth_file: std::env::var_os("DRIVESYNC_GOOGLE_OAUTH_FILE").map(PathBuf::from),
    })
    .await
    .expect("open configured account store");
    let session = Orchestrator::new(OrchestratorCreateRequest {
        detector: DetectorCreateRequest {
            connector,
            scope: DetectorScope {
                sync_id: SyncId(format!("developer-unified:{}", account_id.0)),
                account_id,
                local_root: PathBuf::from(required("DRIVESYNC_LIVE_ROOT")),
                remote_root_id: required("DRIVESYNC_LIVE_REMOTE_ROOT"),
            },
            local_settings: LocalDetectorSettings::default(),
            remote_settings: RemoteDetectorSettings {
                polling_interval: Duration::from_secs(5),
                ..Default::default()
            },
            event_available_sink: None,
        },
    })
    .expect("open observation queue");
    println!(
        "PREPARE {:?}",
        session.prepare().await.map(|r| r.ready_for_analysis())
    );
    status(&session);
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let words: Vec<_> = line.split_whitespace().collect();
        if let ["associate", local, remote] = words.as_slice() {
            let scope = session.detector().scope();
            let inventory = InventoryManager::open(OpenInventoryRequest {
                local_root: scope.local_root.clone(),
                expected: SyncIdentity {
                    sync_id: scope.sync_id.clone(),
                    account_id: scope.account_id.clone(),
                    remote_root_id: scope.remote_root_id.clone(),
                },
                create_if_missing: false,
            })
            .expect("open existing Inventory")
            .inventory;
            let get = |id: &str| {
                inventory
                    .get_item(GetItemRequest {
                        selector: ItemSelector::Id(SyncItemId(id.into())),
                    })
                    .expect("read explicit selection")
                    .item
                    .expect("selected item exists")
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            println!(
                "ASSOCIATION {:?}",
                inventory
                    .associate_items(AssociateItemsRequest {
                        expected_local: get(local),
                        expected_remote: get(remote),
                        decision_id: format!("live-test-explicit-{now}"),
                        decided_at: Timestamp { unix_millis: now },
                    })
                    .map(|result| result.association)
            );
            continue;
        }
        match line.trim() {
            "status" => status(&session),
            "prepare" => {
                println!(
                    "PREPARE {:?}",
                    session.prepare().await.map(|r| r.ready_for_analysis())
                );
                status(&session);
            }
            "quit" => break,
            _ => println!("Commands: status, prepare, associate LOCAL_ID REMOTE_ID, quit"),
        }
    }
    session
        .stop_detector(DetectorStopMode::StopNow)
        .expect("stop observation");
}
