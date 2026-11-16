//! Pure model oracles; no filesystem, network, clock or credentials are used.
use super::*;

#[test]
fn relative_paths_preserve_unix_bytes_and_policy_reserved_names() {
    // MOD-PTH-01/03/04: these are literals, not normalized expectations.
    for bytes in [
        b"dir/file".as_slice(),
        b".hidden",
        b"..backup",
        b"a..b",
        b"with space/line\nbreak",
        b"back\\slash",
        b"invalid-\xff/name",
        b".drivesync/file",
    ] {
        let path = LocalRelativePath::new(bytes.to_vec()).unwrap();
        let borrowed: &[u8] = path.as_bytes();
        assert_eq!(borrowed, bytes);
        assert_eq!(borrowed.as_ptr(), path.as_bytes().as_ptr());
    }
}

#[test]
fn relative_paths_reject_root_escape_and_noncanonical_components() {
    // MOD-PTH-02: validate bytes before any physical path resolution.
    for bytes in [
        b"".as_slice(),
        b"/",
        b"/file",
        b"a/",
        b"a//b",
        b".",
        b"..",
        b"a/./b",
        b"a/../b",
        b"\0a",
        b"a\0b",
        b"a\0",
    ] {
        assert_eq!(
            LocalRelativePath::new(bytes.to_vec()),
            Err(ModelError::InvalidRelativePath)
        );
    }
}

#[test]
fn local_timestamps_preserve_negative_seconds_and_nanosecond_boundaries() {
    // MOD-TIM-01: no timezone conversion, rounding or wall-clock fallback.
    for seconds in [i64::MIN, -1, 0, 42, i64::MAX] {
        for nanos in [0, 999_999_999] {
            let timestamp = LocalTimestamp::new(seconds, nanos).unwrap();
            assert_eq!(timestamp.unix_seconds, seconds);
            assert_eq!(timestamp.nanoseconds, nanos);
        }
    }
}

#[test]
fn local_timestamps_reject_overflowing_nanoseconds_without_carry() {
    // MOD-TIM-02.
    for nanos in [1_000_000_000, u32::MAX] {
        assert_eq!(
            LocalTimestamp::new(0, nanos),
            Err(ModelError::InvalidTimestamp)
        );
    }
}

#[test]
fn metadata_states_do_not_collapse_observed_zero_false_or_empty_values() {
    // MOD-LOC-15.
    let states = [
        MetadataValue::Known(0),
        MetadataValue::NotCollected,
        MetadataValue::NotApplicable,
        MetadataValue::Unsupported,
        MetadataValue::PermissionDenied,
        MetadataValue::Unavailable,
    ];
    for (index, state) in states.iter().enumerate() {
        assert_eq!(*state, state.clone());
        for other in states.iter().skip(index + 1) {
            assert_ne!(state, other);
        }
    }
    assert_ne!(MetadataValue::Known(false), MetadataValue::NotCollected);
    assert_ne!(
        MetadataValue::Known(Vec::<u8>::new()),
        MetadataValue::Unavailable
    );
}

#[test]
fn existing_account_provider_and_timestamp_serialization_remains_compatible() {
    // MOD-ID-03/04: moving the module must preserve existing wire values.
    let account: crate::AccountId = AccountId("account-1".into());
    let provider: crate::ProviderId = ProviderId("google".into());
    let timestamp: crate::Timestamp = Timestamp { unix_millis: -1234 };
    assert_eq!(serde_json::to_string(&account).unwrap(), "\"account-1\"");
    assert_eq!(serde_json::to_string(&provider).unwrap(), "\"google\"");
    assert_eq!(
        serde_json::to_string(&timestamp).unwrap(),
        "{\"unix_millis\":-1234}"
    );
    assert_eq!(
        serde_json::from_str::<AccountId>("\"account-1\"").unwrap(),
        account
    );
}

fn regular_file() -> LocalFile {
    // MOD-VAL-01: intentionally non-default, independent fixture values.
    LocalFile {
        local_id: None,
        relative_path: LocalRelativePath::new(b"folder/file".to_vec()).unwrap(),
        kind: MetadataValue::Known(LocalEntryKind::Regular),
        mode: MetadataValue::Known(LocalFileMode {
            raw_mode: 0o100640,
            permission_bits: 0o640,
        }),
        owner: LocalOwner {
            uid: MetadataValue::Known(1001),
            gid: MetadataValue::Known(1002),
        },
        times: LocalTimes {
            modified_at: MetadataValue::Known(LocalTimestamp {
                unix_seconds: -3,
                nanoseconds: 123,
            }),
            changed_at: MetadataValue::Known(LocalTimestamp {
                unix_seconds: -7,
                nanoseconds: 456,
            }),
            created_at: MetadataValue::NotCollected,
        },
        storage: LocalStorage {
            size_bytes: MetadataValue::Known(23),
            hard_link_count: MetadataValue::Known(2),
            inode: MetadataValue::Known(987),
            filesystem_device: MetadataValue::Known(DeviceNumber { major: 8, minor: 2 }),
        },
        flags: LocalFileFlags {
            statx_attributes: MetadataValue::Known(0x44),
            statx_attributes_mask: MetadataValue::Known(0x40),
        },
        symlink: LocalSymlink {
            target: MetadataValue::NotApplicable,
        },
    }
}

#[test]
fn local_file_validates_all_entry_kinds_without_inferring_identity_or_mutating() {
    // MOD-LOC-01/02/03/16 and MOD-VAL-01/04.
    for (kind, raw_mode) in [
        (LocalEntryKind::Regular, 0o107777),
        (LocalEntryKind::Directory, 0o040750),
        (LocalEntryKind::Symlink, 0o120777),
        (LocalEntryKind::Fifo, 0o010600),
        (LocalEntryKind::Socket, 0o140600),
        (LocalEntryKind::BlockDevice, 0o060600),
        (LocalEntryKind::CharacterDevice, 0o020600),
        (LocalEntryKind::Other, 0o030600),
        (LocalEntryKind::Unknown, 0o100640),
    ] {
        let mut file = regular_file();
        file.kind = MetadataValue::Known(kind);
        file.mode = MetadataValue::Known(LocalFileMode {
            raw_mode,
            permission_bits: (raw_mode & 0o7777) as u16,
        });
        file.symlink.target = match kind {
            LocalEntryKind::Symlink => MetadataValue::Known(b"../outside/non-utf8-\xff".to_vec()),
            LocalEntryKind::Unknown => MetadataValue::NotCollected,
            _ => MetadataValue::NotApplicable,
        };
        let before = file.clone();
        assert_eq!(file.validate(), Ok(()));
        assert_eq!(file, before);
        assert_eq!(file.local_id, None);
    }
}

#[test]
fn local_file_accepts_real_zero_values_and_independent_timestamp_order() {
    // MOD-LOC-04/05/06: zero is not unknown; ctime is not creation or precedence.
    let mut file = regular_file();
    file.owner = LocalOwner {
        uid: MetadataValue::Known(0),
        gid: MetadataValue::Known(0),
    };
    file.storage = LocalStorage {
        size_bytes: MetadataValue::Known(0),
        hard_link_count: MetadataValue::Known(0),
        inode: MetadataValue::Known(0),
        filesystem_device: MetadataValue::Known(DeviceNumber { major: 0, minor: 0 }),
    };
    file.times.created_at =
        MetadataValue::Known(LocalTimestamp::new(i64::MAX, 999_999_999).unwrap());
    assert_eq!(file.validate(), Ok(()));
    assert_eq!(file.local_id, None);
}

#[test]
fn local_file_accepts_unknown_or_partial_basic_evidence_without_completing_it() {
    // MOD-LOC-17 and MOD-VAL-01/03/04: no coverage decision is made here.
    let mut file = regular_file();
    file.kind = MetadataValue::Unavailable;
    file.mode = MetadataValue::PermissionDenied;
    file.owner.uid = MetadataValue::Unsupported;
    file.owner.gid = MetadataValue::NotCollected;
    file.times.modified_at = MetadataValue::Unavailable;
    file.storage.inode = MetadataValue::NotCollected;
    file.symlink.target = MetadataValue::NotCollected;
    let before = file.clone();
    assert_eq!(file.validate(), Ok(()));
    assert_eq!(file, before);
}

#[test]
fn local_file_checks_supplied_logical_identity_without_creating_one() {
    // MOD-VAL-02: identity strings are opaque, never trimmed or regenerated.
    for text in ["", " ", "\t\n", "\u{2003}"] {
        let mut file = regular_file();
        file.local_id = Some(LocalFileId(text.into()));
        let before = file.clone();
        assert_eq!(file.validate(), Err(ModelError::EmptyIdentity));
        assert_eq!(file, before);
    }
    let mut file = regular_file();
    file.local_id = Some(LocalFileId(" opaque-id ".into()));
    assert_eq!(file.validate(), Ok(()));
    assert_eq!(file.local_id.unwrap().0, " opaque-id ");
}

#[test]
fn local_file_rejects_invalid_nanoseconds_in_every_known_timestamp() {
    // MOD-VAL-02: public collector literals cannot bypass nested validation.
    for mutate in [
        (|f: &mut LocalFile| {
            f.times.modified_at = MetadataValue::Known(LocalTimestamp {
                unix_seconds: 5,
                nanoseconds: 1_000_000_000,
            })
        }) as fn(&mut LocalFile),
        |f| {
            f.times.changed_at = MetadataValue::Known(LocalTimestamp {
                unix_seconds: 5,
                nanoseconds: u32::MAX,
            })
        },
        |f| {
            f.times.created_at = MetadataValue::Known(LocalTimestamp {
                unix_seconds: 5,
                nanoseconds: 1_000_000_000,
            })
        },
    ] {
        let mut file = regular_file();
        mutate(&mut file);
        assert_eq!(file.validate(), Err(ModelError::InvalidTimestamp));
    }
}

#[test]
fn local_file_rejects_inconsistent_mode_permissions_or_type() {
    // MOD-VAL-02: independent mode literals exercise both relationships.
    for (kind, raw_mode, permissions) in [
        (LocalEntryKind::Regular, 0o100640, 0o777),
        (LocalEntryKind::Regular, 0o100640, 0o10640),
        (LocalEntryKind::Directory, 0o100640, 0o640),
        (LocalEntryKind::Other, 0o100640, 0o640),
    ] {
        let mut file = regular_file();
        file.kind = MetadataValue::Known(kind);
        file.mode = MetadataValue::Known(LocalFileMode {
            raw_mode,
            permission_bits: permissions,
        });
        assert_eq!(file.validate(), Err(ModelError::InconsistentMetadata));
    }
}

#[test]
fn local_file_flags_require_a_known_pair_or_the_same_unavailable_state() {
    // MOD-LOC-08 and MOD-VAL-02/03: unsupported bits are not reinterpreted.
    let states = [
        MetadataValue::Known(0),
        MetadataValue::NotCollected,
        MetadataValue::Unsupported,
        MetadataValue::PermissionDenied,
        MetadataValue::Unavailable,
    ];
    for (index, attributes) in states.iter().enumerate() {
        for (other_index, mask) in states.iter().enumerate() {
            let mut file = regular_file();
            file.flags = LocalFileFlags {
                statx_attributes: attributes.clone(),
                statx_attributes_mask: mask.clone(),
            };
            assert_eq!(
                file.validate(),
                if index == other_index {
                    Ok(())
                } else {
                    Err(ModelError::InconsistentMetadata)
                }
            );
        }
    }
    // Attribute bits outside the support mask stay raw; validation does not erase them.
    assert_eq!(regular_file().validate(), Ok(()));
}

#[test]
fn local_file_optional_birth_time_and_flags_preserve_each_absence_state() {
    // MOD-VAL-03: optional absence is still meaningful evidence, not zero.
    for state in [
        MetadataValue::NotCollected,
        MetadataValue::Unsupported,
        MetadataValue::PermissionDenied,
        MetadataValue::Unavailable,
    ] {
        let mut file = regular_file();
        file.flags = LocalFileFlags {
            statx_attributes: state.clone(),
            statx_attributes_mask: state.clone(),
        };
        file.times.created_at = match state {
            MetadataValue::NotCollected => MetadataValue::NotCollected,
            MetadataValue::Unsupported => MetadataValue::Unsupported,
            MetadataValue::PermissionDenied => MetadataValue::PermissionDenied,
            MetadataValue::Unavailable => MetadataValue::Unavailable,
            _ => unreachable!(),
        };
        let before = file.clone();
        assert_eq!(file.validate(), Ok(()));
        assert_eq!(file, before);
    }
}

#[test]
fn not_applicable_is_rejected_outside_the_known_non_symlink_target() {
    // MOD-VAL-02/03: every retained attribute is checked, including optional ones.
    let cases: &[fn(&mut LocalFile)] = &[
        |f| f.kind = MetadataValue::NotApplicable,
        |f| f.mode = MetadataValue::NotApplicable,
        |f| f.owner.uid = MetadataValue::NotApplicable,
        |f| f.owner.gid = MetadataValue::NotApplicable,
        |f| f.times.modified_at = MetadataValue::NotApplicable,
        |f| f.times.changed_at = MetadataValue::NotApplicable,
        |f| f.times.created_at = MetadataValue::NotApplicable,
        |f| f.storage.size_bytes = MetadataValue::NotApplicable,
        |f| f.storage.hard_link_count = MetadataValue::NotApplicable,
        |f| f.storage.inode = MetadataValue::NotApplicable,
        |f| f.storage.filesystem_device = MetadataValue::NotApplicable,
        |f| {
            f.flags.statx_attributes = MetadataValue::NotApplicable;
            f.flags.statx_attributes_mask = MetadataValue::NotApplicable;
        },
    ];
    for mutate in cases {
        let mut file = regular_file();
        mutate(&mut file);
        assert_eq!(file.validate(), Err(ModelError::InconsistentMetadata));
    }
}

#[test]
fn symlink_target_states_require_matching_entry_kind_without_resolving_bytes() {
    // MOD-LOC-13: absolute/escaping targets are literal values, never entry paths.
    for bytes in [
        b"/outside/target".as_slice(),
        b"../../elsewhere",
        b"\xff/name",
    ] {
        let mut file = regular_file();
        file.kind = MetadataValue::Known(LocalEntryKind::Symlink);
        file.mode = MetadataValue::Known(LocalFileMode {
            raw_mode: 0o120777,
            permission_bits: 0o777,
        });
        file.symlink.target = MetadataValue::Known(bytes.to_vec());
        assert_eq!(file.validate(), Ok(()));
        assert_eq!(file.symlink.target, MetadataValue::Known(bytes.to_vec()));
    }
    for (kind, target) in [
        (
            MetadataValue::Known(LocalEntryKind::Regular),
            MetadataValue::NotCollected,
        ),
        (
            MetadataValue::Known(LocalEntryKind::Regular),
            MetadataValue::Known(b"elsewhere".to_vec()),
        ),
        (
            MetadataValue::Known(LocalEntryKind::Symlink),
            MetadataValue::NotApplicable,
        ),
        (
            MetadataValue::Known(LocalEntryKind::Unknown),
            MetadataValue::NotApplicable,
        ),
        (MetadataValue::NotCollected, MetadataValue::NotApplicable),
    ] {
        let mut file = regular_file();
        file.kind = kind;
        file.mode = MetadataValue::NotCollected;
        file.symlink.target = target;
        assert_eq!(file.validate(), Err(ModelError::InconsistentMetadata));
    }
}

#[test]
fn validation_errors_use_static_diagnostics_without_entry_metadata() {
    // MOD-VAL-02: actual failed validation must not expose supplied metadata.
    let mut file = regular_file();
    file.relative_path = LocalRelativePath::new(b"private/personal-document".to_vec()).unwrap();
    file.local_id = Some(LocalFileId(" ".into()));
    let error = file.validate().unwrap_err();
    assert_eq!(
        format!("{error}: {error:?}"),
        "empty identity: EmptyIdentity"
    );
    assert!(std::error::Error::source(&error).is_none());
    let path_error = LocalRelativePath::new(b"private/../personal-document".to_vec()).unwrap_err();
    assert_eq!(path_error.to_string(), "invalid relative path");
}

#[test]
fn symlink_and_unknown_kind_allow_partial_targets_but_not_non_applicability() {
    // MOD-VAL-03: expected outcomes are explicit, independent of the validator.
    for kind in [
        MetadataValue::Known(LocalEntryKind::Symlink),
        MetadataValue::Known(LocalEntryKind::Unknown),
        MetadataValue::NotCollected,
        MetadataValue::Unsupported,
        MetadataValue::PermissionDenied,
        MetadataValue::Unavailable,
    ] {
        for target in [
            MetadataValue::Known(b"/literal/target".to_vec()),
            MetadataValue::NotCollected,
            MetadataValue::Unsupported,
            MetadataValue::PermissionDenied,
            MetadataValue::Unavailable,
        ] {
            let mut file = regular_file();
            file.kind = kind.clone();
            file.mode = MetadataValue::NotCollected;
            file.symlink.target = target;
            assert_eq!(file.validate(), Ok(()));
        }
    }
}
