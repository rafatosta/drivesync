use super::*;
use crate::models::{AccountId, SyncRootId};
use std::time::Duration;

fn scope(account: &str, root: &str) -> Scope {
    Scope {
        account: AccountId(account.into()),
        root: SyncRootId(root.into()),
    }
}

fn begin(publisher: &ActivityPublisher, origin: Origin) -> ActivityGuard {
    publisher.begin(
        origin,
        None,
        Phase::Observe,
        State::Running,
        Details::default(),
    )
}

#[test]
fn simultaneous_activity_is_scoped_by_account_root_and_source_session() {
    let channel = ActivityChannel::with_diagnostics(scope("a", "r"), false);
    let other_account = ActivityChannel::with_diagnostics(scope("b", "r"), false);
    let other_root = ActivityChannel::with_diagnostics(scope("a", "s"), false);
    let local = channel.open(Source::Local);
    let remote = channel.open(Source::Remote);
    let other_local = other_account.open(Source::Local);
    let local_guard = begin(&local.publisher(), Origin::Local);
    let remote_guard = begin(&remote.publisher(), Origin::Remote);
    let first = local_guard.child(
        Phase::Checksum,
        State::Running,
        Details::count(Progress::new(Unit::Bytes, 0, Some(200))),
    );
    let second = local_guard.child(
        Phase::Traverse,
        State::Running,
        Details::count(Progress::new(Unit::EntriesCompleted, 0, None)),
    );
    let context = remote_guard.child_from(
        Origin::Shared,
        Phase::Contextualize,
        State::Waiting(Wait::Inventory),
        Details::default(),
    );
    let isolated = begin(&other_local.publisher(), Origin::Local);

    let inbox = channel.inbox();
    let activities = inbox.current();
    assert_eq!(activities.len(), 5);
    assert!(activities.iter().any(|a| a.origin == Origin::Remote));
    assert!(activities
        .iter()
        .any(|a| a.id == context.id.unwrap() && a.parent == remote_guard.id));
    assert_ne!(first.id, second.id);
    assert_eq!(
        activities
            .iter()
            .find(|a| Some(a.id) == first.id)
            .unwrap()
            .details
            .progress[0]
            .unwrap()
            .total,
        Some(200)
    );
    assert_eq!(
        activities
            .iter()
            .find(|a| Some(a.id) == second.id)
            .unwrap()
            .details
            .progress[0]
            .unwrap()
            .total,
        None
    );
    assert_eq!(other_account.inbox().current().len(), 1);
    assert!(other_root.inbox().current().is_empty());
    assert_ne!(inbox.scope(), other_account.inbox().scope());
    assert_ne!(inbox.scope(), other_root.inbox().scope());

    drop(local);
    assert_eq!(inbox.current().len(), 2);
    drop(isolated);
    first.finish(State::Complete);
    second.finish(State::Cancelled);
    assert_eq!(inbox.current().len(), 2);
}

#[test]
fn an_older_sample_cannot_restore_progress_after_completion_failure_or_rebinding() {
    let channel = ActivityChannel::with_diagnostics(scope("a", "r"), true);
    let inbox = channel.inbox();
    let session = channel.open(Source::Local);
    let publisher = session.publisher();
    let guard = begin(&publisher, Origin::Local);
    let id = guard.id.unwrap();
    let at = Instant::now();
    let sample = |sequence, items, path: &str| Sample {
        sequence,
        at: at + Duration::from_millis(sequence),
        value: Reported {
            state: State::Running,
            details: Some(Details::count(Progress::new(
                Unit::EntriesCompleted,
                items,
                None,
            ))),
            scan: Some(ScanProgress {
                path: Some(path.into()),
                items: items as u64,
                pages: 0,
            }),
        },
        coalesced: 0,
    };
    publisher.update(id, sample(10, 12, "current"));
    publisher.update(id, sample(9, 1, "late"));
    let mut work = SynchronizationWork::default();
    assert!(inbox.project(&mut work));
    assert_eq!(work.local.as_ref().unwrap().items, 12);
    let activity = &inbox.current()[0];
    assert_eq!(activity.sequence, 10);
    assert_eq!(activity.details.progress[0].unwrap().completed, 12);

    guard.finish(State::Complete);
    publisher.update(id, sample(11, 13, "after-completion"));
    assert!(inbox.current().is_empty());
    assert!(!inbox.project(&mut work));

    let unfinished = begin(&publisher, Origin::Local);
    session.fail();
    assert!(inbox.current().is_empty());
    publisher.update(unfinished.id.unwrap(), sample(20, 40, "after-failure"));
    assert!(inbox.project(&mut work));
    assert_eq!(
        work.local.as_ref().unwrap().state,
        SynchronizationScanState::Failed
    );
    assert_eq!(work.local.as_ref().unwrap().items, 12);

    let replacement = channel.open(Source::Local);
    session.test_progress(Some("obsolete".into()), 99, 0);
    assert!(inbox.project(&mut work));
    assert!(work.local.is_none());
    replacement.test_progress(Some("replacement".into()), 28, 0);
    assert!(inbox.project(&mut work));
    assert_eq!(work.local.unwrap().items, 28);
    inbox.close();
    assert!(begin(&replacement.publisher(), Origin::Local).id.is_none());
    assert!(inbox.current().is_empty());
    let log = inbox.test_diagnostics();
    assert!(log.contains("event=Finished state=Complete"));
    assert!(log.contains("event=Finished state=Failed"));
    assert!(log.contains("stale=3"));
}

#[test]
fn unavailable_reader_does_not_grow_live_state_or_retain_terminal_activity() {
    let channel = ActivityChannel::with_diagnostics(scope("private-account", "private-root"), true);
    let inbox = channel.inbox();
    let session = channel.open(Source::Remote);
    let publisher = session.publisher();
    let activities: Vec<_> = (0..MAX_ACTIVE + 12)
        .map(|_| begin(&publisher, Origin::Remote))
        .collect();
    assert_eq!(inbox.current().len(), MAX_ACTIVE);
    for completed in 0..1000 {
        activities[0].transition(State::Running, || Details {
            item: Some("private-folder/private-name".into()),
            ..Details::count(Progress::new(Unit::PagesReceived, completed, None))
        });
    }
    assert_eq!(inbox.current().len(), MAX_ACTIVE);
    drop(activities);
    assert!(inbox.current().is_empty());
    let log = inbox.test_diagnostics();
    assert!(log.lines().count() <= diagnostics::CAPACITY + 1);
    assert!(log.contains("event=Finished state=Cancelled"));
    assert!(log.contains("capacity=12"));
    assert!(!log.contains("overwritten=0"));
    assert!(!log.contains("coalesced=0"));
    assert!(!log.contains("private-"));
}

#[tokio::test]
async fn unexpected_task_exit_removes_even_detached_work_from_the_failed_generation() {
    let channel = ActivityChannel::with_diagnostics(scope("a", "r"), true);
    let inbox = channel.inbox();
    let session = channel.open(Source::Remote);
    let publisher = session.publisher();
    let noise = begin(&publisher, Origin::Remote);
    let task_publisher = publisher.clone();
    let exited = tokio::spawn(async move {
        let _guard = begin(&task_publisher, Origin::Remote);
        panic!("isolated detector failure");
    })
    .await;
    assert!(exited.unwrap_err().is_panic());
    session.fail();
    assert!(inbox.current().is_empty());
    noise.set_state(State::Running);
    assert!(inbox.current().is_empty());
    let mut work = SynchronizationWork::default();
    assert!(inbox.project(&mut work));
    assert_eq!(work.remote.unwrap().state, SynchronizationScanState::Failed);
    assert!(inbox
        .test_diagnostics()
        .contains("event=Finished state=Failed"));
}
