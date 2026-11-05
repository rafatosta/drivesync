use super::*;
use crate::{AuthError, RemoteFileCapabilities};
use std::{cell::RefCell, fs, path::PathBuf};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "drivesync-detector-check-{}",
            oauth2::CsrfToken::new_random().secret()
        )))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scope(account: &str, root: &str) -> Scope {
    Scope {
        client: "client".into(),
        account: account.into(),
        root: root.into(),
        corpus: "user".into(),
    }
}

fn item(id: &str, parent: Option<&str>, folder: bool, name: &str) -> File {
    File {
        id: id.into(),
        name: Some(name.into()),
        mime_type: Some(if folder { FOLDER } else { "text/plain" }.into()),
        parents: parent.map(|p| vec![p.into()]),
        trashed: Some(false),
        version: Some("1".into()),
        modified_time: None,
        drive_id: None,
        capabilities: folder.then_some(RemoteFileCapabilities {
            can_list_children: Some(true),
        }),
    }
}

fn change(id: &str, file: Option<File>, removed: bool) -> Change {
    Change {
        file_id: Some(id.into()),
        file,
        removed,
        change_type: Some("file".into()),
        drive_id: None,
    }
}

fn end(token: &str, changes: Vec<Change>) -> ChangePage {
    ChangePage {
        changes,
        next_page_token: None,
        new_start_page_token: Some(token.into()),
    }
}

struct Fake {
    scans: Vec<Vec<File>>,
    scan: RefCell<usize>,
    pages: RefCell<VecDeque<Result<ChangePage, ProviderError>>>,
    repeated_child_page: bool,
}

impl Fake {
    fn new(scans: Vec<Vec<File>>, pages: Vec<Result<ChangePage, ProviderError>>) -> Self {
        Self {
            scans,
            scan: RefCell::new(0),
            pages: RefCell::new(pages.into()),
            repeated_child_page: false,
        }
    }
}

impl Provider for Fake {
    fn start(&self, _: &Corpus) -> Result<String, ProviderError> {
        Ok("start".into())
    }
    fn file(&self, id: &str) -> Result<File, ProviderError> {
        let mut scan = self.scan.borrow_mut();
        let index = (*scan).min(self.scans.len() - 1);
        *scan += 1;
        self.scans[index]
            .iter()
            .find(|f| f.id == id)
            .cloned()
            .ok_or(ProviderError::Incomplete)
    }
    fn children(
        &self,
        _: &Corpus,
        parent: &str,
        page: Option<&str>,
    ) -> Result<FilePage, ProviderError> {
        let index = (*self.scan.borrow() - 1).min(self.scans.len() - 1);
        let children: Vec<_> = self.scans[index]
            .iter()
            .filter(|f| {
                f.parents
                    .as_ref()
                    .is_some_and(|p| p == &[parent.to_string()])
            })
            .cloned()
            .collect();
        let offset = page.map_or(0, |p| p.parse::<usize>().unwrap());
        let files = children.iter().skip(offset).take(1).cloned().collect();
        let next_page_token = if self.repeated_child_page {
            Some("0".into())
        } else if offset + 1 < children.len() {
            Some((offset + 1).to_string())
        } else {
            None
        };
        Ok(FilePage {
            files,
            next_page_token,
            incomplete_search: false,
        })
    }
    fn changes(&self, _: &Corpus, _: &str) -> Result<ChangePage, ProviderError> {
        self.pages
            .borrow_mut()
            .pop_front()
            .expect("unexpected change request")
    }
}

fn files() -> Vec<File> {
    vec![
        item("root", None, true, "Root"),
        item("sub", Some("root"), true, "Sub"),
        item("one", Some("sub"), false, "one"),
    ]
}

fn ack(journal: &Journal, scope: &Scope) {
    for event in journal.pending(scope).unwrap() {
        journal.acknowledge(scope, event.id).unwrap();
    }
}

#[test]
fn initial_scan_reconciles_changes_and_persists_pending_on_restart() {
    let dir = Directory::new();
    let journal = Journal::open(&dir.0).unwrap();
    let scope = scope("account", "root");
    let mut second = files();
    second.push(item("new", Some("root"), false, "new"));
    let fake = Fake::new(
        vec![files(), second],
        vec![
            Ok(end(
                "after-change",
                vec![change(
                    "new",
                    Some(item("new", Some("root"), false, "new")),
                    false,
                )],
            )),
            Ok(end("stable", vec![])),
        ],
    );
    let Check::Initialized(events) =
        check_with(&fake, &journal, &scope, &Corpus::User, Limits::default()).unwrap()
    else {
        panic!("expected initialization")
    };
    assert_eq!(
        events
            .iter()
            .filter(|e| e.event.kind == EventKind::Initial)
            .count(),
        4
    );
    assert!(events.iter().any(
        |e| e.event.kind == EventKind::Reevaluate && e.event.file_id.as_deref() == Some("new")
    ));
    assert_eq!(journal.load(&scope).unwrap().unwrap().cursor, "stable");
    let ids: Vec<_> = events.iter().map(|e| e.id).collect();
    drop(journal);
    let journal = Journal::open(&dir.0).unwrap();
    assert_eq!(
        pending(&journal, &scope)
            .unwrap()
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        ids
    );
}

#[test]
fn paginated_changes_and_subtree_moves_reconcile_without_claiming_deletion() {
    let dir = Directory::new();
    let journal = Journal::open(&dir.0).unwrap();
    let scope = scope("a", "root");
    check_with(
        &Fake::new(vec![files()], vec![Ok(end("a", vec![]))]),
        &journal,
        &scope,
        &Corpus::User,
        Limits::default(),
    )
    .unwrap();
    ack(&journal, &scope);
    let root_only = vec![item("root", None, true, "Root")];
    let page = ChangePage {
        changes: vec![change("sub", None, true)],
        next_page_token: Some("next".into()),
        new_start_page_token: None,
    };
    let fake = Fake::new(
        vec![root_only],
        vec![
            Ok(page),
            Ok(end(
                "b",
                vec![change(
                    "outside",
                    Some(item("outside", Some("else"), false, "x")),
                    false,
                )],
            )),
            Ok(end("c", vec![])),
        ],
    );
    let Check::Changes(events) =
        check_with(&fake, &journal, &scope, &Corpus::User, Limits::default()).unwrap()
    else {
        panic!("expected changes")
    };
    assert!(events
        .iter()
        .any(|e| e.event.file_id.as_deref() == Some("sub")
            && e.event.before == Membership::Inside
            && e.event.after == Membership::Unknown));
    assert!(events
        .iter()
        .any(|e| e.event.file_id.as_deref() == Some("one")
            && e.event.before == Membership::Inside
            && e.event.after == Membership::Unknown));
    assert!(!events
        .iter()
        .any(|e| e.event.file_id.as_deref() == Some("outside")));
    assert_eq!(journal.load(&scope).unwrap().unwrap().cursor, "c");
}

#[test]
fn errors_and_nonconvergence_never_commit_cursor_or_events() {
    let dir = Directory::new();
    let journal = Journal::open(&dir.0).unwrap();
    let scope = scope("a", "root");
    check_with(
        &Fake::new(vec![files()], vec![Ok(end("saved", vec![]))]),
        &journal,
        &scope,
        &Corpus::User,
        Limits::default(),
    )
    .unwrap();
    ack(&journal, &scope);
    let fail = Fake::new(
        vec![files()],
        vec![Err(ProviderError::Request(EngineError::Authentication(
            AuthError::Network,
        )))],
    );
    assert!(matches!(
        check_with(&fail, &journal, &scope, &Corpus::User, Limits::default()),
        Err(DetectionError::Provider(EngineError::Authentication(
            AuthError::Network
        )))
    ));
    let changing = Fake::new(
        vec![files()],
        vec![Ok(end("next", vec![change("one", None, true)]))],
    );
    let limits = Limits {
        rounds: 1,
        ..Limits::default()
    };
    assert!(matches!(
        check_with(&changing, &journal, &scope, &Corpus::User, limits),
        Err(DetectionError::Incomplete)
    ));
    let mut cycle = Fake::new(vec![files()], vec![]);
    cycle.repeated_child_page = true;
    assert!(matches!(
        check_with(&cycle, &journal, &scope, &Corpus::User, Limits::default()),
        Err(DetectionError::Incomplete)
    ));
    assert_eq!(journal.load(&scope).unwrap().unwrap().cursor, "saved");
    assert!(journal.pending(&scope).unwrap().is_empty());
}

#[test]
fn continuous_activity_exhausts_default_rounds_without_losing_pending_events() {
    let dir = Directory::new();
    let journal = Journal::open(&dir.0).unwrap();
    let scope = scope("a", "root");
    check_with(
        &Fake::new(vec![files()], vec![Ok(end("saved", vec![]))]),
        &journal,
        &scope,
        &Corpus::User,
        Limits::default(),
    )
    .unwrap();
    let before = journal.load(&scope).unwrap().unwrap();
    let pending_before = journal.pending(&scope).unwrap();
    let pages = (0..Limits::default().rounds)
        .map(|round| {
            Ok(end(
                &format!("cursor-{round}"),
                vec![change("one", Some(files()[2].clone()), false)],
            ))
        })
        .collect();
    let fake = Fake::new(vec![files()], pages);
    assert!(matches!(
        check_with(&fake, &journal, &scope, &Corpus::User, Limits::default()),
        Err(DetectionError::Incomplete)
    ));
    let after = journal.load(&scope).unwrap().unwrap();
    assert_eq!(after.generation, before.generation);
    assert_eq!(after.cursor, before.cursor);
    assert_eq!(after.inventory, before.inventory);
    assert_eq!(journal.pending(&scope).unwrap(), pending_before);
}

#[test]
fn empty_root_and_two_accounts_have_independent_state() {
    let dir = Directory::new();
    let journal = Journal::open(&dir.0).unwrap();
    for account in ["a", "b"] {
        let scope = scope(account, "root");
        let fake = Fake::new(
            vec![vec![item("root", None, true, "Root")]],
            vec![Ok(end(account, vec![]))],
        );
        let Check::Initialized(events) =
            check_with(&fake, &journal, &scope, &Corpus::User, Limits::default()).unwrap()
        else {
            panic!("expected initialization")
        };
        assert_eq!(events.len(), 2);
        ack(&journal, &scope);
        assert_eq!(journal.load(&scope).unwrap().unwrap().cursor, account);
    }

    let fake = Fake::new(
        vec![vec![item("root", None, true, "Root")]],
        vec![Ok(end("unchanged", vec![]))],
    );
    assert!(matches!(
        check_with(
            &fake,
            &journal,
            &scope("a", "root"),
            &Corpus::User,
            Limits::default()
        ),
        Ok(Check::NoChanges)
    ));
    assert_eq!(
        journal.load(&scope("b", "root")).unwrap().unwrap().cursor,
        "b"
    );
}

#[test]
fn create_modify_rename_move_in_move_out_and_tombstones_are_observations() {
    let base = files();
    let mut renamed = base.clone();
    renamed.iter_mut().find(|f| f.id == "one").unwrap().name = Some("renamed".into());
    let mut modified = base.clone();
    modified.iter_mut().find(|f| f.id == "one").unwrap().version = Some("2".into());
    let mut added = base.clone();
    added.push(item("new", Some("root"), false, "new"));
    let mut moved_out = base.clone();
    moved_out.retain(|f| f.id != "one");
    let scenarios = vec![
        (
            "new",
            added.clone(),
            Some(item("new", Some("root"), false, "new")),
            false,
            Membership::Outside,
            Membership::Inside,
        ),
        (
            "one",
            modified.clone(),
            Some(modified[2].clone()),
            false,
            Membership::Inside,
            Membership::Inside,
        ),
        (
            "one",
            renamed.clone(),
            Some(renamed[2].clone()),
            false,
            Membership::Inside,
            Membership::Inside,
        ),
        (
            "new",
            added,
            Some(item("new", Some("root"), false, "new")),
            false,
            Membership::Outside,
            Membership::Inside,
        ),
        (
            "one",
            moved_out.clone(),
            Some(item("one", Some("else"), false, "one")),
            false,
            Membership::Inside,
            Membership::Outside,
        ),
        (
            "one",
            moved_out,
            None,
            true,
            Membership::Inside,
            Membership::Unknown,
        ),
    ];
    for (id, current, observed, removed, before, after) in scenarios {
        let dir = Directory::new();
        let journal = Journal::open(&dir.0).unwrap();
        let scope = scope("a", "root");
        check_with(
            &Fake::new(vec![base.clone()], vec![Ok(end("a", vec![]))]),
            &journal,
            &scope,
            &Corpus::User,
            Limits::default(),
        )
        .unwrap();
        ack(&journal, &scope);
        let fake = Fake::new(
            vec![current],
            vec![
                Ok(end("b", vec![change(id, observed, removed)])),
                Ok(end("c", vec![])),
            ],
        );
        let Check::Changes(events) =
            check_with(&fake, &journal, &scope, &Corpus::User, Limits::default()).unwrap()
        else {
            panic!("expected changes")
        };
        let event = events
            .iter()
            .find(|e| e.event.file_id.as_deref() == Some(id))
            .unwrap();
        assert_eq!(event.event.kind, EventKind::Reevaluate);
        assert_eq!(event.event.before, before);
        assert_eq!(event.event.after, after);
    }
}

#[test]
fn moving_folder_inside_emits_reevaluations_for_unchanged_descendants() {
    let dir = Directory::new();
    let journal = Journal::open(&dir.0).unwrap();
    let scope = scope("a", "root");
    let initial = vec![item("root", None, true, "Root")];
    check_with(
        &Fake::new(vec![initial], vec![Ok(end("a", vec![]))]),
        &journal,
        &scope,
        &Corpus::User,
        Limits::default(),
    )
    .unwrap();
    ack(&journal, &scope);
    let moved = vec![
        item("root", None, true, "Root"),
        item("sub", Some("root"), true, "Sub"),
        item("nested", Some("sub"), false, "file"),
    ];
    let fake = Fake::new(
        vec![moved.clone()],
        vec![
            Ok(end("b", vec![change("sub", Some(moved[1].clone()), false)])),
            Ok(end("c", vec![])),
        ],
    );
    let Check::Changes(events) =
        check_with(&fake, &journal, &scope, &Corpus::User, Limits::default()).unwrap()
    else {
        panic!("expected changes")
    };
    assert!(events
        .iter()
        .any(|e| e.event.file_id.as_deref() == Some("nested")
            && e.event.before == Membership::Outside
            && e.event.after == Membership::Inside));
}

#[test]
fn multiple_changes_are_redelivered_and_invalid_cursor_preserves_queue() {
    let dir = Directory::new();
    let journal = Journal::open(&dir.0).unwrap();
    let scope = scope("a", "root");
    check_with(
        &Fake::new(vec![files()], vec![Ok(end("a", vec![]))]),
        &journal,
        &scope,
        &Corpus::User,
        Limits::default(),
    )
    .unwrap();
    ack(&journal, &scope);
    let change_one = change("one", Some(files()[2].clone()), false);
    let fake = Fake::new(
        vec![files()],
        vec![
            Ok(end("b", vec![change_one.clone(), change_one])),
            Ok(end("c", vec![])),
        ],
    );
    let Check::Changes(events) =
        check_with(&fake, &journal, &scope, &Corpus::User, Limits::default()).unwrap()
    else {
        panic!("expected changes")
    };
    assert_eq!(events.len(), 2);
    assert_ne!(events[0].id, events[1].id);
    let invalid = Fake::new(vec![files()], vec![Err(ProviderError::RejectedCursor)]);
    assert!(matches!(
        check_with(&invalid, &journal, &scope, &Corpus::User, Limits::default()),
        Err(DetectionError::CursorRejected)
    ));
    assert_eq!(journal.load(&scope).unwrap().unwrap().cursor, "c");
    assert_eq!(pending(&journal, &scope).unwrap().len(), 2);
}

#[test]
fn inaccessible_subfolder_and_reconfigured_root_preserve_old_state() {
    let dir = Directory::new();
    let journal = Journal::open(&dir.0).unwrap();
    let original = scope("a", "root");
    check_with(
        &Fake::new(vec![files()], vec![Ok(end("a", vec![]))]),
        &journal,
        &original,
        &Corpus::User,
        Limits::default(),
    )
    .unwrap();
    let pending_count = pending(&journal, &original).unwrap().len();
    let mut inaccessible = files();
    inaccessible[1].capabilities = None;
    let fake = Fake::new(vec![inaccessible], vec![]);
    assert!(matches!(
        check_with(&fake, &journal, &original, &Corpus::User, Limits::default()),
        Err(DetectionError::Incomplete)
    ));
    let replacement = scope("a", "other-root");
    let fake = Fake::new(
        vec![vec![item("other-root", None, true, "Other")]],
        vec![Ok(end("b", vec![]))],
    );
    check_with(
        &fake,
        &journal,
        &replacement,
        &Corpus::User,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(journal.load(&original).unwrap().unwrap().cursor, "a");
    assert_eq!(pending(&journal, &original).unwrap().len(), pending_count);
    assert_eq!(journal.load(&replacement).unwrap().unwrap().cursor, "b");
}

#[test]
fn killed_query_preserves_checkpoint_and_pending_events() {
    const CHILD: &str = "DRIVESYNC_TEST_KILLED_QUERY";
    struct PausedProvider {
        fake: Fake,
        marker: PathBuf,
    }
    impl Provider for PausedProvider {
        fn start(&self, corpus: &Corpus) -> Result<String, ProviderError> {
            self.fake.start(corpus)
        }
        fn changes(&self, _: &Corpus, _: &str) -> Result<ChangePage, ProviderError> {
            fs::write(&self.marker, b"query in progress").unwrap();
            std::thread::sleep(std::time::Duration::from_secs(10));
            Err(ProviderError::Incomplete)
        }
        fn children(
            &self,
            corpus: &Corpus,
            parent: &str,
            page: Option<&str>,
        ) -> Result<FilePage, ProviderError> {
            self.fake.children(corpus, parent, page)
        }
        fn file(&self, id: &str) -> Result<File, ProviderError> {
            self.fake.file(id)
        }
    }

    if let Some(path) = std::env::var_os(CHILD) {
        let path = PathBuf::from(path);
        let journal = Journal::open(&path).unwrap();
        let provider = PausedProvider {
            fake: Fake::new(vec![files()], vec![]),
            marker: path.join("query-started"),
        };
        let result = check_with(
            &provider,
            &journal,
            &scope("a", "root"),
            &Corpus::User,
            Limits::default(),
        );
        panic!("query unexpectedly finished: {result:?}");
    }

    let dir = Directory::new();
    let journal = Journal::open(&dir.0).unwrap();
    let scope = scope("a", "root");
    check_with(
        &Fake::new(vec![files()], vec![Ok(end("saved", vec![]))]),
        &journal,
        &scope,
        &Corpus::User,
        Limits::default(),
    )
    .unwrap();
    let old_checkpoint = journal.load(&scope).unwrap().unwrap();
    let old_pending = journal.pending(&scope).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "remote_detector::detector::tests::killed_query_preserves_checkpoint_and_pending_events",
            "--nocapture",
        ])
        .env(CHILD, &dir.0)
        .spawn()
        .unwrap();
    let marker = dir.0.join("query-started");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    let mut started = false;
    while std::time::Instant::now() < deadline {
        if marker.is_file() {
            started = true;
            break;
        }
        if child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let kill_result = child.kill();
    let exit = child.wait().unwrap();
    assert!(started, "provider query did not start");
    kill_result.expect("query exited before interruption");
    assert!(!exit.success());

    let journal = Journal::open(&dir.0).unwrap();
    let checkpoint = journal.load(&scope).unwrap().unwrap();
    assert_eq!(checkpoint.generation, old_checkpoint.generation);
    assert_eq!(checkpoint.cursor, old_checkpoint.cursor);
    assert_eq!(checkpoint.inventory, old_checkpoint.inventory);
    assert_eq!(journal.pending(&scope).unwrap(), old_pending);

    let fake = Fake::new(
        vec![files()],
        vec![
            Ok(end(
                "next",
                vec![change("one", Some(files()[2].clone()), false)],
            )),
            Ok(end("stable", vec![])),
        ],
    );
    assert!(matches!(
        check_with(&fake, &journal, &scope, &Corpus::User, Limits::default()),
        Ok(Check::Changes(_))
    ));
    let pending = journal.pending(&scope).unwrap();
    assert_eq!(&pending[..old_pending.len()], old_pending);
    assert_eq!(pending.len(), old_pending.len() + 1);
    assert_eq!(journal.load(&scope).unwrap().unwrap().cursor, "stable");
}

#[test]
fn incremental_checks_skip_tree_walk_for_empty_feed_and_known_parent_file() {
    let directory = Directory::new();
    let journal = Journal::open(&directory.0).unwrap();
    let scope = scope("account", "root");
    let baseline = Fake::new(vec![files()], vec![Ok(end("baseline", vec![]))]);
    verify(
        &baseline,
        &journal,
        &scope,
        &Corpus::User,
        Limits::default(),
        true,
    )
    .unwrap();
    ack(&journal, &scope);

    let quiet = Fake::new(vec![], vec![Ok(end("quiet", vec![]))]);
    assert!(matches!(
        verify(
            &quiet,
            &journal,
            &scope,
            &Corpus::User,
            Limits::default(),
            true
        )
        .unwrap(),
        Check::NoChanges
    ));
    assert_eq!(journal.load(&scope).unwrap().unwrap().cursor, "quiet");
    let added = item("fresh", Some("sub"), false, "Fresh");
    let changed = Fake::new(
        vec![],
        vec![Ok(end(
            "updated",
            vec![change("fresh", Some(added), false)],
        ))],
    );
    let Check::Changes(events) = verify(
        &changed,
        &journal,
        &scope,
        &Corpus::User,
        Limits::default(),
        true,
    )
    .unwrap() else {
        panic!("expected a new event")
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event.before, Membership::Outside);
    assert_eq!(events[0].event.after, Membership::Inside);
    let pending = Fake::new(vec![], vec![Ok(end("updated", vec![]))]);
    let Check::Changes(redelivered) = verify(
        &pending,
        &journal,
        &scope,
        &Corpus::User,
        Limits::default(),
        true,
    )
    .unwrap() else {
        panic!("expected pending event")
    };
    assert_eq!(redelivered[0].id, events[0].id);
}
