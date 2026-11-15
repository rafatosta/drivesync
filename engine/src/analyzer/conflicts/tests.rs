//! Pure identification tests with synthetic snapshots and independent expectations.
use super::*;
use crate::inventory::{BaselineSide, SyncBaseline, SyncItem, SyncItemId, SyncStatus};
use crate::models::*;

fn request() -> ConflictRequest {
    let n = MetadataValue::NotCollected;
    let local = LocalFile {
        local_id: Some(LocalFileId("local".into())),
        relative_path: LocalRelativePath::new(b"parent/file".to_vec()).unwrap(),
        kind: MetadataValue::Known(LocalEntryKind::Regular),
        mode: n.clone(),
        owner: LocalOwner {
            uid: MetadataValue::NotCollected,
            gid: MetadataValue::NotCollected,
        },
        times: LocalTimes {
            modified_at: MetadataValue::NotCollected,
            changed_at: MetadataValue::NotCollected,
            created_at: MetadataValue::NotCollected,
        },
        storage: LocalStorage {
            size_bytes: MetadataValue::Known(42),
            hard_link_count: MetadataValue::NotCollected,
            inode: MetadataValue::NotCollected,
            filesystem_device: MetadataValue::NotCollected,
        },
        flags: LocalFileFlags {
            statx_attributes: MetadataValue::NotCollected,
            statx_attributes_mask: MetadataValue::NotCollected,
        },
        symlink: LocalSymlink {
            target: MetadataValue::NotApplicable,
        },
    };
    let remote = RemoteFile {
        reference: RemoteFileRef {
            account_id: AccountId("account".into()),
            file_id: RemoteFileId("remote".into()),
        },
        provider: ProviderId("google".into()),
        name: MetadataValue::Known("file".into()),
        kind: MetadataValue::Known(RemoteEntryKind::File),
        mime_type: MetadataValue::NotCollected,
        parent_ids: MetadataValue::Known(vec![RemoteFileId("parent".into())]),
        trashed: MetadataValue::Known(false),
        version: MetadataValue::Known("v1".into()),
        created_time: MetadataValue::NotCollected,
        modified_time: MetadataValue::NotCollected,
        size: MetadataValue::Known(42),
        md5_checksum: MetadataValue::NotCollected,
        sha256_checksum: MetadataValue::NotCollected,
        provider_data: MetadataValue::NotCollected,
    };
    let side = BaselineSide {
        presence: PresenceState::Present,
        name: None,
        local_name_bytes: None,
        parent_identity: None,
        version: Some("v1".into()),
        checksum: None,
        size: Some(42),
        modified_at: None,
    };
    let mut local_base = side.clone();
    local_base.local_name_bytes = Some(b"file".to_vec());
    local_base.parent_identity = Some(BaselineParent::Local(SyncItemId("parent".into())));
    let mut remote_base = side;
    remote_base.name = Some("file".into());
    remote_base.parent_identity = Some(BaselineParent::Remote(RemoteFileRef {
        account_id: AccountId("account".into()),
        file_id: RemoteFileId("parent".into()),
    }));
    ConflictRequest {
        item: SyncItem {
            id: SyncItemId("item".into()),
            kind: ItemKind::File,
            local_file_id: local.local_id.clone(),
            remote_ref: Some(remote.reference.clone()),
            local: Some(local),
            remote: Some(remote),
            local_latest: None,
            remote_latest: None,
            baseline: Some(SyncBaseline {
                local: local_base,
                remote: remote_base,
                confirmed_at: Timestamp { unix_millis: 1 },
            }),
            local_presence: PresenceState::Present,
            remote_presence: PresenceState::Present,
            status: SyncStatus::PendingReconciliation,
            conflict_reason: None,
            revision: 7,
            last_checked_at: None,
            last_synced_at: None,
        },
        evidence: ConflictEvidence {
            baseline_confirmed: true,
            local_scope_complete: true,
            remote_scope_complete: true,
            content: ContentEvidence {
                local_to_baseline: Equal,
                remote_to_baseline: Equal,
                current_sides: Equal,
            },
            local_parent: Some(SyncItemId("parent".into())),
            current_parents: Equal,
        },
    }
}

fn assessment(request: ConflictRequest) -> ConflictAssessment {
    evaluate(request).unwrap()
}
fn set_local_name(r: &mut ConflictRequest, name: &[u8]) {
    r.item.local.as_mut().unwrap().relative_path =
        LocalRelativePath::new([b"parent/".as_slice(), name].concat()).unwrap();
}
#[test]
fn equivalent_states_have_no_conflict_and_preserve_identity_revision_and_input() {
    let r = request();
    let before = r.clone();
    let a = assessment(r.clone());
    assert_eq!(r, before);
    assert_eq!(a.item_id, SyncItemId("item".into()));
    assert_eq!(a.item_revision, 7);
    assert_eq!(a.presence, ConflictPresence::NoConflict);
    assert!(!a.requires_user_intervention());
    assert!(a.requirements.is_empty());
    assert_eq!(
        a.findings,
        [
            ConflictDimension::Presence,
            ConflictDimension::Content,
            ConflictDimension::Name,
            ConflictDimension::Location
        ]
        .map(|dimension| ConflictFinding {
            dimension,
            status: FindingStatus::Compatible,
            reason: FindingReason::Equivalent
        })
    );
}
#[test]
fn content_matrix_distinguishes_intervention_from_missing_evidence() {
    for (local, remote, current, expected) in [
        (Different, Equal, Different, ConflictPresence::NoConflict),
        (Equal, Different, Different, ConflictPresence::NoConflict),
        (Different, Different, Different, ConflictPresence::Conflict),
        (Different, Different, Equal, ConflictPresence::NoConflict),
        (Unknown, Unknown, Unknown, ConflictPresence::Undetermined),
        (
            Different,
            Unknown,
            Different,
            ConflictPresence::Undetermined,
        ),
        (
            Different,
            Different,
            Unknown,
            ConflictPresence::Undetermined,
        ),
        (Unknown, Unknown, Equal, ConflictPresence::NoConflict),
    ] {
        let mut r = request();
        r.evidence.content = ContentEvidence {
            local_to_baseline: local,
            remote_to_baseline: remote,
            current_sides: current,
        };
        let a = assessment(r);
        assert_eq!(a.presence, expected);
        assert_eq!(
            a.requires_user_intervention(),
            expected == ConflictPresence::Conflict
        );
        assert_eq!(
            a.findings[1].status,
            match expected {
                ConflictPresence::NoConflict => FindingStatus::Compatible,
                ConflictPresence::Conflict => FindingStatus::Divergent,
                ConflictPresence::Undetermined => FindingStatus::Unknown,
            }
        );
        assert_eq!(
            a.requirements.is_empty(),
            expected != ConflictPresence::Undetermined
        );
    }
}
#[test]
fn names_are_compared_as_exact_bytes_without_selecting_a_version() {
    for (local, remote, expected) in [
        (b"new".as_slice(), "file", ConflictPresence::NoConflict),
        (b"file".as_slice(), "new", ConflictPresence::NoConflict),
        (b"same".as_slice(), "same", ConflictPresence::NoConflict),
        (b"one".as_slice(), "two", ConflictPresence::Conflict),
        (b"\xff".as_slice(), "different", ConflictPresence::Conflict),
    ] {
        let mut r = request();
        set_local_name(&mut r, local);
        r.item.remote.as_mut().unwrap().name = MetadataValue::Known(remote.into());
        let a = assessment(r);
        assert_eq!(a.presence, expected);
        assert_eq!(a.findings[2].dimension, ConflictDimension::Name);
    }
}
#[test]
fn independent_changes_remain_compatible() {
    let mut r = request();
    set_local_name(&mut r, b"renamed");
    r.evidence.content = ContentEvidence {
        local_to_baseline: Equal,
        remote_to_baseline: Different,
        current_sides: Different,
    };
    let a = assessment(r);
    assert_eq!(a.presence, ConflictPresence::NoConflict);
    assert_eq!(a.findings[1].reason, FindingReason::UnilateralChange);
    assert_eq!(a.findings[2].reason, FindingReason::UnilateralChange);
}
#[test]
fn conflicting_moves_and_names_are_identified_independently() {
    let mut r = request();
    set_local_name(&mut r, b"local-name");
    r.item.remote.as_mut().unwrap().name = MetadataValue::Known("remote-name".into());
    r.evidence.local_parent = Some(SyncItemId("local-parent".into()));
    r.item.remote.as_mut().unwrap().parent_ids =
        MetadataValue::Known(vec![RemoteFileId("remote-parent".into())]);
    r.evidence.current_parents = Different;
    let a = assessment(r.clone());
    assert!(a.requires_user_intervention());
    assert_eq!(
        a.findings
            .iter()
            .filter(|f| f.status == FindingStatus::Divergent)
            .map(|f| f.dimension)
            .collect::<Vec<_>>(),
        vec![ConflictDimension::Name, ConflictDimension::Location]
    );
    r.evidence.current_parents = Equal;
    assert_eq!(assessment(r).findings[3].status, FindingStatus::Compatible);
}
#[test]
fn deletion_versus_content_name_or_parent_change_needs_intervention_on_both_sides() {
    for local_survives in [true, false] {
        for dimension in [
            ConflictDimension::Content,
            ConflictDimension::Name,
            ConflictDimension::Location,
        ] {
            let mut r = request();
            r.evidence.content.current_sides = Unknown;
            if local_survives {
                r.item.remote_presence = PresenceState::AbsentConfirmed;
            } else {
                r.item.local_presence = PresenceState::AbsentConfirmed;
            }
            match (local_survives, dimension) {
                (true, ConflictDimension::Content) => {
                    r.evidence.content.local_to_baseline = Different
                }
                (false, ConflictDimension::Content) => {
                    r.evidence.content.remote_to_baseline = Different
                }
                (true, ConflictDimension::Name) => set_local_name(&mut r, b"renamed"),
                (false, ConflictDimension::Name) => {
                    r.item.remote.as_mut().unwrap().name = MetadataValue::Known("renamed".into())
                }
                (true, _) => r.evidence.local_parent = Some(SyncItemId("new-parent".into())),
                (false, _) => {
                    r.item.remote.as_mut().unwrap().parent_ids =
                        MetadataValue::Known(vec![RemoteFileId("new-parent".into())])
                }
            }
            let a = assessment(r);
            assert!(a.requires_user_intervention());
            assert_eq!(a.findings[0].reason, FindingReason::DeleteVsChange);
        }
    }
}
#[test]
fn unilateral_deletion_does_not_itself_create_a_user_conflict() {
    for local_survives in [true, false] {
        let mut r = request();
        r.evidence.content.current_sides = Unknown;
        if local_survives {
            r.item.remote_presence = PresenceState::AbsentConfirmed;
        } else {
            r.item.local_presence = PresenceState::AbsentConfirmed;
        }
        let a = assessment(r);
        assert_eq!(a.presence, ConflictPresence::NoConflict);
        assert!(!a.requires_user_intervention());
    }
}
#[test]
fn deletion_with_unknown_surviving_content_requires_evidence_not_a_choice() {
    let mut r = request();
    r.item.local_presence = PresenceState::AbsentConfirmed;
    r.evidence.content.current_sides = Unknown;
    r.evidence.content.remote_to_baseline = Unknown;
    let a = assessment(r);
    assert_eq!(a.presence, ConflictPresence::Undetermined);
    assert!(!a.requires_user_intervention());
    assert_eq!(a.requirements, vec![EvidenceRequirement::VerifyContent]);
}
#[test]
fn presence_and_coverage_matrix_never_promotes_unknown_absence() {
    for local in [
        PresenceState::Present,
        PresenceState::AbsentConfirmed,
        PresenceState::Unknown,
    ] {
        for remote in [
            PresenceState::Present,
            PresenceState::AbsentConfirmed,
            PresenceState::Unknown,
        ] {
            for complete in [false, true] {
                let mut r = request();
                r.item.local_presence = local.clone();
                r.item.remote_presence = remote.clone();
                r.evidence.content.current_sides = Unknown;
                r.evidence.local_scope_complete = complete;
                let a = assessment(r);
                if !complete || local == PresenceState::Unknown || remote == PresenceState::Unknown
                {
                    assert_eq!(a.presence, ConflictPresence::Undetermined);
                    assert!(!a.requirements.is_empty());
                } else if local == PresenceState::AbsentConfirmed
                    && remote == PresenceState::AbsentConfirmed
                {
                    assert_eq!(a.presence, ConflictPresence::NoConflict);
                }
            }
        }
    }
}
#[test]
fn first_sync_and_untrusted_baseline_do_not_fabricate_history() {
    for baseline_absent in [true, false] {
        let mut r = request();
        if baseline_absent {
            r.item.baseline = None;
        }
        r.evidence.baseline_confirmed = false;
        r.evidence.content.current_sides = Different;
        let a = assessment(r);
        assert_eq!(a.presence, ConflictPresence::Undetermined);
        assert!(a
            .requirements
            .contains(&EvidenceRequirement::VerifyBaseline));
    }
    let mut r = request();
    r.item.baseline = None;
    r.evidence.baseline_confirmed = false;
    assert_eq!(assessment(r).presence, ConflictPresence::NoConflict);
}
#[test]
fn new_items_and_directories_are_identified_without_resolution_actions() {
    for local_present in [true, false] {
        let mut r = request();
        r.item.baseline = None;
        r.evidence.baseline_confirmed = false;
        r.evidence.content.current_sides = Unknown;
        if local_present {
            r.item.remote_presence = PresenceState::AbsentConfirmed;
        } else {
            r.item.local_presence = PresenceState::AbsentConfirmed;
        }
        let a = assessment(r);
        assert_eq!(a.presence, ConflictPresence::NoConflict);
        assert_eq!(a.findings[0].reason, FindingReason::NewItem);
    }
    let mut r = request();
    r.item.kind = ItemKind::Directory;
    r.item.local.as_mut().unwrap().kind = MetadataValue::Known(LocalEntryKind::Directory);
    r.item.remote.as_mut().unwrap().kind = MetadataValue::Known(RemoteEntryKind::Directory);
    r.evidence.content.current_sides = Unknown;
    let a = assessment(r);
    assert_eq!(a.presence, ConflictPresence::NoConflict);
    assert_eq!(a.findings.len(), 3);
    assert!(!a
        .findings
        .iter()
        .any(|f| f.dimension == ConflictDimension::Content));
}
#[test]
fn latest_unknown_metadata_is_not_replaced_with_retained_values() {
    for missing in [
        MetadataValue::NotCollected,
        MetadataValue::Unsupported,
        MetadataValue::Unavailable,
        MetadataValue::PermissionDenied,
    ] {
        let mut r = request();
        let mut latest = r.item.remote.clone().unwrap();
        latest.name = missing;
        r.item.remote_latest = Some(latest);
        let a = assessment(r);
        assert_eq!(a.presence, ConflictPresence::Undetermined);
        assert_eq!(a.requirements, vec![EvidenceRequirement::VerifyName]);
    }
}
#[test]
fn proven_conflict_is_preserved_alongside_unknown_dimensions() {
    let mut r = request();
    r.evidence.content = ContentEvidence {
        local_to_baseline: Different,
        remote_to_baseline: Different,
        current_sides: Different,
    };
    r.item.remote.as_mut().unwrap().name = MetadataValue::Unavailable;
    let a = assessment(r);
    assert_eq!(a.presence, ConflictPresence::Conflict);
    assert!(a.requires_user_intervention());
    assert_eq!(a.requirements, vec![EvidenceRequirement::VerifyName]);
    assert_eq!(a.findings[2].status, FindingStatus::Unknown);
}
#[test]
fn wrong_qualified_identity_is_rejected_even_when_names_and_content_match() {
    let mut r = request();
    r.item.remote.as_mut().unwrap().reference.account_id = AccountId("another-account".into());
    assert_eq!(evaluate(r), Err(ConflictError::WrongItemIdentity));
    let mut r = request();
    r.item.local.as_mut().unwrap().local_id = Some(LocalFileId("another-file".into()));
    assert_eq!(evaluate(r), Err(ConflictError::WrongItemIdentity));
    let mut r = request();
    r.item.baseline.as_mut().unwrap().remote.parent_identity =
        Some(BaselineParent::Remote(RemoteFileRef {
            account_id: AccountId("other".into()),
            file_id: RemoteFileId("parent".into()),
        }));
    assert_eq!(evaluate(r), Err(ConflictError::WrongItemIdentity));
}
#[test]
fn invalid_requests_and_contradictory_evidence_are_typed_errors() {
    let mut r = request();
    r.item.id.0.clear();
    assert_eq!(evaluate(r), Err(ConflictError::InvalidRequest));
    let mut r = request();
    r.item.revision = 0;
    assert_eq!(evaluate(r), Err(ConflictError::InvalidRequest));
    let mut r = request();
    r.item.local = None;
    assert_eq!(evaluate(r), Err(ConflictError::InconsistentInput));
    let mut r = request();
    r.item.baseline = None;
    assert_eq!(evaluate(r), Err(ConflictError::InvalidEvidence));
    let mut r = request();
    r.evidence.content.current_sides = Different;
    assert_eq!(evaluate(r), Err(ConflictError::InvalidEvidence));
    let mut r = request();
    r.item.remote_presence = PresenceState::AbsentConfirmed;
    assert_eq!(evaluate(r), Err(ConflictError::InvalidEvidence));
}
#[test]
fn size_and_mtime_never_establish_content_equivalence() {
    let mut r = request();
    r.item.local.as_mut().unwrap().times.modified_at = MetadataValue::Known(LocalTimestamp {
        unix_seconds: 1,
        nanoseconds: 0,
    });
    r.item.remote.as_mut().unwrap().modified_time =
        MetadataValue::Known("1970-01-01T00:00:01Z".into());
    let baseline = r.item.baseline.as_mut().unwrap();
    baseline.local.modified_at = Some(Timestamp { unix_millis: 1000 });
    baseline.remote.modified_at = Some(Timestamp { unix_millis: 1000 });
    r.evidence.content = ContentEvidence {
        local_to_baseline: Unknown,
        remote_to_baseline: Unknown,
        current_sides: Unknown,
    };
    let a = assessment(r);
    assert_eq!(a.presence, ConflictPresence::Undetermined);
    assert_eq!(a.requirements, vec![EvidenceRequirement::VerifyContent]);
}
#[test]
fn repeated_and_interleaved_evaluations_have_identical_outputs() {
    let mut r = request();
    r.evidence.content = ContentEvidence {
        local_to_baseline: Different,
        remote_to_baseline: Different,
        current_sides: Different,
    };
    let expected = assessment(r.clone());
    for _ in 0..30 {
        assert_eq!(assessment(request()).presence, ConflictPresence::NoConflict);
        assert_eq!(assessment(r.clone()), expected);
    }
}

#[test]
fn unknown_presence_with_missing_snapshot_remains_undetermined() {
    let mut r = request();
    r.item.local = None;
    r.item.local_presence = PresenceState::Unknown;
    r.evidence.content.current_sides = Unknown;
    let a = assessment(r);
    assert_eq!(a.presence, ConflictPresence::Undetermined);
    assert_eq!(a.requirements, vec![EvidenceRequirement::ObserveLocal]);
}
#[test]
fn trash_uncertainty_and_unsupported_kind_do_not_create_a_user_choice() {
    for value in [MetadataValue::Known(true), MetadataValue::Unavailable] {
        let mut r = request();
        r.item.remote.as_mut().unwrap().trashed = value;
        let a = assessment(r);
        assert_eq!(a.presence, ConflictPresence::Undetermined);
        assert_eq!(a.requirements, vec![EvidenceRequirement::ObserveRemote]);
    }
    let mut r = request();
    r.item.kind = ItemKind::Other;
    r.evidence.content.current_sides = Unknown;
    let a = assessment(r);
    assert_eq!(a.presence, ConflictPresence::Undetermined);
    assert_eq!(a.requirements, vec![EvidenceRequirement::VerifyKind]);
}
#[test]
fn absent_side_with_unconfirmed_baseline_is_not_treated_as_a_new_item() {
    let mut r = request();
    r.item.local_presence = PresenceState::AbsentConfirmed;
    r.evidence.content.current_sides = Unknown;
    r.evidence.baseline_confirmed = false;
    let a = assessment(r);
    assert_eq!(a.presence, ConflictPresence::Undetermined);
    assert_eq!(a.requirements, vec![EvidenceRequirement::VerifyBaseline]);
}
#[test]
fn deleted_side_does_not_hide_unknown_evidence_when_survivor_changed() {
    let mut r = request();
    r.item.local_presence = PresenceState::AbsentConfirmed;
    r.evidence.content.current_sides = Unknown;
    r.evidence.content.remote_to_baseline = Different;
    r.item.remote.as_mut().unwrap().name = MetadataValue::Unavailable;
    let a = assessment(r);
    assert!(a.requires_user_intervention());
    assert_eq!(a.requirements, vec![EvidenceRequirement::VerifyName]);
}
#[test]
fn invalid_latest_provider_kind_and_parent_mapping_are_rejected() {
    let mut r = request();
    let mut latest = r.item.remote.clone().unwrap();
    latest.provider = ProviderId("another-provider".into());
    r.item.remote_latest = Some(latest);
    assert_eq!(evaluate(r), Err(ConflictError::WrongItemIdentity));
    let mut r = request();
    r.item.remote.as_mut().unwrap().kind = MetadataValue::Known(RemoteEntryKind::Directory);
    assert_eq!(evaluate(r), Err(ConflictError::InconsistentInput));
    let mut r = request();
    r.evidence.local_parent = Some(SyncItemId("".into()));
    assert_eq!(evaluate(r), Err(ConflictError::WrongItemIdentity));
}
