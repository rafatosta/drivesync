use super::*;
use crate::InventoryEntry;
fn inventory() -> RemoteInventory {
    RemoteInventory {
        account_id: AccountId("test".into()),
        root_id: "root".into(),
        entries: vec![InventoryEntry {
            id: "a".into(),
            parent_id: "root".into(),
            name: "file.txt".into(),
            mime_type: "text/plain".into(),
            version: Some(1),
            size: Some(3),
            md5_checksum: Some("hash".into()),
            web_view_link: None,
            kind: InventoryKind::Binary,
        }],
    }
}
#[test]
fn three_way_requires_confirmed_local_evidence() {
    let base = ConfirmedBase {
        remote_version: 2,
        local_fingerprint: "hash".into(),
    };
    assert_eq!(
        decide(None, &LocalObservation::Absent, 3),
        PlanAction::Download
    );
    assert_eq!(
        decide(None, &LocalObservation::UnmanagedFile, 3),
        PlanAction::Blocked(ConflictReason::ExistingLocalData)
    );
    assert_eq!(
        decide(Some(&base), &LocalObservation::Verified("hash".into()), 2),
        PlanAction::NoOp
    );
    assert_eq!(
        decide(Some(&base), &LocalObservation::Verified("hash".into()), 3),
        PlanAction::Download
    );
    assert_eq!(
        decide(
            Some(&base),
            &LocalObservation::Verified("changed".into()),
            3
        ),
        PlanAction::Blocked(ConflictReason::ExistingLocalData)
    );
    assert_eq!(
        decide(Some(&base), &LocalObservation::Absent, 1),
        PlanAction::DeferredUnknown
    );
    assert_eq!(
        decide(None, &LocalObservation::Unknown, 3),
        PlanAction::DeferredUnknown
    );
}
#[test]
fn ambiguous_and_unsafe_names_never_reach_observer() {
    let mut inv = inventory();
    let mut duplicate = inv.entries[0].clone();
    duplicate.id = "b".into();
    inv.entries.push(duplicate);
    let result = plan(&inv, Path::new("/test"), |_| {
        panic!("ambiguous path observed")
    })
    .unwrap();
    assert!(result
        .entries
        .iter()
        .all(|e| e.action == PlanAction::Blocked(ConflictReason::UnsafeOrAmbiguousPath)));
    inv.entries.pop();
    inv.entries[0].name = "../outside".into();
    assert!(
        plan(&inv, Path::new("/test"), |_| panic!("unsafe path observed"))
            .unwrap()
            .entries[0]
            .relative_path
            .is_none()
    );
    inv.entries[0].name = ".drivesync".into();
    assert!(
        plan(&inv, Path::new("/test"), |_| panic!("reserved path observed"))
            .unwrap()
            .entries[0]
            .relative_path
            .is_none()
    );
}
#[test]
fn metadata_observer_preserves_files_and_rejects_symlinks() {
    let root = std::env::temp_dir().join(format!(
        "drivesync-plan-{}",
        oauth2::CsrfToken::new_random().secret()
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("file.txt"), b"keep").unwrap();
    std::os::unix::fs::symlink(&root, root.join("link")).unwrap();
    assert_eq!(
        observe(&root, Path::new("link/file.txt")),
        LocalObservation::SymlinkOrSpecial
    );
    let result = plan(&inventory(), &root, |p| observe(&root, p)).unwrap();
    assert_eq!(
        result.entries[0].action,
        PlanAction::Blocked(ConflictReason::ExistingLocalData)
    );
    assert_eq!(std::fs::read(root.join("file.txt")).unwrap(), b"keep");
    assert_eq!(
        observe(&root, Path::new("missing/child")),
        LocalObservation::Absent
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn folder_ambiguity_blocks_descendants_and_missing_versions_defer() {
    let mut inv = inventory();
    let mut folder = inv.entries[0].clone();
    folder.id = "folder".into();
    folder.name = "directory".into();
    folder.kind = InventoryKind::Folder;
    inv.entries[0].parent_id = "folder".into();
    inv.entries.push(folder.clone());
    let result = plan(&inv, Path::new("/local"), |_| LocalObservation::Absent).unwrap();
    assert_eq!(
        result.entries[0].relative_path,
        Some(PathBuf::from("directory/file.txt"))
    );
    assert_eq!(result.entries[1].action, PlanAction::CreateDirectory);
    folder.id = "duplicate".into();
    inv.entries.push(folder);
    let result = plan(&inv, Path::new("/local"), |_| {
        panic!("ambiguous descendant observed")
    })
    .unwrap();
    assert!(result.entries.iter().all(|e| e.relative_path.is_none()));
    let mut inv = inventory();
    inv.entries[0].version = None;
    assert_eq!(
        plan(&inv, Path::new("/local"), |_| LocalObservation::Absent)
            .unwrap()
            .entries[0]
            .action,
        PlanAction::DeferredUnknown
    );
}
