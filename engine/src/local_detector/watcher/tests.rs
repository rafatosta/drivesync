use super::*;
use std::fs;
use std::io::Write;
use std::os::unix::{
    ffi::OsStringExt,
    fs::{symlink, PermissionsExt},
};
use std::time::Duration;

struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "drivesync-activity-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn watcher(&self) -> LocalWatcher {
        self.with_options(LocalWatchOptions::default())
    }
    fn with_options(&self, options: LocalWatchOptions) -> LocalWatcher {
        LocalWatcher::start(
            LocalScope::new("client", "account", &self.0).unwrap(),
            options,
        )
        .unwrap()
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn root_wd(w: &LocalWatcher) -> i32 {
    *w.watches
        .iter()
        .find(|(_, d)| d.path.as_os_str().is_empty())
        .unwrap()
        .0
}
fn collect(w: &mut LocalWatcher, predicate: impl Fn(&[LocalNotice]) -> bool) -> Vec<LocalNotice> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut all = Vec::new();
    loop {
        all.extend(w.poll().notices);
        if predicate(&all) {
            return all;
        }
        assert!(
            Instant::now() < deadline,
            "missing expected notice: {all:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn has(events: &[LocalNotice], path: &str, activity: &LocalActivity) -> bool {
    events
        .iter()
        .any(|e| e.path == Path::new(path) && &e.activity == activity)
}

#[test]
fn startup_is_not_creation_and_restart_requires_review() {
    let tree = Tree::new();
    fs::create_dir(tree.0.join("existing")).unwrap();
    fs::write(tree.0.join("existing/file"), b"keep").unwrap();
    for _ in 0..2 {
        let mut watcher = tree.watcher();
        let batch = watcher.poll();
        assert_eq!(batch.scope.root(), tree.0);
        assert_eq!(batch.state, LocalWatchState::Observing);
        assert!(has(
            &batch.notices,
            "",
            &LocalActivity::Recheck(LocalCoverageGap::Startup)
        ));
        assert!(!batch
            .notices
            .iter()
            .any(|n| n.activity == LocalActivity::Created));
    }
    assert_eq!(fs::read(tree.0.join("existing/file")).unwrap(), b"keep");
}

#[test]
fn create_write_close_metadata_remove_are_activity_hints() {
    let tree = Tree::new();
    let mut w = tree.watcher();
    w.poll();
    let path = tree.0.join("file");
    fs::write(&path, b"hello").unwrap();
    let events = collect(&mut w, |e| has(e, "file", &LocalActivity::ClosedAfterWrite));
    for activity in [
        LocalActivity::Created,
        LocalActivity::Written,
        LocalActivity::ClosedAfterWrite,
    ] {
        assert!(has(&events, "file", &activity));
    }
    assert!(events
        .iter()
        .filter(|e| e.path == Path::new("file"))
        .all(|e| e.is_directory.is_none()));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    collect(&mut w, |e| has(e, "file", &LocalActivity::MetadataChanged));
    fs::remove_file(&path).unwrap();
    collect(&mut w, |e| has(e, "file", &LocalActivity::Removed));
}

#[test]
fn atomic_save_reports_replacement_move_without_content_claims() {
    let tree = Tree::new();
    fs::write(tree.0.join("document"), b"old").unwrap();
    let mut w = tree.watcher();
    w.poll();
    fs::write(tree.0.join("temporary"), b"new").unwrap();
    fs::rename(tree.0.join("temporary"), tree.0.join("document")).unwrap();
    let activity = LocalActivity::Moved {
        from: "temporary".into(),
    };
    collect(&mut w, |e| has(e, "document", &activity));
    assert_eq!(fs::read(tree.0.join("document")).unwrap(), b"new");
}

#[test]
fn nested_directory_move_rebases_descendant_watches() {
    let tree = Tree::new();
    fs::create_dir_all(tree.0.join("b/child")).unwrap();
    fs::create_dir(tree.0.join("a")).unwrap();
    let mut w = tree.watcher();
    w.poll();
    fs::rename(tree.0.join("b"), tree.0.join("a/b")).unwrap();
    let events = collect(&mut w, |e| {
        has(e, "a/b", &LocalActivity::Moved { from: "b".into() })
    });
    assert!(events.iter().any(|e| e.is_directory == Some(true)));
    fs::write(tree.0.join("a/b/child/file"), b"x").unwrap();
    collect(&mut w, |e| {
        has(e, "a/b/child/file", &LocalActivity::Written)
    });
    assert!(w.watches.values().all(|d| !d.path.starts_with("b")));
}

#[test]
fn new_directories_and_received_subtrees_are_watched_and_require_review() {
    let tree = Tree::new();
    let outside = Tree::new();
    fs::create_dir_all(outside.0.join("received/child")).unwrap();
    fs::write(outside.0.join("received/child/old"), b"x").unwrap();
    let mut w = tree.watcher();
    w.poll();
    fs::create_dir(tree.0.join("new")).unwrap();
    collect(&mut w, |e| has(e, "new", &LocalActivity::Created));
    fs::write(tree.0.join("new/file"), b"x").unwrap();
    collect(&mut w, |e| has(e, "new/file", &LocalActivity::Written));
    fs::rename(outside.0.join("received"), tree.0.join("received")).unwrap();
    let events = collect(&mut w, |e| has(e, "received", &LocalActivity::MovedIn));
    assert!(has(
        &events,
        "received",
        &LocalActivity::Recheck(LocalCoverageGap::SubtreeAttached)
    ));
    assert_eq!(w.state(), LocalWatchState::Incomplete);
    fs::write(tree.0.join("received/child/next"), b"x").unwrap();
    collect(&mut w, |e| {
        has(e, "received/child/next", &LocalActivity::Written)
    });
}

#[test]
fn moved_out_subtree_stops_reporting_external_activity() {
    let tree = Tree::new();
    let outside = Tree::new();
    fs::create_dir_all(tree.0.join("sub/child")).unwrap();
    let mut w = tree.watcher();
    w.poll();
    fs::rename(tree.0.join("sub"), outside.0.join("sub")).unwrap();
    fs::write(outside.0.join("sub/child/external"), b"secret").unwrap();
    let events = collect(&mut w, |e| has(e, "sub", &LocalActivity::MovedOut));
    assert!(!events.iter().any(|e| e.path.ends_with("external")));
    assert_eq!(w.watches.len(), 1);
    fs::write(outside.0.join("sub/child/after"), b"outside").unwrap();
    assert!(!w.poll().notices.iter().any(|e| e.path.ends_with("after")));
}

#[test]
fn files_move_in_out_and_rename_within_scope() {
    let tree = Tree::new();
    let outside = Tree::new();
    fs::write(outside.0.join("file"), b"x").unwrap();
    let mut w = tree.watcher();
    w.poll();
    fs::rename(outside.0.join("file"), tree.0.join("file")).unwrap();
    collect(&mut w, |e| has(e, "file", &LocalActivity::MovedIn));
    fs::rename(tree.0.join("file"), tree.0.join("renamed")).unwrap();
    collect(&mut w, |e| {
        has(
            e,
            "renamed",
            &LocalActivity::Moved {
                from: "file".into(),
            },
        )
    });
    fs::rename(tree.0.join("renamed"), outside.0.join("file")).unwrap();
    collect(&mut w, |e| has(e, "renamed", &LocalActivity::MovedOut));
}

#[test]
fn cookies_span_polls_and_expiration_does_not_invent_a_destination() {
    let tree = Tree::new();
    let mut w = tree.watcher();
    w.poll();
    let wd = root_wd(&w);
    w.process(wd, inotify::ReadFlags::MOVED_FROM, 42, Some(b"before"));
    assert!(w.poll().notices.is_empty());
    w.process(wd, inotify::ReadFlags::MOVED_TO, 42, Some(b"after"));
    assert!(has(
        &w.poll().notices,
        "after",
        &LocalActivity::Moved {
            from: "before".into()
        }
    ));
    w.process(wd, inotify::ReadFlags::MOVED_FROM, 43, Some(b"gone"));
    w.moves.get_mut(&43).unwrap().since = Instant::now() - Duration::from_secs(1);
    assert!(has(&w.poll().notices, "gone", &LocalActivity::MovedOut));
}

#[test]
fn non_utf8_names_are_preserved_and_symlinks_are_not_followed() {
    let tree = Tree::new();
    let outside = Tree::new();
    symlink(&outside.0, tree.0.join("link")).unwrap();
    let mut w = tree.watcher();
    w.poll();
    assert_eq!(w.watches.len(), 1);
    fs::write(outside.0.join("outside"), b"x").unwrap();
    let name = std::ffi::OsString::from_vec(vec![b'f', 0xff]);
    fs::write(tree.0.join(&name), b"x").unwrap();
    let events = collect(&mut w, |e| e.iter().any(|n| n.path == Path::new(&name)));
    assert!(!events.iter().any(|n| n.path.ends_with("outside")));
    assert_eq!(fs::read(outside.0.join("outside")).unwrap(), b"x");
    let scope = LocalScope::new("c", "a", &tree.0.join("link")).unwrap();
    assert!(matches!(
        LocalWatcher::start(scope, LocalWatchOptions::default()),
        Err(LocalWatchError::Unavailable)
    ));
}

#[test]
fn root_replacement_or_move_stops_session_and_preserves_new_root() {
    let tree = Tree::new();
    let outside = Tree::new();
    let mut w = tree.watcher();
    w.poll();
    fs::rename(&tree.0, outside.0.join("original")).unwrap();
    fs::create_dir(&tree.0).unwrap();
    fs::write(tree.0.join("replacement"), b"x").unwrap();
    let batch = w.poll();
    assert_eq!(batch.state, LocalWatchState::Stopped);
    assert!(has(
        &batch.notices,
        "",
        &LocalActivity::Recheck(LocalCoverageGap::RootChanged)
    ));
    assert!(w.poll().notices.is_empty());
    assert_eq!(fs::read(tree.0.join("replacement")).unwrap(), b"x");
}

#[test]
fn scope_isolation_has_no_shared_event_stream() {
    let a = Tree::new();
    let b = Tree::new();
    let mut wa = a.watcher();
    let mut wb = b.watcher();
    wa.poll();
    wb.poll();
    fs::write(a.0.join("only-a"), b"x").unwrap();
    collect(&mut wa, |e| has(e, "only-a", &LocalActivity::Created));
    assert!(wb.poll().notices.is_empty());
    let other_scope = LocalScope::new("other-client", "other-account", &a.0).unwrap();
    let mut other = LocalWatcher::start(other_scope.clone(), LocalWatchOptions::default()).unwrap();
    assert_eq!(other.poll().scope, other_scope);
}

#[test]
fn application_overflow_is_bounded_reserved_and_sticky() {
    let tree = Tree::new();
    let mut w = tree.with_options(LocalWatchOptions {
        max_queued: 2,
        ..Default::default()
    });
    w.poll();
    let wd = root_wd(&w);
    for _ in 0..20 {
        w.process(wd, inotify::ReadFlags::CREATE, 0, Some(b"activity"));
    }
    let batch = w.poll();
    assert_eq!(batch.notices.len(), 3);
    assert_eq!(
        batch.notices.last().unwrap().activity,
        LocalActivity::Recheck(LocalCoverageGap::ApplicationOverflow)
    );
    assert_eq!(batch.state, LocalWatchState::Incomplete);
    assert_eq!(w.poll().state, LocalWatchState::Incomplete);
}

#[test]
fn kernel_overflow_and_unexpected_watch_loss_are_explicit() {
    let tree = Tree::new();
    let mut w = tree.watcher();
    w.poll();
    w.process(-1, inotify::ReadFlags::QUEUE_OVERFLOW, 0, None);
    assert!(has(
        &w.poll().notices,
        "",
        &LocalActivity::Recheck(LocalCoverageGap::KernelOverflow)
    ));
    w.process(999999, inotify::ReadFlags::IGNORED, 0, None);
    assert!(has(
        &w.poll().notices,
        "",
        &LocalActivity::Recheck(LocalCoverageGap::WatchLost)
    ));
}

#[test]
fn watch_traversal_and_pending_move_limits_are_explicit() {
    let tree = Tree::new();
    fs::create_dir(tree.0.join("child")).unwrap();
    let mut limited = tree.with_options(LocalWatchOptions {
        max_watches: 1,
        ..Default::default()
    });
    assert_eq!(limited.watches.len(), 1);
    assert!(limited
        .poll()
        .notices
        .iter()
        .any(|n| n.activity == LocalActivity::Recheck(LocalCoverageGap::ResourceLimit)));
    fs::write(tree.0.join("file"), b"x").unwrap();
    let mut limited = tree.with_options(LocalWatchOptions {
        max_scan_entries: 1,
        ..Default::default()
    });
    assert!(limited
        .poll()
        .notices
        .iter()
        .any(|n| n.activity == LocalActivity::Recheck(LocalCoverageGap::ResourceLimit)));
    let mut w = tree.with_options(LocalWatchOptions {
        max_pending_moves: 1,
        ..Default::default()
    });
    w.poll();
    let wd = root_wd(&w);
    w.process(wd, inotify::ReadFlags::MOVED_FROM, 1, Some(b"a"));
    w.process(wd, inotify::ReadFlags::MOVED_FROM, 2, Some(b"b"));
    assert_eq!(w.moves.len(), 1);
    assert!(has(
        &w.poll().notices,
        "b",
        &LocalActivity::Recheck(LocalCoverageGap::ResourceLimit)
    ));
}

#[test]
fn lost_subdirectory_and_inaccessible_attachment_preserve_other_watches() {
    let tree = Tree::new();
    fs::create_dir(tree.0.join("child")).unwrap();
    let mut w = tree.watcher();
    w.poll();
    let wd = *w
        .watches
        .iter()
        .find(|(_, d)| d.path == Path::new("child"))
        .unwrap()
        .0;
    inotify::remove_watch(&w.source, wd).unwrap();
    collect(&mut w, |e| {
        has(
            e,
            "child",
            &LocalActivity::Recheck(LocalCoverageGap::WatchLost),
        )
    });
    w.install_tree(Path::new("missing"));
    assert!(has(
        &w.poll().notices,
        "missing",
        &LocalActivity::Recheck(LocalCoverageGap::Inaccessible)
    ));
    fs::write(tree.0.join("healthy"), b"x").unwrap();
    collect(&mut w, |e| has(e, "healthy", &LocalActivity::Written));
}

#[test]
fn deleted_subtree_does_not_leave_active_watches() {
    let tree = Tree::new();
    fs::create_dir_all(tree.0.join("child/sub")).unwrap();
    let mut w = tree.watcher();
    w.poll();
    fs::remove_dir_all(tree.0.join("child")).unwrap();
    collect(&mut w, |e| has(e, "child", &LocalActivity::Removed));
    assert_eq!(w.watches.len(), 1);
}

#[test]
fn burst_is_processed_over_bounded_polls_without_silent_loss() {
    let tree = Tree::new();
    let mut w = tree.with_options(LocalWatchOptions {
        max_reads_per_poll: 1,
        ..Default::default()
    });
    w.poll();
    for i in 0..150 {
        fs::write(tree.0.join(format!("f{i}")), b"x").unwrap();
    }
    let events = collect(&mut w, |e| {
        e.iter()
            .filter(|n| n.activity == LocalActivity::Created)
            .count()
            == 150
    });
    assert_eq!(
        events
            .iter()
            .filter(|n| n.activity == LocalActivity::Created)
            .count(),
        150
    );
}

#[test]
fn invalid_scopes_limits_and_symlink_ancestors_are_rejected() {
    for root in [
        "/",
        "relative",
        "/tmp/../bad",
        "/tmp/./bad",
        "/tmp//bad",
        "/tmp/bad/",
    ] {
        assert!(LocalScope::new("c", "a", Path::new(root)).is_err());
    }
    let tree = Tree::new();
    assert!(LocalScope::new("", "a", &tree.0).is_err());
    assert!(LocalScope::new("c", "\n", &tree.0).is_err());
    let scope = LocalScope::new("c", "a", &tree.0).unwrap();
    assert!(matches!(
        LocalWatcher::start(
            scope,
            LocalWatchOptions {
                max_queued: 0,
                ..Default::default()
            }
        ),
        Err(LocalWatchError::InvalidInput)
    ));
    let outside = Tree::new();
    fs::create_dir(outside.0.join("child")).unwrap();
    symlink(&outside.0, tree.0.join("link")).unwrap();
    let scope = LocalScope::new("c", "a", &tree.0.join("link/child")).unwrap();
    assert!(matches!(
        LocalWatcher::start(scope, LocalWatchOptions::default()),
        Err(LocalWatchError::Unavailable)
    ));
}

#[test]
fn pending_directory_does_not_misattribute_events() {
    let tree = Tree::new();
    fs::create_dir(tree.0.join("child")).unwrap();
    let mut w = tree.watcher();
    w.poll();
    let root = root_wd(&w);
    let child = *w
        .watches
        .iter()
        .find(|(_, d)| d.path == Path::new("child"))
        .unwrap()
        .0;
    w.process(
        root,
        inotify::ReadFlags::MOVED_FROM | inotify::ReadFlags::ISDIR,
        7,
        Some(b"child"),
    );
    w.process(child, inotify::ReadFlags::MODIFY, 0, Some(b"ambiguous"));
    let events = w.poll().notices;
    assert!(!events.iter().any(|n| n.path.ends_with("ambiguous")));
    assert!(has(
        &events,
        "child",
        &LocalActivity::Recheck(LocalCoverageGap::MoveUncertain)
    ));
}

#[test]
fn open_writer_produces_write_before_close() {
    let tree = Tree::new();
    let mut w = tree.watcher();
    w.poll();
    let mut file = File::create(tree.0.join("open")).unwrap();
    file.write_all(b"x").unwrap();
    let events = collect(&mut w, |e| has(e, "open", &LocalActivity::Written));
    assert!(!has(&events, "open", &LocalActivity::ClosedAfterWrite));
    drop(file);
    collect(&mut w, |e| has(e, "open", &LocalActivity::ClosedAfterWrite));
}
