use super::*;
use crate::{
    detectors::observation::{LocalObservation, RemoteObservation},
    inventory::models::BaselineState,
    models::{
        AccountId, Checksum, DateTime, FileKind, ObservationEvent, ObservationKind, Path,
        ProviderId, RemoteId,
    },
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_DIRECTORY_ID: AtomicU64 = AtomicU64::new(0);

fn context() -> (Context, PathBuf) {
    let directory = std::env::temp_dir().join(format!(
        "drivesync-context-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&directory);
    let context = Context::open(ContextConfig {
        inventory_directory: directory.clone(),
        account_id: AccountId("account-a".into()),
        provider_id: ProviderId("google-drive".into()),
        sync_root_id: SyncRootId("root-a".into()),
        local_root: PathRoot(PathBuf::from("/sync")),
        remote_root_id: RemoteId("drive-root".into()),
        local_root_available: true,
        remote_available: false,
        initial_association_authorized: false,
    })
    .unwrap();
    (context, directory)
}

fn local_observation() -> LocalObservation {
    LocalObservation {
        sync_root_id: SyncRootId("root-a".into()),
        state: FileState::Present,
        path: Some(PathBuf::from("folder/file.txt")),
        previous_path: None,
        name: Evidence::Known("file.txt".into()),
        parent: Evidence::Known(PathBuf::from("folder")),
        kind: Evidence::Known(FileKind::Regular),
        identity: Evidence::NotCollected,
        size: Evidence::Known(7),
        modified_at: Evidence::Known(DateTime { unix_millis: 5 }),
        changed_at: Evidence::NotCollected,
        checksum: Evidence::Known(Checksum {
            algorithm: "sha256".into(),
            value: "abc".into(),
        }),
        capabilities: Evidence::NotCollected,
        observed_at: DateTime { unix_millis: 6 },
        coverage: ObservationCoverage::Complete,
        event: None,
    }
}

#[test]
fn local_scanner_builds_file_without_fabricating_events_or_remote_facts() {
    let (context, directory) = context();
    let file = context
        .build(Observation::Local {
            source: LocalObservationSource::Scanner,
            data: local_observation(),
        })
        .unwrap();
    assert_eq!(file.inventory_lookup, InventoryLookupStatus::NotFound);
    assert_eq!(file.baseline.state, BaselineState::NeverSynced);
    assert_eq!(file.local.state, FileState::Present);
    assert_eq!(file.remote.state, FileState::Unknown);
    assert_eq!(file.observations.latest_origin, ObservationOrigin::Scanner);
    assert!(file.observations.events.is_empty());
    assert_eq!(
        file.observations.local_coverage,
        ObservationCoverage::Complete
    );
    assert_eq!(
        file.observations.remote_coverage,
        ObservationCoverage::Unavailable
    );
    assert_eq!(file.inventory_revision, None);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn remote_observation_preserves_event_and_pagination_evidence() {
    let (context, directory) = context();
    let mut remote = RemoteObservation {
        account_id: AccountId("account-a".into()),
        provider_id: ProviderId("google-drive".into()),
        sync_root_id: SyncRootId("root-a".into()),
        state: FileState::Present,
        remote_id: Some(RemoteId("remote-a".into())),
        name: Evidence::Known("file.txt".into()),
        parents: Evidence::Known(vec![RemoteId("drive-root".into())]),
        mime_type: Evidence::Known("text/plain".into()),
        size: Evidence::Known(7),
        checksum: Evidence::NotCollected,
        version: Evidence::Known(3),
        modified_time: Evidence::NotCollected,
        trashed: Evidence::Known(false),
        capabilities: Evidence::NotCollected,
        observed_at: DateTime { unix_millis: 9 },
        coverage: ObservationCoverage::Partial,
        page_token: Some("page-2".into()),
        event: Some(ObservationEvent {
            origin: ObservationOrigin::Remote,
            kind: ObservationKind::Modified,
            observed_at: DateTime { unix_millis: 8 },
            old_path: None,
            new_path: None,
            remote_id: Some(RemoteId("remote-a".into())),
            move_cookie: None,
        }),
    };
    let file = context.build(Observation::Remote(remote.clone())).unwrap();
    assert_eq!(file.inventory_lookup, InventoryLookupStatus::NotFound);
    assert_eq!(file.remote.id, Some(RemoteId("remote-a".into())));
    assert_eq!(file.local.state, FileState::Unknown);
    assert_eq!(
        file.observations.remote_coverage,
        ObservationCoverage::Partial
    );
    assert_eq!(
        file.observations.remote_page_token.as_deref(),
        Some("page-2")
    );
    assert_eq!(file.observations.events, vec![remote.event.take().unwrap()]);

    remote.account_id = AccountId("account-b".into());
    assert_eq!(
        context.build(Observation::Remote(remote)),
        Err(ContextError::ScopeMismatch)
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn escaping_local_paths_are_rejected() {
    let (context, directory) = context();
    let mut observation = local_observation();
    observation.path = Some(Path::from("../outside"));
    assert_eq!(
        context.build(Observation::Local {
            source: LocalObservationSource::Watcher,
            data: observation,
        }),
        Err(ContextError::PathOutsideRoot)
    );
    fs::remove_dir_all(directory).unwrap();
}
