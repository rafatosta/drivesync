use super::*;
use crate::{
    models::{AccountId, OperationAction, ProviderId, RemoteId, SyncRootId},
    orchestrator::operations::models::OperatorConfig,
};
use std::future::pending;

fn config(account: &str, root: &str) -> OperatorConfig {
    OperatorConfig {
        account_id: AccountId(account.into()),
        sync_root_id: SyncRootId(root.into()),
        provider_id: ProviderId("google".into()),
        local_root: "/test/root".into(),
        remote_root_id: RemoteId("private-remote".into()),
        expected_root_identity: None,
        initial_association_authorized: false,
        trash_propagation_authorized: false,
    }
}

fn execute(config: &OperatorConfig, execution: &str) -> Span {
    Span::execution(
        config,
        Kind::Execute(OperationAction::Download),
        Some(&Id(execution.into())),
        Some(&ItemId("private-item".into())),
        Phase::Receive,
    )
}

#[tokio::test]
async fn isolates_concurrent_executions_and_mismatched_account_or_root() {
    let a = config("account-one", "root-one");
    let other_root = config("account-one", "root-two");
    let other_account = config("account-two", "root-one");
    let inbox = ActivityInbox::for_config(&a);
    let second = ActivityInbox::for_config(&other_root);
    let third = ActivityInbox::for_config(&other_account);
    let check = |records: Vec<Activity>, config: &OperatorConfig, expected: &[&str]| {
        assert_eq!(records.len(), expected.len() * 2);
        let mut executions = Vec::new();
        for [root, child] in records.as_chunks::<2>().0 {
            assert_eq!(child.parent, Some(root.id));
            assert!(Arc::ptr_eq(&root.execution, &child.execution));
            executions.push(child.execution.id.as_ref().unwrap().0.as_str());
            assert_eq!(child.execution.scope.account, config.account_id);
            assert_eq!(child.execution.scope.root, config.sync_root_id);
        }
        executions.sort_unstable();
        assert_eq!(executions, expected);
    };
    let ready = tokio::sync::Barrier::new(5);
    let released = tokio::sync::Barrier::new(5);
    let execution = |sink: ActivityInbox, scope: OperatorConfig, id: &'static str| {
        let (ready, released) = (&ready, &released);
        async move {
            sink.observe(async {
                let root = execute(&scope, id);
                root.within(async {
                    let mut child = Span::start(Phase::Download, State::Waiting(Wait::Response));
                    ready.wait().await;
                    released.wait().await;
                    child.finish(Outcome::Complete);
                })
                .await;
            })
            .await;
        }
    };
    tokio::join!(
        execution(inbox.clone(), a.clone(), "e1"),
        execution(inbox.clone(), a.clone(), "e2"),
        execution(second.clone(), other_root.clone(), "e1"),
        execution(third.clone(), other_account.clone(), "e1"),
        async {
            ready.wait().await;
            check(inbox.current(), &a, &["e1", "e2"]);
            check(second.current(), &other_root, &["e1"]);
            check(third.current(), &other_account, &["e1"]);
            inbox
                .observe(async {
                    assert!(!execute(&other_root, "crossed").enabled());
                    assert!(!execute(&other_account, "crossed").enabled());
                })
                .await;
            released.wait().await;
        }
    );
    assert!(inbox.current().is_empty());
    assert!(second.current().is_empty());
    assert!(third.current().is_empty());
}

#[tokio::test]
async fn samples_progress_and_waits_and_rejects_late_or_out_of_order_updates() {
    let config = config("account", "root");
    let inbox = ActivityInbox::for_config(&config);
    let next = Progress::Bytes(Bytes {
        total: None,
        received: Some(23),
        written: Some(20),
        ..Bytes::default()
    });
    inbox
        .observe(async {
            let mut root = execute(&config, "execution");
            let download = root.within_sync(|| Span::start(Phase::Download, State::Running));
            let context = download.context.as_ref().unwrap().clone();
            let id = context.parent.unwrap();
            let order = download
                .reporter
                .sample(true, || Update::Progress(next))
                .unwrap();
            download.progress(Progress::Bytes(Bytes::default()), true);
            download.progress(next, false);
            context.channel.update(id, order);
            assert_eq!(
                inbox.current()[1].progress,
                Some(Progress::Bytes(Bytes::default()))
            );
            let wait = download.response(Phase::Download, pending::<()>());
            let mut wait = std::pin::pin!(wait);
            assert!(
                std::future::poll_fn(|cx| Poll::Ready(wait.as_mut().poll(cx)))
                    .await
                    .is_pending()
            );
            assert_eq!(inbox.current()[1].state, State::Waiting(Wait::Response));
            download.progress(next, true);
            let late = download
                .reporter
                .sample(true, || Update::Phase(Phase::Download, State::Running))
                .unwrap();
            // Ending a parent retires the open child and makes both updates and descendants stale.
            root.finish(Outcome::Cancelled);
            context.channel.update(id, late);
            download
                .within_sync(|| assert!(!Span::start(Phase::Checksum, State::Running).enabled()));
            assert!(inbox.current().is_empty());
        })
        .await;
    let mut records = inbox.take_updates();
    let root = records.pop().unwrap();
    let child = records.pop().unwrap();
    assert_eq!(root.state, State::Finished(Outcome::Cancelled));
    assert_eq!(child.state, State::Finished(Outcome::Cancelled));
    assert_eq!(child.progress, Some(next));
    assert!(child.bytes_per_second().is_some());
    assert!(records
        .iter()
        .filter(|r| r.id == child.id)
        .all(|r| r.sequence < child.sequence && r.updated <= child.updated));
}

#[tokio::test]
async fn capacity_and_cancellation_are_independent_of_a_reader() {
    let config = config("account-private", "root-private");
    let inbox = ActivityInbox::for_config(&config);
    inbox
        .observe(async {
            let root = execute(&config, "execution-private");
            root.within(async {
                let mut children = Vec::new();
                for _ in 0..(ACTIVE_LIMIT * 2) {
                    children.push(Span::start(Phase::Download, State::Running));
                }
                assert_eq!(inbox.current().len(), ACTIVE_LIMIT);
                for child in &children {
                    child.phase(Phase::Download, State::Waiting(Wait::Provider));
                    child.phase(Phase::Download, State::Running);
                }
                assert_eq!(inbox.take_updates().len(), UPDATE_LIMIT);
                inbox.close();
                assert!(!Span::start(Phase::Upload, State::Running).enabled());
            })
            .await;
            assert!(!execute(&config, "late").enabled());
        })
        .await;
    assert!(inbox.current().is_empty());
    let mut output = Vec::new();
    inbox.0.as_ref().unwrap().write_to(&mut output).unwrap();
    let trace = String::from_utf8(output).unwrap();
    assert!(trace.contains("Finished(Cancelled)"));
    assert!(trace.contains("overwritten="));
    for secret in [
        "account-private",
        "root-private",
        "execution-private",
        "private-item",
        "private-remote",
    ] {
        assert!(!trace.contains(secret));
    }
}

#[tokio::test]
async fn dropped_future_and_hash_threshold_keep_terminal_progress() {
    let config = config("account", "root");
    let inbox = ActivityInbox::for_config(&config);
    let work = inbox.observe(async {
        let mut root = execute(&config, "execution");
        root.within(async {
            for size in [128, HASH_PROGRESS_MIN * 2] {
                let mut hash = HashProgress::new(Phase::Checksum, Some(size), Algorithm::Sha256);
                hash.processed((size / 2) as usize);
                hash.processed((size / 2) as usize);
                hash.finish(Outcome::Complete);
            }
            pending::<()>().await;
        })
        .await;
        root.finish(Outcome::Complete);
    });
    {
        let mut work = std::pin::pin!(work);
        assert!(
            std::future::poll_fn(|cx| Poll::Ready(work.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        assert_eq!(inbox.current().len(), 1);
    }
    assert!(inbox.current().is_empty());
    let records = inbox.take_updates();
    let samples: Vec<_> = records
        .iter()
        .filter(|r| r.phase == Phase::Checksum && r.progress.is_some() && r.state == State::Running)
        .collect();
    assert_eq!(samples.len(), 1);
    assert!(
        matches!(samples[0].progress, Some(Progress::Bytes(bytes)) if bytes.hashed == Some(HASH_PROGRESS_MIN))
    );
    let terminal: Vec<_> = records
        .iter()
        .filter(|r| r.phase == Phase::Checksum && r.state == State::Finished(Outcome::Complete))
        .collect();
    assert_eq!(terminal.len(), 2);
    for (sample, processed) in terminal.iter().zip([128, HASH_PROGRESS_MIN * 2]) {
        assert!(
            matches!(sample.progress, Some(Progress::Bytes(bytes)) if bytes.hashed == Some(processed))
        );
    }
    assert_eq!(
        records.last().unwrap().state,
        State::Finished(Outcome::Cancelled)
    );
}

#[tokio::test]
async fn concurrent_updates_remain_monotonic_without_blocking_on_a_consumer() {
    let config = config("account", "root");
    let inbox = ActivityInbox::for_config(&config);
    inbox
        .observe(async {
            let mut execution = execute(&config, "run");
            std::thread::scope(|scope| {
                for _ in 0..8 {
                    let execution = &execution;
                    scope.spawn(move || {
                        for _ in 0..24 {
                            execution.phase(Phase::Dispatch, State::Waiting(Wait::Provider));
                        }
                    });
                }
            });
            execution.finish(Outcome::Complete);
        })
        .await;
    let updates = inbox.take_updates();
    assert!(updates.len() > 1);
    assert!(updates
        .windows(2)
        .all(|window| window[0].sequence < window[1].sequence
            && window[0].updated <= window[1].updated));
    assert_eq!(
        updates.last().unwrap().state,
        State::Finished(Outcome::Complete)
    );
}

#[tokio::test]
async fn blocked_diagnostic_output_does_not_hold_the_mailbox_lock() {
    use std::sync::mpsc;

    struct BlockedOutput {
        ready: mpsc::SyncSender<()>,
        release: Option<mpsc::Receiver<()>>,
    }

    impl Write for BlockedOutput {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if let Some(release) = self.release.take() {
                self.ready.send(()).unwrap();
                release.recv().unwrap();
            }
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let config = config("account", "root");
    let inbox = ActivityInbox::for_config(&config);
    inbox
        .observe(async {
            let mut execution = execute(&config, "run");
            let channel = inbox.0.as_ref().unwrap().clone();
            let (ready, entered) = mpsc::sync_channel(0);
            let (release, blocked) = mpsc::sync_channel(0);
            let (published, observed) = mpsc::sync_channel(0);
            std::thread::scope(|scope| {
                scope.spawn(move || {
                    channel
                        .write_to(BlockedOutput {
                            ready,
                            release: Some(blocked),
                        })
                        .unwrap();
                });
                entered.recv_timeout(Duration::from_secs(5)).unwrap();
                let span = &execution;
                scope.spawn(move || {
                    span.phase(Phase::Validate, State::Running);
                    let _ = published.send(());
                });
                let advanced = observed.recv_timeout(Duration::from_secs(5));
                release.send(()).unwrap();
                drop(observed);
                advanced.expect("producer should advance before the writer is unblocked");
            });
            execution.finish(Outcome::Complete);
        })
        .await;
    assert_eq!(
        inbox.take_updates().last().unwrap().state,
        State::Finished(Outcome::Complete)
    );
}
