//! Deterministic provider simulations; no credentials, network or user data.
use super::*;
use crate::{
    remote::{Change, ChangePage},
    AccountId, AuthError, Cancellation, EngineError, InventoryEntry, InventoryKind,
    RemoteInventory,
};
use std::collections::VecDeque;
fn item(id: &str, parent: &str, folder: bool) -> InventoryEntry {
    InventoryEntry {
        id: id.into(),
        parent_id: parent.into(),
        name: id.into(),
        mime_type: if folder {
            "application/vnd.google-apps.folder"
        } else {
            "text/plain"
        }
        .into(),
        version: Some(1),
        size: Some(3),
        md5_checksum: Some("900150983cd24fb0d6963f7d28e17f72".into()),
        kind: if folder {
            InventoryKind::Folder
        } else {
            InventoryKind::Binary
        },
    }
}
fn inventory(entries: Vec<InventoryEntry>) -> RemoteInventory {
    RemoteInventory {
        account_id: AccountId("account".into()),
        root_id: "root".into(),
        entries,
    }
}
fn checkpoint(entries: Vec<InventoryEntry>) -> Checkpoint {
    Checkpoint {
        cursor: "old".into(),
        inventory: inventory(entries),
        notices: Vec::new(),
    }
}
fn terminal(events: Vec<Change>) -> Result<ChangePage, EngineError> {
    Ok(ChangePage {
        changes: events,
        next: None,
        terminal: Some("new".into()),
    })
}
fn event(item: InventoryEntry) -> Change {
    Change {
        id: item.id.clone(),
        removed: false,
        item: Some(item),
    }
}
struct Fake {
    pages: VecDeque<Result<ChangePage, EngineError>>,
    scans: VecDeque<RemoteInventory>,
    calls: Vec<String>,
    cancel_on_page: Option<Cancellation>,
}
impl Fake {
    fn new(pages: Vec<Result<ChangePage, EngineError>>) -> Self {
        Self {
            pages: pages.into(),
            scans: VecDeque::new(),
            calls: Vec::new(),
            cancel_on_page: None,
        }
    }
}
impl Provider for Fake {
    fn start(&mut self) -> Result<String, EngineError> {
        self.calls.push("start".into());
        Ok("start".into())
    }
    fn scan(&mut self) -> Result<RemoteInventory, EngineError> {
        self.calls.push("scan".into());
        Ok(self.scans.pop_front().expect("unexpected full traversal"))
    }
    fn page(&mut self, token: &str) -> Result<ChangePage, EngineError> {
        self.calls.push(format!("page:{token}"));
        if let Some(cancel) = &self.cancel_on_page {
            cancel.cancel();
        }
        self.pages.pop_front().expect("unexpected page")
    }
}
fn collect_with(previous: Option<&Checkpoint>, fake: &mut Fake) -> Result<Checkpoint, EngineError> {
    collect(
        &AccountId("account".into()),
        "root",
        previous,
        &Cancellation::new(),
        fake,
    )
}
#[test]
fn bootstrap_captures_boundary_before_scan_and_replays_window() {
    let mut changed = item("file", "root", false);
    changed.version = Some(2);
    let mut fake = Fake::new(vec![terminal(vec![event(changed.clone())])]);
    fake.scans
        .push_back(inventory(vec![item("file", "root", false)]));
    let result = collect_with(None, &mut fake).unwrap();
    assert_eq!(fake.calls, ["start", "scan", "page:start"]);
    assert_eq!(result.inventory.entries, [changed]);
    assert_eq!(result.cursor, "new");
}
#[test]
fn unchanged_cycle_uses_feed_only_and_last_event_wins_across_empty_pages() {
    let previous = checkpoint(vec![item("file", "root", false)]);
    let mut fake = Fake::new(vec![terminal(vec![])]);
    assert_eq!(
        collect_with(Some(&previous), &mut fake)
            .unwrap()
            .inventory
            .entries,
        previous.inventory.entries
    );
    assert_eq!(fake.calls, ["page:old"]);
    let mut changed = item("file", "root", false);
    changed.version = Some(3);
    let mut fake = Fake::new(vec![
        Ok(ChangePage {
            changes: vec![],
            next: Some("p2".into()),
            terminal: None,
        }),
        Ok(ChangePage {
            changes: vec![event(item("file", "root", false))],
            next: Some("p3".into()),
            terminal: None,
        }),
        terminal(vec![event(changed.clone())]),
    ]);
    assert_eq!(
        collect_with(Some(&previous), &mut fake)
            .unwrap()
            .inventory
            .entries,
        [changed]
    );
    assert_eq!(fake.calls, ["page:old", "page:p2", "page:p3"]);
}
#[test]
fn additions_unrelated_changes_and_subtree_unavailability() {
    let previous = checkpoint(vec![
        item("folder", "root", true),
        item("child", "folder", false),
    ]);
    let mut fake = Fake::new(vec![terminal(vec![
        event(item("new", "root", false)),
        event(item("foreign", "outside", false)),
        Change {
            id: "folder".into(),
            removed: true,
            item: None,
        },
    ])]);
    let result = collect_with(Some(&previous), &mut fake).unwrap();
    assert_eq!(result.inventory.entries, [item("new", "root", false)]);
    assert_eq!(result.notices.len(), 2);
    assert!(result
        .notices
        .iter()
        .any(|n| n.path.as_deref() == Some(std::path::Path::new("folder/child"))));
    let mut fake = Fake::new(vec![
        terminal(vec![event(item("folder", "root", true))]),
        terminal(vec![]),
    ]);
    let mut restored = previous.inventory.clone();
    restored.entries.push(item("new", "root", false));
    fake.scans.push_back(restored);
    let returned = collect_with(Some(&result), &mut fake).unwrap();
    assert!(returned.notices.is_empty());
}
#[test]
fn topology_changes_rescan_descendants_without_leaf_events() {
    for folder in [item("folder", "root", true), {
        let mut f = item("folder", "root", true);
        f.name = "renamed".into();
        f
    }] {
        let previous = checkpoint(vec![]);
        let mut fake = Fake::new(vec![
            terminal(vec![event(folder.clone())]),
            terminal(vec![]),
        ]);
        fake.scans.push_back(inventory(vec![
            folder.clone(),
            item("child", "folder", false),
        ]));
        let result = collect_with(Some(&previous), &mut fake).unwrap();
        assert_eq!(result.inventory.entries.len(), 2);
        assert_eq!(fake.calls, ["page:old", "start", "scan", "page:start"]);
    }
}
#[test]
fn rejected_cursor_bootstraps_but_transient_errors_do_not() {
    let previous = checkpoint(vec![item("old", "root", false)]);
    let mut fake = Fake::new(vec![
        Err(EngineError::ChangeCursorRejected),
        terminal(vec![]),
    ]);
    fake.scans
        .push_back(inventory(vec![item("new", "root", false)]));
    let result = collect_with(Some(&previous), &mut fake).unwrap();
    assert_eq!(result.notices.len(), 1);
    assert_eq!(fake.calls, ["page:old", "start", "scan", "page:start"]);
    for error in [
        EngineError::RemoteForbidden,
        EngineError::RemoteRateLimited,
        EngineError::Authentication(AuthError::Network),
    ] {
        let mut fake = Fake::new(vec![Err(error.clone())]);
        assert!(matches!(collect_with(Some(&previous),&mut fake),Err(e) if e==error));
        assert_eq!(fake.calls, ["page:old"]);
        assert_eq!(previous.cursor, "old");
    }
}
#[test]
fn repeated_tokens_partial_failure_cancellation_and_wrong_scope_fail_closed() {
    let previous = checkpoint(vec![]);
    let mut fake = Fake::new(vec![Ok(ChangePage {
        changes: vec![event(item("f", "root", false))],
        next: Some("old".into()),
        terminal: None,
    })]);
    assert!(collect_with(Some(&previous), &mut fake).is_err());
    let mut fake = Fake::new(vec![
        Ok(ChangePage {
            changes: vec![event(item("f", "root", false))],
            next: Some("next".into()),
            terminal: None,
        }),
        Err(EngineError::RemoteRateLimited),
    ]);
    assert!(collect_with(Some(&previous), &mut fake).is_err());
    assert!(previous.inventory.entries.is_empty());
    let cancel = Cancellation::new();
    cancel.cancel();
    let mut fake = Fake::new(vec![]);
    assert!(collect(
        &previous.inventory.account_id,
        "root",
        Some(&previous),
        &cancel,
        &mut fake
    )
    .is_err());
    assert!(fake.calls.is_empty());
    assert!(collect(
        &AccountId("other".into()),
        "root",
        Some(&previous),
        &Cancellation::new(),
        &mut fake
    )
    .is_err());
}

#[test]
fn cancellation_after_a_page_and_unstable_topology_never_publish_results() {
    let old = checkpoint(vec![]);
    let cancel = Cancellation::new();
    let mut fake = Fake::new(vec![terminal(vec![event(item("f", "root", false))])]);
    fake.cancel_on_page = Some(cancel.clone());
    assert!(matches!(
        collect(
            &old.inventory.account_id,
            "root",
            Some(&old),
            &cancel,
            &mut fake
        ),
        Err(EngineError::Authentication(AuthError::Cancelled))
    ));
    let mut fake = Fake::new(
        (0..3)
            .map(|n| terminal(vec![event(item(&format!("folder{n}"), "root", true))]))
            .collect(),
    );
    for _ in 0..3 {
        fake.scans.push_back(inventory(vec![]));
    }
    assert!(matches!(
        collect_with(Some(&old), &mut fake),
        Err(EngineError::RemoteChanged)
    ));
    assert_eq!(old.cursor, "old");
}
#[test]
fn folder_moves_outside_root_reconcile_and_file_renames_preserve_old_paths() {
    let old = checkpoint(vec![
        item("folder", "root", true),
        item("child", "folder", false),
    ]);
    let mut moved = item("folder", "outside", true);
    moved.version = Some(2);
    let mut fake = Fake::new(vec![terminal(vec![event(moved)]), terminal(vec![])]);
    fake.scans.push_back(inventory(vec![]));
    let result = collect_with(Some(&old), &mut fake).unwrap();
    assert!(result.inventory.entries.is_empty());
    assert_eq!(result.notices.len(), 2);
    let old = checkpoint(vec![item("file", "root", false)]);
    let mut renamed = item("file", "root", false);
    renamed.name = "newname".into();
    renamed.version = Some(2);
    let mut fake = Fake::new(vec![terminal(vec![event(renamed.clone())])]);
    let result = collect_with(Some(&old), &mut fake).unwrap();
    assert_eq!(result.inventory.entries, [renamed]);
    assert_eq!(
        result.notices[0].path.as_deref(),
        Some(std::path::Path::new("file"))
    );
    assert_eq!(fake.calls, ["page:old"]);
}
#[test]
fn root_metadata_without_parent_is_not_a_removed_root() {
    let old = checkpoint(vec![]);
    let mut fake = Fake::new(vec![terminal(vec![Change {
        id: "root".into(),
        removed: false,
        item: None,
    }])]);
    assert!(collect_with(Some(&old), &mut fake).is_ok());
    let mut fake = Fake::new(vec![terminal(vec![Change {
        id: "root".into(),
        removed: true,
        item: None,
    }])]);
    assert!(matches!(
        collect_with(Some(&old), &mut fake),
        Err(EngineError::RemoteFolderUnavailable)
    ));
}

#[test]
fn forced_reconciliation_refreshes_silent_revisions_and_preserves_notices() {
    let mut old = checkpoint(vec![item("file", "root", false)]);
    old.notices.push(Notice {
        id: "previously-moved".into(),
        path: Some("old/path.txt".into()),
    });
    let mut newer = item("file", "root", false);
    newer.version = Some(2);
    let mut provider = Fake::new(vec![terminal(vec![])]);
    provider.scans.push_back(inventory(vec![newer.clone()]));
    let result = reconcile(
        &old.inventory.account_id,
        "root",
        &old,
        &Cancellation::new(),
        &mut provider,
    )
    .unwrap();
    assert_eq!(result.inventory.entries, vec![newer]);
    assert_eq!(result.notices, old.notices);
    assert_eq!(provider.calls, vec!["start", "scan", "page:start"]);
    assert_eq!(result.cursor, "new");
    // The captured pre-scan boundary must still catch a concurrent newer revision.
    let mut newest = item("file", "root", false);
    newest.version = Some(3);
    let mut provider = Fake::new(vec![terminal(vec![event(newest.clone())])]);
    provider.scans.push_back(result.inventory.clone());
    assert_eq!(
        reconcile(
            &old.inventory.account_id,
            "root",
            &result,
            &Cancellation::new(),
            &mut provider
        )
        .unwrap()
        .inventory
        .entries,
        vec![newest]
    );
}

#[test]
fn failed_forced_refresh_does_not_change_prior_observations_or_notices() {
    let mut old = checkpoint(vec![item("file", "root", false)]);
    old.notices.push(Notice {
        id: "missing".into(),
        path: Some("old.txt".into()),
    });
    let before = serde_json::to_string(&old).unwrap();
    let mut provider = Fake::new(vec![Err(EngineError::Authentication(AuthError::Network))]);
    provider.scans.push_back(old.inventory.clone());
    assert!(matches!(
        reconcile(
            &old.inventory.account_id,
            "root",
            &old,
            &Cancellation::new(),
            &mut provider
        ),
        Err(EngineError::Authentication(AuthError::Network))
    ));
    assert_eq!(serde_json::to_string(&old).unwrap(), before);
    let cancel = Cancellation::new();
    cancel.cancel();
    let mut provider = Fake::new(vec![]);
    assert!(matches!(
        reconcile(
            &old.inventory.account_id,
            "root",
            &old,
            &cancel,
            &mut provider
        ),
        Err(EngineError::Authentication(AuthError::Cancelled))
    ));
    assert!(provider.calls.is_empty());
    assert_eq!(serde_json::to_string(&old).unwrap(), before);
}

#[test]
fn previous_paths_distinguish_moves_from_unavailability_without_creating_issues() {
    let previous = checkpoint(vec![item("file", "root", false)]);
    let mut renamed = item("file", "root", false);
    renamed.name = "renamed.txt".into();
    let mut fake = Fake::new(vec![terminal(vec![event(renamed)])]);
    let moved = collect_with(Some(&previous), &mut fake).unwrap();
    let activity = previous_paths(&moved).unwrap();
    assert_eq!(
        activity,
        [crate::PreviousPath {
            previous_path: Some("file".into()),
            current_path: Some("renamed.txt".into()),
        }]
    );
    let mut fake = Fake::new(vec![terminal(vec![])]);
    let unchanged = collect_with(Some(&moved), &mut fake).unwrap();
    assert_eq!(previous_paths(&unchanged).unwrap(), activity);
    let mut fake = Fake::new(vec![terminal(vec![Change {
        id: "file".into(),
        removed: true,
        item: None,
    }])]);
    let unavailable = collect_with(Some(&unchanged), &mut fake).unwrap();
    assert!(previous_paths(&unavailable)
        .unwrap()
        .iter()
        .all(|a| a.current_path.is_none()));
}
