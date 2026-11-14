//! Pure local envelope invariants, independent of watcher and storage implementations.
use super::*;
fn local_file() -> LocalFile {
    LocalFile {
        local_id: None,
        relative_path: LocalRelativePath::new(b"file".to_vec()).unwrap(),
        kind: MetadataValue::Known(LocalEntryKind::Regular),
        mode: MetadataValue::Known(LocalFileMode {
            raw_mode: 0o100600,
            permission_bits: 0o600,
        }),
        owner: LocalOwner {
            uid: MetadataValue::Known(1),
            gid: MetadataValue::Known(2),
        },
        times: LocalTimes {
            modified_at: MetadataValue::Known(LocalTimestamp::new(-1, 7).unwrap()),
            changed_at: MetadataValue::Known(LocalTimestamp::new(0, 8).unwrap()),
            created_at: MetadataValue::Unsupported,
        },
        storage: LocalStorage {
            size_bytes: MetadataValue::Known(0),
            hard_link_count: MetadataValue::Known(1),
            inode: MetadataValue::Known(3),
            filesystem_device: MetadataValue::Known(DeviceNumber { major: 8, minor: 1 }),
        },
        flags: LocalFileFlags {
            statx_attributes: MetadataValue::Unsupported,
            statx_attributes_mask: MetadataValue::Unsupported,
        },
        symlink: LocalSymlink {
            target: MetadataValue::NotApplicable,
        },
    }
}
fn event(kind: FileEventKind) -> File {
    let path = LocalRelativePath::new(b"file".to_vec()).unwrap();
    File {
        id: FileId("event-id".into()),
        sync_id: SyncId("sync".into()),
        observed_at: Timestamp { unix_millis: 42 },
        coverage: ObservationCoverage::Complete,
        scope: FileScope::Local(path.clone()),
        event: FileEvent {
            local: None,
            remote: None,
            kind,
            uncertainty: (kind == FileEventKind::RecheckRequired)
                .then_some(ObservationUncertainty::ObservationUnstable),
            previous: None,
            current: Some(FileLocation::Local(path)),
        },
        data: Some(FileData::Local(local_file())),
    }
}
#[test]
fn local_envelopes_validate_all_eight_event_kinds_and_preserve_complete_payload() {
    for kind in [
        FileEventKind::Created,
        FileEventKind::Modified,
        FileEventKind::MetadataChanged,
        FileEventKind::Renamed,
        FileEventKind::Moved,
        FileEventKind::Deleted,
        FileEventKind::RecheckRequired,
        FileEventKind::ScopeUnavailable,
    ] {
        let mut file = event(kind);
        match kind {
            FileEventKind::Renamed | FileEventKind::Moved => {
                file.event.previous = Some(FileLocation::Local(
                    LocalRelativePath::new(b"old".to_vec()).unwrap(),
                ))
            }
            FileEventKind::Deleted => {
                file.event.previous = file.event.current.take();
                file.data = None;
            }
            FileEventKind::ScopeUnavailable => {
                file.event.current = None;
                file.data = None;
                file.coverage = ObservationCoverage::Unavailable;
            }
            _ => {}
        }
        let before = file.clone();
        assert_eq!(file.validate(), Ok(()));
        assert_eq!(file, before);
    }
}
#[test]
fn whole_sync_only_accepts_uncertainty_without_data_or_locations() {
    for kind in [
        FileEventKind::RecheckRequired,
        FileEventKind::ScopeUnavailable,
    ] {
        let mut file = event(kind);
        file.scope = FileScope::WholeSync;
        file.data = None;
        file.event.current = None;
        file.coverage = ObservationCoverage::Unavailable;
        assert_eq!(file.validate(), Ok(()));
        file.event.previous = Some(FileLocation::Local(
            LocalRelativePath::new(b"file".to_vec()).unwrap(),
        ));
        assert_eq!(file.validate(), Err(ModelError::InconsistentEvent));
    }
    let mut file = event(FileEventKind::Created);
    file.scope = FileScope::WholeSync;
    assert_eq!(file.validate(), Err(ModelError::InconsistentEvent));
}
#[test]
fn local_envelope_rejects_missing_evidence_wrong_scope_or_claimed_complete_partial_data() {
    let mutations: &[fn(&mut File)] = &[
        |f| f.id = FileId(" ".into()),
        |f| f.sync_id = SyncId("".into()),
        |f| f.data = None,
        |f| f.event.current = None,
        |f| f.coverage = ObservationCoverage::Unavailable,
        |f| f.scope = FileScope::Local(LocalRelativePath::new(b"other".to_vec()).unwrap()),
    ];
    for mutate in mutations {
        let mut file = event(FileEventKind::Created);
        mutate(&mut file);
        assert!(file.validate().is_err());
    }
    let mut file = event(FileEventKind::Created);
    if let Some(FileData::Local(data)) = &mut file.data {
        data.owner.uid = MetadataValue::Unavailable;
    }
    assert_eq!(file.validate(), Err(ModelError::InconsistentEvent));
    file.coverage = ObservationCoverage::Partial;
    assert_eq!(file.validate(), Ok(()));
}
#[test]
fn remote_locations_cannot_be_inserted_into_local_scopes() {
    let mut file = event(FileEventKind::RecheckRequired);
    file.event.current = Some(FileLocation::Remote {
        reference: RemoteFileRef {
            account_id: AccountId("account".into()),
            file_id: RemoteFileId("remote".into()),
        },
        parent_id: None,
        name: None,
    });
    assert_eq!(file.validate(), Err(ModelError::InconsistentEvent));
    // Consumers can still select an uninhabited remote parameter for local-only data.
    fn local_only(data: FileData<std::convert::Infallible>) {
        match data {
            FileData::Local(_) => {}
            FileData::Remote(impossible) => match impossible {},
        }
    }
    local_only(FileData::Local(local_file()));
}

#[test]
fn rechecks_require_a_reason_and_classified_events_cannot_carry_uncertainty() {
    let mut file = event(FileEventKind::RecheckRequired);
    file.event.uncertainty = None;
    assert_eq!(file.validate(), Err(ModelError::InconsistentEvent));
    let mut file = event(FileEventKind::Created);
    file.event.uncertainty = Some(ObservationUncertainty::ObservationUnstable);
    assert_eq!(file.validate(), Err(ModelError::InconsistentEvent));
}

#[test]
fn remote_observation_preserves_raw_removal_without_metadata_or_inferred_action() {
    let mut file = File {
        id: FileId("observation".into()),
        sync_id: SyncId("sync".into()),
        event: FileEvent {
            local: None,
            kind: FileEventKind::Observed,
            remote: Some(RemoteObservation {
                source: RemoteObservationSource::Changes,
                removed: Some(true),
                time: Some("2026-11-14T12:00:00Z".into()),
                change_type: Some("file".into()),
                drive_id: None,
            }),
            uncertainty: None,
            previous: None,
            current: None,
        },
        observed_at: Timestamp { unix_millis: 1 },
        coverage: ObservationCoverage::Partial,
        scope: FileScope::Remote(RemoteFileRef {
            account_id: AccountId("account".into()),
            file_id: RemoteFileId("remote-file".into()),
        }),
        data: None,
    };
    assert_eq!(file.validate(), Ok(()));
    let bytes = serde_json::to_vec(&file).unwrap();
    assert_eq!(serde_json::from_slice::<File>(&bytes).unwrap(), file);
    file.event.kind = FileEventKind::Deleted;
    assert_eq!(file.validate(), Err(ModelError::InconsistentEvent));
    file.event.kind = FileEventKind::Observed;
    file.event.remote = None;
    assert_eq!(file.validate(), Err(ModelError::InconsistentEvent));
}

#[test]
fn local_native_observation_preserves_signals_without_previous_location_or_metadata() {
    let mut file = event(FileEventKind::Observed);
    file.coverage = ObservationCoverage::Partial;
    file.event.local = Some(LocalObservation {
        capture_id: "session".into(),
        mask: libc::IN_MOVED_FROM,
        cookie: 42,
    });
    file.data = None;
    file.event.current = None;
    assert_eq!(file.validate(), Ok(()));
    let bytes = serde_json::to_vec(&file).unwrap();
    assert_eq!(serde_json::from_slice::<File>(&bytes).unwrap(), file);
    file.event.previous = Some(FileLocation::Local(
        LocalRelativePath::new(b"before".to_vec()).unwrap(),
    ));
    assert_eq!(file.validate(), Err(ModelError::InconsistentEvent));
    file.event.previous = None;
    file.event.local.as_mut().unwrap().mask = 0;
    assert_eq!(file.validate(), Err(ModelError::InconsistentEvent));
}
