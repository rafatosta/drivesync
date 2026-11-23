//! Opt-in real-account Stage 2 validation. Supply an isolated scope; normal builds never run it.
use drivesync_engine::{
    models::RemoteId, Engine, EngineConfig, ListAccountsRequest, StartSynchronizationRequest,
};
use sha2::{Digest, Sha256};
use std::{os::unix::ffi::OsStrExt, path::PathBuf};
fn scope(request: &StartSynchronizationRequest) -> String {
    let mut hash = Sha256::new();
    for bytes in [
        request.account_id.0.as_bytes(),
        request.remote_root_id.0.as_bytes(),
        request.local_root.as_os_str().as_bytes(),
    ] {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    format!("{:x}", hash.finalize())
}
fn snapshots(
    directory: &std::path::Path,
    request: &StartSynchronizationRequest,
) -> Vec<serde_json::Value> {
    let filename = format!(
        "{:x}.sqlite3",
        Sha256::digest(request.account_id.0.as_bytes())
    );
    let db = rusqlite::Connection::open_with_flags(
        directory.join(filename),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let mut statement = db.prepare("SELECT snapshot_json FROM inventory_items WHERE account_id = ?1 AND sync_root_id = ?2 ORDER BY item_id").unwrap();
    statement
        .query_map(
            rusqlite::params![request.account_id.0, scope(request)],
            |row| row.get::<_, String>(0),
        )
        .unwrap()
        .map(|row| serde_json::from_str(&row.unwrap()).unwrap())
        .collect()
}
#[tokio::main]
async fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(),7,"Usage: validate_stage2 ACCOUNTS_DIRECTORY OAUTH_FILE LOCAL_ROOT REMOTE_ROOT_ID EMAIL unauthorized|authorized|restart");
    let accounts = PathBuf::from(&args[1]);
    let engine = Engine::open(EngineConfig {
        data_directory: accounts.clone(),
        application_id: "com.rtosta.DriveSync.Devel".into(),
        google_oauth_file: Some(PathBuf::from(&args[2])),
    })
    .await
    .unwrap();
    let available = engine
        .list_accounts(ListAccountsRequest { provider: None })
        .await
        .unwrap();
    let found: Vec<_> = available
        .accounts
        .iter()
        .filter(|a| a.email.as_deref() == Some(args[5].as_str()))
        .collect();
    assert_eq!(
        found.len(),
        1,
        "Exactly one matching connected test account is required"
    );
    let request = StartSynchronizationRequest {
        account_id: found[0].id.clone(),
        local_root: PathBuf::from(&args[3]),
        remote_root_id: RemoteId(args[4].clone()),
    };
    let mut name = accounts.file_name().unwrap().to_os_string();
    name.push(".synchronization");
    let inventory = accounts.with_file_name(name);
    if args[6] == "authorized" {
        engine
            .start_synchronization_with_initial_association(request.clone())
            .await
            .unwrap();
    } else {
        assert!(args[6] == "unauthorized" || args[6] == "restart");
        engine.start_synchronization(request.clone()).await.unwrap();
    }
    let mut status = engine
        .subscribe_synchronization(request.account_id.clone())
        .await
        .unwrap();
    let mut previous = String::new();
    for round in 0..60 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let rows = snapshots(&inventory, &request);
        let synced = rows
            .iter()
            .filter(|row| row["baseline"]["state"] == "Synced")
            .count();
        let current = status.current();
        let message = format!(
            "mode={} records={} synced={} state={:?} issue={:?}",
            args[6],
            rows.len(),
            synced,
            current.state,
            current.issue.as_ref().map(|issue| issue.kind)
        );
        if message != previous {
            println!("{message}");
            previous = message;
        }
        if args[6] == "unauthorized" && rows.len() >= 3 {
            assert_eq!(synced, 0);
            break;
        }
        if args[6] != "unauthorized" && synced >= 5 && (args[6] != "restart" || round >= 9) {
            break;
        }
    }
    engine
        .stop_synchronization(request.account_id.clone())
        .await
        .unwrap();
    let rows = snapshots(&inventory, &request);
    let synced = rows
        .iter()
        .filter(|row| row["baseline"]["state"] == "Synced")
        .count();
    println!("final_records={} final_synced={}", rows.len(), synced);
    for row in &rows {
        println!(
            "item={} remote={} revision={} baseline={}",
            row["identity"]["item_id"],
            row["identity"]["remote_id"],
            row["revision"],
            row["baseline"]["state"]
        );
    }
    if args[6] == "unauthorized" {
        assert_eq!(synced, 0);
    } else {
        assert_eq!(synced, 5, "Expected three folders and two equal files");
    }
}
