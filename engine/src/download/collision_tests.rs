//! I10-I14: real inventory decoding, durable planning and download collision safety.
use super::baseline_tests::{cycle, prepare};
use super::*;
use crate::{ConfirmedBase, ConflictReason};
use serde_json::json;

fn binary(id: &str, parent: &str, name: &str, bytes: &[u8]) -> InventoryEntry {
    let mut item = inventory().entries.remove(0);
    item.id = id.into();
    item.parent_id = parent.into();
    item.name = name.into();
    item.size = Some(bytes.len() as u64);
    item.md5_checksum = Some(format!("{:x}", md5::Md5::digest(bytes)));
    item
}
fn folder(id: &str, name: &str) -> InventoryEntry {
    let mut item = binary(id, "root", name, b"");
    item.kind = InventoryKind::Folder;
    item.mime_type = "application/vnd.google-apps.folder".into();
    item
}
/// Drives the production scanner through parent-specific wire pages. A one-item
/// page deliberately separates homonyms; reversed order must preserve decisions.
fn scanned(entries: &[InventoryEntry], reverse: bool, paginated: bool) -> RemoteInventory {
    let mut ordered = entries.to_vec();
    if reverse {
        ordered.reverse();
    }
    let mut calls = Vec::new();
    let inv = crate::remote::inventory_scan(&AccountId("google:test".into()), "root", &Cancellation::new(), |parent, token| {
        calls.push((parent.to_owned(), token.map(str::to_owned)));
        let siblings: Vec<_> = ordered.iter().filter(|e| e.parent_id == parent).collect();
        let offset = token.map(|t| t.parse::<usize>().unwrap()).unwrap_or(0);
        let end = if paginated { (offset + 1).min(siblings.len()) } else { siblings.len() };
        let files: Vec<_> = siblings[offset..end].iter().map(|e| json!({
            "id":e.id,"parents":[e.parent_id],"name":e.name,"mimeType":e.mime_type,
            "trashed":false,"version":e.version.map(|v| v.to_string()),
            "size":e.size.map(|v| v.to_string()),"md5Checksum":e.md5_checksum,
            "capabilities":{"canDownload":true}
        })).collect();
        Ok(serde_json::to_vec(&json!({"files":files,"nextPageToken":(end < siblings.len()).then(|| end.to_string())})).unwrap())
    }).unwrap();
    assert_eq!(inv.entries.len(), entries.len());
    if paginated && entries.iter().filter(|e| e.parent_id == "root").count() > 1 {
        assert!(calls
            .iter()
            .any(|(parent, token)| parent == "root" && token.is_some()));
    }
    inv
}
fn bases(f: &Fixture, inv: &RemoteInventory) -> HashMap<String, ConfirmedBase> {
    Store::open(&f.db())
        .unwrap()
        .confirmed_bases("client", &inv.account_id, &inv.root_id, &f.root)
        .unwrap()
}
/// Reopens both plan and bases and checks all blocked IDs, not just file bytes.
fn blocked_cycle(f: &Fixture, inv: &RemoteInventory, source: &mut Fake) {
    let before = bases(f, inv);
    let remote = source.content.clone();
    let metadata = source.entries.clone();
    for _ in 0..2 {
        let plan = prepare(f, inv).unwrap();
        let reopened = Store::open(&f.db())
            .unwrap()
            .load("client", &inv.account_id, &inv.root_id, &f.root)
            .unwrap()
            .unwrap();
        assert_eq!(reopened.entries.len(), inv.entries.len());
        for entry in &plan.entries {
            assert_eq!(
                entry.action,
                PlanAction::Blocked(ConflictReason::UnsafeOrAmbiguousPath)
            );
            assert!(entry.relative_path.is_none());
            assert_eq!(
                reopened
                    .entries
                    .iter()
                    .find(|e| e.remote_id == entry.remote_id),
                Some(entry)
            );
        }
        let report = cycle(f, inv, source).unwrap();
        assert_eq!(report.issues.len(), inv.entries.len());
        assert!(report
            .issues
            .iter()
            .all(|i| i.error == EngineError::DownloadConflict));
        assert_eq!(
            (report.downloaded, report.updated, report.recovered),
            (0, 0, 0)
        );
        assert!(report.preserved_copies.is_empty());
        assert!(source.media_ids.is_empty());
        assert!(source.metadata_ids.is_empty());
        assert_eq!(source.content, remote);
        assert_eq!(source.entries, metadata);
        assert_eq!(bases(f, inv), before);
    }
}

#[test]
fn i10_i11_i13_homonyms_block_before_and_after_installation_across_pages() {
    for installed in [false, true] {
        for same_bytes in [false, true] {
            for reverse in [false, true] {
                for paginated in [false, true] {
                    let f = Fixture::new();
                    let a = binary("a", "root", "dados.db", b"BASE-V1");
                    let b_bytes = if same_bytes {
                        &b"BASE-V1"[..]
                    } else {
                        &b"OUTRO-OBJETO"[..]
                    };
                    let b = binary("b", "root", "dados.db", b_bytes);
                    if installed {
                        let initial = scanned(std::slice::from_ref(&a), false, false);
                        let mut source = Fake::new(&initial);
                        source.content.insert("a".into(), b"BASE-V1".to_vec());
                        assert_eq!(cycle(&f, &initial, &mut source).unwrap().downloaded, 1);
                    }
                    let inv = scanned(&[a, b], reverse, paginated);
                    let mut source = Fake::new(&inv);
                    source.content.insert("a".into(), b"BASE-V1".to_vec());
                    source.content.insert("b".into(), b_bytes.to_vec());
                    blocked_cycle(&f, &inv, &mut source);
                    if installed {
                        assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), b"BASE-V1");
                        assert_eq!(bases(&f, &inv).len(), 1);
                        assert!(bases(&f, &inv).contains_key("a"));
                        assert_eq!(
                            filesystem::fingerprint(
                                &f.root,
                                std::path::Path::new("dados.db"),
                                &Cancellation::new()
                            )
                            .unwrap(),
                            Some(bases(&f, &inv)["a"].local_fingerprint.clone())
                        );
                    } else {
                        assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
                        assert!(bases(&f, &inv).is_empty());
                    }
                }
            }
        }
    }
}

#[test]
fn i12_i13_folder_homonyms_block_distinct_descendants_in_every_order() {
    for reverse in [false, true] {
        for paginated in [false, true] {
            let f = Fixture::new();
            let inv = scanned(
                &[
                    folder("fa", "same"),
                    folder("fb", "same"),
                    binary("a", "fa", "a.db", b"BASE-V1"),
                    binary("b", "fb", "b.db", b"OUTRO-OBJETO"),
                ],
                reverse,
                paginated,
            );
            let mut source = Fake::new(&inv);
            source.content.insert("a".into(), b"BASE-V1".to_vec());
            source.content.insert("b".into(), b"OUTRO-OBJETO".to_vec());
            blocked_cycle(&f, &inv, &mut source);
            assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
        }
    }
}

#[test]
fn i13_same_names_under_distinct_paths_install_the_correct_objects() {
    for reverse in [false, true] {
        for paginated in [false, true] {
            let f = Fixture::new();
            let inv = scanned(
                &[
                    folder("fa", "left"),
                    folder("fb", "right"),
                    binary("a", "fa", "dados.db", b"BASE-V1"),
                    binary("b", "fb", "dados.db", b"OUTRO-OBJETO"),
                ],
                reverse,
                paginated,
            );
            let mut source = Fake::new(&inv);
            source.content.insert("a".into(), b"BASE-V1".to_vec());
            source.content.insert("b".into(), b"OUTRO-OBJETO".to_vec());
            for iteration in 0..2 {
                let report = cycle(&f, &inv, &mut source).unwrap();
                assert!(report.issues.is_empty());
                assert_eq!(report.downloaded, if iteration == 0 { 2 } else { 0 });
                assert_eq!(fs::read(f.root.join("left/dados.db")).unwrap(), b"BASE-V1");
                assert_eq!(
                    fs::read(f.root.join("right/dados.db")).unwrap(),
                    b"OUTRO-OBJETO"
                );
                let mut media = source.media_ids.clone();
                media.sort();
                assert_eq!(media, ["a", "b"]);
                assert_eq!(bases(&f, &inv).len(), 2);
            }
        }
    }
}

#[test]
fn i14_removed_or_renamed_homonym_revalidates_the_remaining_object() {
    for remaining_a in [false, true] {
        for rename in [false, true] {
            let f = Fixture::new();
            let a = binary("a", "root", "dados.db", b"BASE-V1");
            let b = binary("b", "root", "dados.db", b"OUTRO-OBJETO");
            let initial = scanned(std::slice::from_ref(&a), false, false);
            let mut source = Fake::new(&initial);
            source.content.insert("a".into(), b"BASE-V1".to_vec());
            cycle(&f, &initial, &mut source).unwrap();
            let old = bases(&f, &initial)["a"].clone();
            let collision = scanned(&[a.clone(), b.clone()], true, true);
            let mut source = Fake::new(&collision);
            source.content.insert("a".into(), b"BASE-V1".to_vec());
            source.content.insert("b".into(), b"OUTRO-OBJETO".to_vec());
            blocked_cycle(&f, &collision, &mut source);
            let (remaining, mut departing) = if remaining_a { (a, b) } else { (b, a) };
            let mut entries = vec![remaining];
            if rename {
                departing.name = "other.db".into();
                entries.push(departing.clone());
            }
            let current = scanned(&entries, false, true);
            source.entries = current
                .entries
                .iter()
                .map(|e| (e.id.clone(), e.clone()))
                .collect();
            for _ in 0..2 {
                let plan = prepare(&f, &current).unwrap();
                let remaining = plan
                    .entries
                    .iter()
                    .find(|e| e.relative_path.as_deref() == Some(std::path::Path::new("dados.db")))
                    .unwrap();
                assert_eq!(
                    remaining.action,
                    if remaining_a {
                        PlanAction::NoOp
                    } else {
                        PlanAction::Blocked(ConflictReason::ExistingLocalData)
                    }
                );
                let report = cycle(&f, &current, &mut source).unwrap();
                assert_eq!(report.issues.len(), usize::from(!remaining_a));
                assert_eq!(fs::read(f.root.join("dados.db")).unwrap(), b"BASE-V1");
                assert_eq!(
                    source.media_ids,
                    if rename {
                        vec![departing.id.clone()]
                    } else {
                        vec![]
                    }
                );
                if remaining_a {
                    assert_eq!(bases(&f, &current)["a"], old);
                } else {
                    assert!(!bases(&f, &current).contains_key("b"));
                }
                if rename {
                    assert_eq!(
                        fs::read(f.root.join("other.db")).unwrap(),
                        if remaining_a {
                            &b"OUTRO-OBJETO"[..]
                        } else {
                            &b"BASE-V1"[..]
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn sync009_unicode_case_type_and_generated_destinations_survive_restart() {
    for (left, right) in [
        ("Report.pdf", "report.PDF"),
        ("Straße", "STRASSE"),
        ("café", "cafe\u{301}"),
        ("Σ", "ς"),
    ] {
        for reverse in [false, true] {
            let f = Fixture::new();
            let inv = scanned(
                &[
                    folder("dir", left),
                    binary("other", "root", right, b"OTHER"),
                    binary("child", "dir", "child", b"CHILD"),
                ],
                reverse,
                true,
            );
            blocked_cycle(&f, &inv, &mut Fake::new(&inv));
            assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
        }
    }
    let f = Fixture::new();
    let mut link = binary("link", "root", "report", b"");
    link.kind = InventoryKind::GoogleWorkspaceLink;
    let inv = RemoteInventory {
        entries: vec![link, binary("binary", "root", "REPORT.desktop", b"KEEP")],
        ..inventory()
    };
    blocked_cycle(&f, &inv, &mut Fake::new(&inv));
}

#[test]
fn sync009_invalid_destinations_are_durable_and_do_not_read_remote() {
    for name in [
        "CON.txt".to_owned(),
        "trailing.".into(),
        "bad:name".into(),
        "é".repeat(128),
    ] {
        let f = Fixture::new();
        let inv = RemoteInventory {
            entries: vec![binary("invalid", "root", &name, b"KEEP")],
            ..inventory()
        };
        blocked_cycle(&f, &inv, &mut Fake::new(&inv));
        assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
    }
}

#[test]
fn sync009_file_folder_mismatches_preserve_real_local_data() {
    for remote_folder in [false, true] {
        let f = Fixture::new();
        let item = if remote_folder {
            fs::write(f.root.join("target"), b"KEEP").unwrap();
            folder("item", "target")
        } else {
            fs::create_dir(f.root.join("target")).unwrap();
            fs::write(f.root.join("target/child"), b"KEEP").unwrap();
            binary("item", "root", "target", b"REMOTE")
        };
        let inv = scanned(&[item], false, false);
        let mut source = Fake::new(&inv);
        for _ in 0..2 {
            let plan = prepare(&f, &inv).unwrap();
            assert_eq!(
                plan.entries[0].action,
                PlanAction::Blocked(ConflictReason::LocalTypeMismatch)
            );
            assert_eq!(cycle(&f, &inv, &mut source).unwrap().issues.len(), 1);
            let path = if remote_folder {
                "target"
            } else {
                "target/child"
            };
            assert_eq!(fs::read(f.root.join(path)).unwrap(), b"KEEP");
            assert!(bases(&f, &inv).is_empty());
            assert!(source.media_ids.is_empty());
        }
    }
}

#[test]
fn sync009_concurrent_destination_type_change_preserves_data_and_replans() {
    for directory in [false, true] {
        let f = Fixture::new();
        let inv = scanned(&[binary("item", "root", "target", b"REMOTE")], false, false);
        let plan = prepare(&f, &inv).unwrap();
        assert_eq!(plan.entries[0].action, PlanAction::Download);
        if directory {
            fs::create_dir(f.root.join("target")).unwrap();
            fs::write(f.root.join("target/child"), b"KEEP").unwrap();
        } else {
            fs::write(f.root.join("target"), b"KEEP").unwrap();
        }
        let mut source = Fake::new(&inv);
        let report = super::concurrency_tests::execute_plan(
            &f,
            &inv,
            &plan,
            &mut source,
            &Cancellation::new(),
            |_| {},
        )
        .unwrap();
        assert_eq!(report.downloaded, 0);
        assert_eq!(report.issues.len(), 1);
        assert!(bases(&f, &inv).is_empty());
        assert!(source.media_ids.is_empty());
        let path = if directory { "target/child" } else { "target" };
        assert_eq!(fs::read(f.root.join(path)).unwrap(), b"KEEP");
        assert!(matches!(
            prepare(&f, &inv).unwrap().entries[0].action,
            PlanAction::Blocked(_)
        ));
    }
}
