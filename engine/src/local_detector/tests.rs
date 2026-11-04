use super::*;

fn entry(object: LocalObject) -> LocalEntry {
    LocalEntry {
        metadata: LocalMetadata {
            device: 1,
            inode: 2,
            mode: 0,
            size: 0,
            modified_ns: 0,
            changed_ns: 0,
        },
        object,
    }
}

fn directory() -> LocalEntry {
    entry(LocalObject::Directory)
}

fn file(digest: u8) -> LocalEntry {
    entry(LocalObject::File {
        sha256: [digest; 32],
    })
}

fn inventory(entries: Vec<(&[u8], LocalEntry)>) -> LocalInventory {
    LocalInventory::new(
        entries
            .into_iter()
            .map(|(path, entry)| (path.to_vec(), entry)),
    )
    .unwrap()
}

#[test]
fn baseline_includes_root_and_completion_without_claiming_creation() {
    let current = inventory(vec![
        (b"", directory()),
        (b"folder", directory()),
        (b"folder/file", file(1)),
    ]);
    let events = reconcile_local_inventories(None, &current);
    assert_eq!(events.len(), 4);
    assert!(matches!(&events[0], LocalObservation::Initial { path, .. } if path.is_empty()));
    assert_eq!(events.last(), Some(&LocalObservation::InitialComplete));
    assert!(events[..3]
        .iter()
        .all(|event| matches!(event, LocalObservation::Initial { .. })));
    assert!(reconcile_local_inventories(Some(&current), &current).is_empty());
}

#[test]
fn changes_are_conservative_and_detect_equal_metadata_with_new_content() {
    let old = inventory(vec![
        (b"", directory()),
        (b"folder", directory()),
        (b"folder/old", file(1)),
        (b"unchanged", file(3)),
    ]);
    let current = inventory(vec![
        (b"", directory()),
        (b"folder", directory()),
        (b"folder/new", file(1)),
        (b"unchanged", file(4)),
    ]);
    let events = reconcile_local_inventories(Some(&old), &current);
    assert_eq!(events.len(), 3);
    assert!(matches!(
        &events[0],
        LocalObservation::Reevaluate { path, previous: None, current: Some(_) }
            if path == b"folder/new"
    ));
    assert!(matches!(
        &events[1],
        LocalObservation::Reevaluate { path, previous: Some(_), current: None }
            if path == b"folder/old"
    ));
    assert!(matches!(
        &events[2],
        LocalObservation::Reevaluate { path, previous: Some(_), current: Some(_) }
            if path == b"unchanged"
    ));
}

#[test]
fn non_utf8_paths_and_symlink_targets_are_preserved_without_traversal() {
    let old = inventory(vec![(b"", directory())]);
    let symlink = entry(LocalObject::Symlink {
        target: b"/outside/\xff".to_vec(),
    });
    let current = inventory(vec![(b"", directory()), (b"\xff", symlink.clone())]);
    assert_eq!(
        reconcile_local_inventories(Some(&old), &current),
        vec![LocalObservation::Reevaluate {
            path: b"\xff".to_vec(),
            previous: None,
            current: Some(symlink),
        }]
    );
}

#[test]
fn metadata_and_subtree_changes_are_observations_not_operation_claims() {
    let old = inventory(vec![
        (b"", directory()),
        (b"folder", directory()),
        (b"folder/nested", file(1)),
        (b"metadata", file(2)),
    ]);
    let mut changed = file(2);
    changed.metadata.mode = 0o600;
    let current = inventory(vec![(b"", directory()), (b"metadata", changed)]);
    let events = reconcile_local_inventories(Some(&old), &current);
    assert_eq!(events.len(), 3);
    for (event, expected) in
        events
            .iter()
            .zip([b"folder".as_slice(), b"folder/nested", b"metadata"])
    {
        assert!(matches!(event, LocalObservation::Reevaluate { path, .. } if path == expected));
    }
}

#[test]
fn invalid_or_duplicate_paths_and_non_directory_ancestors_are_rejected() {
    for path in [
        b"/absolute".as_slice(),
        b"double//slash",
        b"./file",
        b"folder/../file",
        b"trailing/",
        b"nul\0byte",
    ] {
        assert!(
            LocalInventory::new(vec![(Vec::new(), directory()), (path.to_vec(), file(1))]).is_err()
        );
    }
    assert!(LocalInventory::new(vec![]).is_err());
    assert!(LocalInventory::new(vec![(Vec::new(), file(1))]).is_err());
    assert!(
        LocalInventory::new(vec![(Vec::new(), directory()), (Vec::new(), directory())]).is_err()
    );
    assert!(LocalInventory::new(vec![
        (Vec::new(), directory()),
        (b"folder/file".to_vec(), file(1))
    ])
    .is_err());
    assert!(LocalInventory::new(vec![
        (Vec::new(), directory()),
        (
            b"folder".to_vec(),
            entry(LocalObject::Symlink {
                target: b"/outside".to_vec()
            })
        ),
        (b"folder/file".to_vec(), file(1))
    ])
    .is_err());
}
