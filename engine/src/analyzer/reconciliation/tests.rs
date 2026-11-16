//! Synthetic, independent expectations for the pure three-way decision core.
use super::*;
use crate::inventory::{BaselineSide, SyncBaseline, SyncItemId, SyncStatus};
use crate::models::*;

fn request() -> ReconciliationRequest {
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
    ReconciliationRequest {
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
        evidence: ReconciliationEvidence {
            local_observation: ObservationConfidence::Complete,
            remote_observation: ObservationConfidence::Complete,
            local_content: Change::Unchanged,
            remote_content: Change::Unchanged,
            content_comparison: Comparison::Equal,
            local_parent: Some(SyncItemId("parent".into())),
            parent_comparison: Comparison::Equal,
        },
    }
}
fn propagate(dimension: Dimension, source: Side) -> Decision {
    Decision::Propagate { dimension, source }
}
fn outcome(r: ReconciliationRequest) -> ReconciliationAssessment {
    reconcile(r).unwrap()
}

#[test]
fn unchanged_preserves_item_revision_and_input() {
    let r = request();
    let before = r.clone();
    let a = outcome(r.clone());
    assert_eq!(r, before);
    assert_eq!(
        a,
        ReconciliationAssessment {
            item_id: SyncItemId("item".into()),
            item_revision: 7,
            decisions: vec![],
            divergences: vec![],
            requirements: vec![]
        }
    );
}
#[test]
fn content_matrix_never_uses_size_or_version_as_content_proof() {
    for (local, remote, cross, decisions, conflicts, requirements) in [
        (
            Change::Changed,
            Change::Unchanged,
            Comparison::Different,
            vec![propagate(Dimension::Content, Side::Local)],
            0,
            vec![],
        ),
        (
            Change::Unchanged,
            Change::Changed,
            Comparison::Different,
            vec![propagate(Dimension::Content, Side::Remote)],
            0,
            vec![],
        ),
        (
            Change::Changed,
            Change::Changed,
            Comparison::Different,
            vec![],
            1,
            vec![],
        ),
        (
            Change::Changed,
            Change::Changed,
            Comparison::Equal,
            vec![],
            0,
            vec![],
        ),
        (
            Change::Unknown,
            Change::Unknown,
            Comparison::Unknown,
            vec![],
            0,
            vec![Requirement::Compare(Dimension::Content)],
        ),
        (
            Change::Changed,
            Change::Unknown,
            Comparison::Different,
            vec![],
            0,
            vec![Requirement::Compare(Dimension::Content)],
        ),
        (
            Change::Changed,
            Change::Changed,
            Comparison::Unknown,
            vec![],
            0,
            vec![Requirement::Compare(Dimension::Content)],
        ),
    ] {
        let mut r = request();
        r.evidence.local_content = local;
        r.evidence.remote_content = remote;
        r.evidence.content_comparison = cross;
        let a = outcome(r);
        assert_eq!(a.decisions, decisions);
        assert_eq!(a.divergences.len(), conflicts);
        assert_eq!(a.requirements, requirements);
        if conflicts == 1 {
            assert_eq!(
                a.divergences,
                vec![Divergence {
                    dimension: Dimension::Content,
                    reason: DivergenceReason::ConcurrentChanges
                }]
            );
        }
    }
}
#[test]
fn names_use_exact_bytes_and_detect_convergence_and_conflict() {
    for (local, remote, decisions, conflict) in [
        (
            b"new".as_slice(),
            "file",
            vec![propagate(Dimension::Name, Side::Local)],
            false,
        ),
        (
            b"file".as_slice(),
            "new",
            vec![propagate(Dimension::Name, Side::Remote)],
            false,
        ),
        (b"new".as_slice(), "new", vec![], false),
        (b"one".as_slice(), "two", vec![], true),
        (
            b"\xff".as_slice(),
            "file",
            vec![propagate(Dimension::Name, Side::Local)],
            false,
        ),
    ] {
        let mut r = request();
        r.item.local.as_mut().unwrap().relative_path =
            LocalRelativePath::new([b"parent/".as_slice(), local].concat()).unwrap();
        r.item.remote.as_mut().unwrap().name = MetadataValue::Known(remote.into());
        let a = outcome(r);
        assert_eq!(a.decisions, decisions);
        assert_eq!(a.divergences.len(), usize::from(conflict));
    }
}
#[test]
fn moves_compare_each_side_to_its_own_baseline() {
    for side in [Side::Local, Side::Remote] {
        let mut r = request();
        r.evidence.parent_comparison = Comparison::Different;
        match side {
            Side::Local => r.evidence.local_parent = Some(SyncItemId("other".into())),
            Side::Remote => {
                r.item.remote.as_mut().unwrap().parent_ids =
                    MetadataValue::Known(vec![RemoteFileId("other".into())])
            }
        }
        assert_eq!(
            outcome(r).decisions,
            vec![propagate(Dimension::Location, side)]
        );
    }
    let mut r = request();
    r.evidence.local_parent = Some(SyncItemId("other".into()));
    r.item.remote.as_mut().unwrap().parent_ids =
        MetadataValue::Known(vec![RemoteFileId("another".into())]);
    r.evidence.parent_comparison = Comparison::Unknown;
    assert_eq!(
        outcome(r).requirements,
        vec![Requirement::Compare(Dimension::Location)]
    );
}
#[test]
fn independent_changes_have_stable_order_even_with_content_conflict() {
    let mut r = request();
    r.evidence.remote_content = Change::Changed;
    r.evidence.content_comparison = Comparison::Different;
    r.item.local.as_mut().unwrap().relative_path =
        LocalRelativePath::new(b"parent/renamed".to_vec()).unwrap();
    r.evidence.local_parent = Some(SyncItemId("moved".into()));
    r.evidence.parent_comparison = Comparison::Different;
    assert_eq!(
        outcome(r.clone()).decisions,
        vec![
            propagate(Dimension::Content, Side::Remote),
            propagate(Dimension::Name, Side::Local),
            propagate(Dimension::Location, Side::Local)
        ]
    );
    r.evidence.local_content = Change::Changed;
    let a = outcome(r.clone());
    assert_eq!(
        a.decisions,
        vec![
            propagate(Dimension::Name, Side::Local),
            propagate(Dimension::Location, Side::Local)
        ]
    );
    assert_eq!(a.divergences[0].reason, DivergenceReason::ConcurrentChanges);
    for _ in 0..20 {
        assert_eq!(outcome(r.clone()), a);
    }
}
#[test]
fn creation_is_an_intent_blocked_on_destination_resolution() {
    for source in [Side::Local, Side::Remote] {
        for kind in [ItemKind::File, ItemKind::Directory] {
            let mut r = request();
            r.item.baseline = None;
            r.item.kind = kind.clone();
            if kind == ItemKind::Directory {
                r.item.local.as_mut().unwrap().kind =
                    MetadataValue::Known(LocalEntryKind::Directory);
                r.item.remote.as_mut().unwrap().kind =
                    MetadataValue::Known(RemoteEntryKind::Directory);
            }
            r.evidence.local_content = Change::Unknown;
            r.evidence.remote_content = Change::Unknown;
            r.evidence.content_comparison = Comparison::Unknown;
            match source {
                Side::Local => r.item.remote_presence = PresenceState::AbsentConfirmed,
                Side::Remote => r.item.local_presence = PresenceState::AbsentConfirmed,
            }
            let a = outcome(r);
            assert_eq!(a.decisions, vec![Decision::Create { source, kind }]);
            assert_eq!(a.requirements, vec![Requirement::ResolveDestination]);
        }
    }
}
#[test]
fn deletion_requires_policy_and_delete_vs_change_is_a_conflict() {
    for survivor in [Side::Local, Side::Remote] {
        let mut r = request();
        r.evidence.content_comparison = Comparison::Unknown;
        match survivor {
            Side::Local => r.item.remote_presence = PresenceState::AbsentConfirmed,
            Side::Remote => r.item.local_presence = PresenceState::AbsentConfirmed,
        }
        assert_eq!(
            outcome(r.clone()).requirements,
            vec![Requirement::AuthorizeDeletion]
        );
        match survivor {
            Side::Local => r.evidence.local_content = Change::Unknown,
            Side::Remote => r.evidence.remote_content = Change::Unknown,
        }
        assert_eq!(
            outcome(r.clone()).requirements,
            vec![Requirement::VerifySurvivingState]
        );
        match survivor {
            Side::Local => r.evidence.local_content = Change::Changed,
            Side::Remote => r.evidence.remote_content = Change::Changed,
        }
        let a = outcome(r);
        assert!(a.decisions.is_empty());
        assert_eq!(
            a.divergences,
            vec![Divergence {
                dimension: Dimension::Presence,
                reason: DivergenceReason::DeleteVsChange
            }]
        );
    }
}
#[test]
fn all_presence_and_coverage_combinations_preserve_uncertainty() {
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
            for coverage in [
                ObservationConfidence::Complete,
                ObservationConfidence::Partial,
                ObservationConfidence::Unknown,
            ] {
                let mut r = request();
                r.item.local_presence = local.clone();
                r.item.remote_presence = remote.clone();
                r.evidence.content_comparison = Comparison::Unknown;
                r.evidence.local_observation = coverage;
                let a = outcome(r);
                if local == PresenceState::Unknown
                    || remote == PresenceState::Unknown
                    || coverage != ObservationConfidence::Complete
                {
                    assert!(a.decisions.is_empty());
                    assert!(!a.requirements.is_empty());
                }
            }
        }
    }
}
#[test]
fn missing_snapshot_is_not_an_absence_proof() {
    let mut r = request();
    r.item.local = None;
    assert_eq!(
        reconcile(r.clone()),
        Err(ReconciliationError::InconsistentInput)
    );
    r.item.local_presence = PresenceState::Unknown;
    r.evidence.content_comparison = Comparison::Unknown;
    assert_eq!(
        outcome(r).requirements,
        vec![Requirement::Observe(Side::Local)]
    );
}
#[test]
fn first_sync_never_overwrites_preexisting_versions() {
    let mut r = request();
    r.item.baseline = None;
    r.evidence.local_content = Change::Unknown;
    r.evidence.remote_content = Change::Unknown;
    r.evidence.content_comparison = Comparison::Different;
    let a = outcome(r);
    assert!(a.decisions.is_empty());
    assert_eq!(a.divergences[0].reason, DivergenceReason::MissingBaseline);
    assert_eq!(
        a.requirements,
        vec![Requirement::VerifyIdentity, Requirement::VerifyContent]
    );
}
#[test]
fn unknown_latest_metadata_does_not_fall_back_to_retained_known_values() {
    for value in [
        MetadataValue::NotCollected,
        MetadataValue::Unsupported,
        MetadataValue::Unavailable,
        MetadataValue::PermissionDenied,
    ] {
        let mut r = request();
        let mut latest = r.item.remote.clone().unwrap();
        latest.name = value;
        latest.parent_ids = MetadataValue::Unavailable;
        r.item.remote_latest = Some(latest);
        r.evidence.parent_comparison = Comparison::Unknown;
        let a = outcome(r);
        assert!(a.decisions.is_empty());
        assert_eq!(
            a.requirements,
            vec![
                Requirement::Compare(Dimension::Name),
                Requirement::Compare(Dimension::Location)
            ]
        );
    }
}
#[test]
fn qualified_identities_and_structural_consistency_are_validated() {
    let mut r = request();
    r.item.remote.as_mut().unwrap().reference.account_id = AccountId("other".into());
    assert_eq!(reconcile(r), Err(ReconciliationError::InvalidItemIdentity));
    let mut r = request();
    r.item.local.as_mut().unwrap().local_id = Some(LocalFileId("different".into()));
    assert_eq!(reconcile(r), Err(ReconciliationError::InvalidItemIdentity));
    let mut r = request();
    r.item.baseline.as_mut().unwrap().remote.parent_identity =
        Some(BaselineParent::Remote(RemoteFileRef {
            account_id: AccountId("other".into()),
            file_id: RemoteFileId("parent".into()),
        }));
    assert_eq!(reconcile(r), Err(ReconciliationError::InvalidItemIdentity));
    let mut r = request();
    r.item.id.0.clear();
    assert_eq!(reconcile(r), Err(ReconciliationError::InvalidRequest));
    let mut r = request();
    r.item.revision = 0;
    assert_eq!(reconcile(r), Err(ReconciliationError::InvalidRequest));
    let mut r = request();
    r.item.remote.as_mut().unwrap().kind = MetadataValue::Known(RemoteEntryKind::Directory);
    assert_eq!(reconcile(r), Err(ReconciliationError::InconsistentInput));
}
#[test]
fn content_proofs_for_absent_sides_or_missing_baseline_are_rejected() {
    let mut r = request();
    r.item.remote_presence = PresenceState::AbsentConfirmed;
    assert_eq!(reconcile(r), Err(ReconciliationError::InvalidEvidence));
    let mut r = request();
    r.item.baseline = None;
    assert_eq!(reconcile(r), Err(ReconciliationError::InvalidEvidence));
}
#[test]
fn unsupported_entry_types_and_incompatible_baselines_never_produce_commands() {
    let mut r = request();
    r.item.kind = ItemKind::Other;
    r.evidence.content_comparison = Comparison::Unknown;
    assert_eq!(outcome(r).requirements, vec![Requirement::SupportedKind]);
    let mut r = request();
    r.item.baseline.as_mut().unwrap().local.presence = PresenceState::Unknown;
    let a = outcome(r);
    assert!(a.decisions.is_empty());
    assert_eq!(a.requirements, vec![Requirement::VerifyBaseline]);
}

#[test]
fn unknown_kind_or_trash_state_cannot_authorize_propagation() {
    for trashed in [MetadataValue::Known(true), MetadataValue::NotCollected] {
        let mut r = request();
        r.item.remote.as_mut().unwrap().trashed = trashed;
        assert_eq!(
            outcome(r).requirements,
            vec![Requirement::Observe(Side::Remote)]
        );
    }
    let mut r = request();
    r.item.local.as_mut().unwrap().kind = MetadataValue::Unavailable;
    r.item.local.as_mut().unwrap().symlink.target = MetadataValue::Unavailable;
    assert_eq!(
        outcome(r).requirements,
        vec![Requirement::Observe(Side::Local)]
    );
}
#[test]
fn contradictory_content_proofs_and_provider_changes_are_rejected() {
    let mut r = request();
    r.evidence.content_comparison = Comparison::Different;
    assert_eq!(reconcile(r), Err(ReconciliationError::InvalidEvidence));
    let mut r = request();
    let mut latest = r.item.remote.clone().unwrap();
    latest.provider = ProviderId("other".into());
    r.item.remote_latest = Some(latest);
    assert_eq!(reconcile(r), Err(ReconciliationError::InvalidItemIdentity));
}
#[test]
fn both_absent_and_directory_content_have_no_transfer_intent() {
    let mut r = request();
    r.item.local_presence = PresenceState::AbsentConfirmed;
    r.item.remote_presence = PresenceState::AbsentConfirmed;
    r.evidence.content_comparison = Comparison::Unknown;
    let a = outcome(r);
    assert!(a.decisions.is_empty());
    assert!(a.requirements.is_empty());
    assert!(a.divergences.is_empty());
    let mut r = request();
    r.item.kind = ItemKind::Directory;
    r.item.local.as_mut().unwrap().kind = MetadataValue::Known(LocalEntryKind::Directory);
    r.item.remote.as_mut().unwrap().kind = MetadataValue::Known(RemoteEntryKind::Directory);
    r.evidence.content_comparison = Comparison::Unknown;
    r.evidence.local_content = Change::Unknown;
    r.evidence.remote_content = Change::Unknown;
    assert!(outcome(r).requirements.is_empty());
}
#[test]
fn absence_in_baseline_allows_new_creation_but_unknown_baseline_does_not() {
    let mut r = request();
    r.item.remote_presence = PresenceState::AbsentConfirmed;
    r.evidence.content_comparison = Comparison::Unknown;
    let base = r.item.baseline.as_mut().unwrap();
    base.local.presence = PresenceState::AbsentConfirmed;
    base.remote.presence = PresenceState::AbsentConfirmed;
    assert_eq!(
        outcome(r.clone()).decisions,
        vec![Decision::Create {
            source: Side::Local,
            kind: ItemKind::File
        }]
    );
    r.item.baseline.as_mut().unwrap().remote.presence = PresenceState::Unknown;
    let a = outcome(r);
    assert!(a.decisions.is_empty());
    assert_eq!(a.requirements, vec![Requirement::VerifyBaseline]);
}
#[test]
fn missing_parent_mapping_and_multiple_remote_parents_require_evidence() {
    let mut r = request();
    r.evidence.local_parent = None;
    r.evidence.parent_comparison = Comparison::Unknown;
    r.item.remote.as_mut().unwrap().parent_ids =
        MetadataValue::Known(vec![RemoteFileId("one".into()), RemoteFileId("two".into())]);
    let a = outcome(r);
    assert!(a.decisions.is_empty());
    assert_eq!(
        a.requirements,
        vec![Requirement::Compare(Dimension::Location)]
    );
}
#[test]
fn optional_local_id_is_not_fabricated_or_required_for_pure_comparison() {
    let mut r = request();
    r.item.local.as_mut().unwrap().local_id = None;
    assert!(outcome(r).requirements.is_empty());
}

#[test]
fn unbound_initial_observations_identify_direction_without_fabricating_absence() {
    for source in [Side::Local, Side::Remote] {
        let mut r = request();
        r.item.baseline = None;
        r.evidence.local_content = Change::Unknown;
        r.evidence.remote_content = Change::Unknown;
        r.evidence.content_comparison = Comparison::Unknown;
        let opposite = match source {
            Side::Local => {
                r.item.remote_presence = PresenceState::Unknown;
                r.item.remote_ref = None;
                r.item.remote = None;
                Side::Remote
            }
            Side::Remote => {
                r.item.local_presence = PresenceState::Unknown;
                r.item.local_file_id = None;
                r.item.local = None;
                Side::Local
            }
        };
        let original = r.clone();
        let a = outcome(r.clone());
        assert_eq!(
            a.decisions,
            vec![Decision::Create {
                source,
                kind: ItemKind::File
            }]
        );
        assert_eq!(
            a.requirements,
            vec![
                Requirement::Observe(opposite),
                Requirement::VerifyIdentity,
                Requirement::ResolveDestination
            ]
        );
        assert_eq!(r, original);
        assert_eq!(a, outcome(r.clone()));
        match source {
            Side::Local => r.item.remote_ref = request().item.remote_ref,
            Side::Remote => r.item.local_file_id = request().item.local_file_id,
        }
        assert!(outcome(r).decisions.is_empty());
        let mut partial = original;
        match source {
            Side::Local => partial.evidence.local_observation = ObservationConfidence::Partial,
            Side::Remote => partial.evidence.remote_observation = ObservationConfidence::Partial,
        }
        assert!(outcome(partial).decisions.is_empty());
    }
}

#[test]
fn moves_between_root_and_nested_parent_are_known_changes() {
    let mut r = request();
    r.item.baseline.as_mut().unwrap().local.parent_identity = None;
    r.evidence.parent_comparison = Comparison::Different;
    assert_eq!(
        outcome(r).decisions,
        vec![propagate(Dimension::Location, Side::Local)]
    );

    let mut r = request();
    r.item.local.as_mut().unwrap().relative_path =
        LocalRelativePath::new(b"file".to_vec()).unwrap();
    r.evidence.local_parent = None;
    r.evidence.parent_comparison = Comparison::Different;
    assert_eq!(
        outcome(r).decisions,
        vec![propagate(Dimension::Location, Side::Local)]
    );
}
