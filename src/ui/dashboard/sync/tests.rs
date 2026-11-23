use super::{backend::*, components::*, controller::DashboardController, models::*};
use adw::{gtk, prelude::*};
fn activity() -> ActivityView {
    ActivityView {
        scope: "fixture-a".into(),
        operation_id: Some("download-1".into()),
        item_id: "item-1".into(),
        name: "Relatório 日本語 🦀.docx".repeat(20),
        kind: ActivityKind::Download,
        direction: Direction::RemoteToLocal,
        phase: Phase::Running,
        progress: ProgressView::Indeterminate,
    }
}
#[test]
fn invalid_progress_cannot_become_a_percentage() {
    for value in [
        ProgressView::Indeterminate,
        ProgressView::Bytes {
            transferred: 1,
            total: 0,
        },
        ProgressView::Steps {
            completed: 3,
            total: 2,
        },
    ] {
        assert_eq!(value.measured(), None);
    }
    assert_eq!(
        ProgressView::Bytes {
            transferred: 2,
            total: 4
        }
        .measured(),
        Some((2, 4))
    );
}
#[test]
#[ignore = "requires isolated GTK display"]
fn isolated_components_and_fake_backend() {
    adw::init().unwrap();
    for _ in 0..3 {
        let status = SyncStatus::new();
        for state in [
            StatusView::Initializing,
            StatusView::Scanning,
            StatusView::Synchronizing,
            StatusView::Synced,
            StatusView::Attention,
            StatusView::Offline,
            StatusView::Failed,
            StatusView::Stopped,
        ] {
            status.set_state(state.clone(), "fixture");
            assert_eq!(status.state(), state);
        }
        let mut count = 0;
        let mut child = status.widget.first_child();
        while let Some(current) = child {
            count += 1;
            child = current.next_sibling();
        }
        assert_eq!(count, 3);
        let current = SyncActivity::new();
        current.set_activity(activity());
        assert!(!current.update_progress(
            "fixture-b",
            "download-1",
            ProgressView::Bytes {
                transferred: 9,
                total: 10
            }
        ));
        assert!(!current.update_progress("fixture-a", "previous", ProgressView::Indeterminate));
        assert!(current.update_progress(
            "fixture-a",
            "download-1",
            ProgressView::Bytes {
                transferred: 5,
                total: 10
            }
        ));
        let mut next = activity();
        next.operation_id = Some("download-2".into());
        next.kind = ActivityKind::Upload;
        next.direction = Direction::LocalToRemote;
        current.set_activity(next);
        assert!(!current.update_progress(
            "fixture-a",
            "download-1",
            ProgressView::Bytes {
                transferred: 10,
                total: 10
            }
        ));
        current.clear();
        assert_eq!(current.state(), None);
        assert!(!current.widget.is_visible());
        let attention = SyncAttention::new();
        let item = AttentionItemView {
            scope: "fixture-a".into(),
            id: "issue-1".into(),
            title: "Acesso".into(),
            summary: "Sem acesso".into(),
            action: AttentionAction::OpenIssue("issue-1".into()),
        };
        let activated = std::rc::Rc::new(std::cell::Cell::new(false));
        attention.connect_activated({
            let activated = activated.clone();
            move |action| {
                assert_eq!(action, AttentionAction::OpenIssue("issue-1".into()));
                activated.set(true);
            }
        });
        attention.set_items(vec![item.clone(), item]);
        let button = attention
            .widget
            .first_child()
            .unwrap()
            .downcast::<gtk::Button>()
            .unwrap();
        assert!(button.next_sibling().is_none());
        button.emit_clicked();
        assert!(activated.get());
        attention.clear();
        assert!(!attention.widget.is_visible());
        let dialog = ConflictResolver::new();
        dialog.set_conflict(ConflictView {
            id: "conflict-1".into(),
            revision: 7,
            name: "Relatório".into(),
            local: "Metadados indisponíveis".into(),
            remote: "Metadados indisponíveis".into(),
            differences: "Conteúdo".into(),
        });
        let choices = std::rc::Rc::new(std::cell::Cell::new(0));
        dialog.connect_choice({
            let choices = choices.clone();
            move |request| {
                assert_eq!(request.revision, 7);
                choices.set(choices.get() + 1);
            }
        });
        dialog.local.emit_clicked();
        assert_eq!(dialog.state(), ConflictResolverState::Confirming);
        dialog.confirm.emit_clicked();
        dialog.confirm.emit_clicked();
        assert_eq!(choices.get(), 1);
        assert_eq!(dialog.state(), ConflictResolverState::Submitting);
        for state in [
            ConflictResolverState::Applying,
            ConflictResolverState::Stale,
            ConflictResolverState::Failed,
            ConflictResolverState::Resolved,
        ] {
            dialog.set_state(state);
            assert_eq!(dialog.state(), state);
        }
        dialog.window.close();
        let issue = SyncIssueDialog::new();
        issue.set_issue(IssueView {
            title: "Acesso".into(),
            description: "Descrição longa ".repeat(100),
            guidance: None,
        });
        issue.present();
        issue.close();
    }
    gtk::glib::MainContext::default().block_on(async {
        let controller = DashboardController::new();
        let snapshot = Snapshot {
            scope: "fixture-a".into(),
            status: StatusView::Synchronizing,
            description: "Baixando".into(),
            activity: Some(activity()),
            attention: vec![],
        };
        controller.select(
            "fixture-a".into(),
            Box::new(FakeSyncBackend::new([snapshot.clone()])),
        );
        gtk::glib::timeout_future(std::time::Duration::from_millis(20)).await;
        assert_eq!(controller.activity.state(), Some(activity()));
        controller.select("fixture-b".into(), Box::new(FakeSyncBackend::new([])));
        assert!(!controller.apply(snapshot));
        assert_eq!(controller.activity.state(), None);
        controller.reset();
    });
}
