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
    init_fixture_locale();
    adw::init().unwrap();
    for _ in 0..3 {
        let status = SyncStatus::new();
        let spinner = status
            .widget
            .last_child()
            .unwrap()
            .last_child()
            .and_downcast::<gtk::Spinner>()
            .unwrap();
        let icon = status
            .widget
            .first_child()
            .and_downcast::<gtk::Image>()
            .unwrap();
        assert!(icon.icon_name().is_none());
        use StatusView::*;
        for (state, waits) in [
            (Initializing, false),
            (Checking, true),
            (VerificationIncomplete, false),
            (WaitingForEvidence, false),
            (Finalizing, false),
            (Synchronizing, true),
            (Recovering, true),
            (Synced, false),
            (Attention, false),
            (Offline, false),
            (Failed, false),
            (Stopped, false),
        ] {
            status.set_state(state.clone(), "fixture");
            assert_eq!(spinner.is_visible(), waits);
            assert_eq!(status.state(), state);
        }
        let mut count = 0;
        let mut child = status.widget.first_child();
        while let Some(current) = child {
            count += 1;
            child = current.next_sibling();
        }
        assert_eq!(count, 2);
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
        for (kind, direction, phase, progress) in [
            (
                ActivityKind::Download,
                Direction::RemoteToLocal,
                Phase::Running,
                ProgressView::Indeterminate,
            ),
            (
                ActivityKind::Upload,
                Direction::LocalToRemote,
                Phase::Running,
                ProgressView::Bytes {
                    transferred: 5,
                    total: 10,
                },
            ),
            (
                ActivityKind::Apply,
                Direction::None,
                Phase::Running,
                ProgressView::Steps {
                    completed: 1,
                    total: 2,
                },
            ),
            (
                ActivityKind::Verify,
                Direction::None,
                Phase::Running,
                ProgressView::Bytes {
                    transferred: 1,
                    total: 0,
                },
            ),
            (
                ActivityKind::Apply,
                Direction::None,
                Phase::Failed,
                ProgressView::Indeterminate,
            ),
        ] {
            let mut fixture = activity();
            fixture.kind = kind;
            fixture.direction = direction;
            fixture.phase = phase;
            fixture.progress = progress;
            current.set_activity(fixture.clone());
            assert_eq!(current.state(), Some(fixture));
            current.clear();
            assert!(current.state().is_none());
        }

        let mut rename = activity();
        rename.name = "after.bin".into();
        rename.kind = ActivityKind::Rename {
            previous_name: "before.bin".into(),
        };
        rename.direction = Direction::LocalToRemote;
        current.set_activity(rename);
        assert_eq!(current.state().unwrap().progress, ProgressView::None);
        assert!(!current.update_progress(
            "fixture-a",
            "download-1",
            ProgressView::Bytes {
                transferred: 5,
                total: 10,
            }
        ));
        let mut child = current.widget.first_child();
        while let Some(widget) = child {
            if widget.is::<gtk::ProgressBar>() || widget.is::<gtk::Spinner>() {
                assert!(!widget.is_visible());
            }
            child = widget.next_sibling();
        }

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
            .last_child()
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
        dialog.set_conflict(ConflictView {
            id: "conflict-1".into(),
            revision: 8,
            name: "Relatório".into(),
            local: "Versão atualizada".into(),
            remote: "Sem metadados".into(),
            differences: "Conteúdo".into(),
        });
        dialog.confirm.emit_clicked();
        assert_eq!(
            choices.get(),
            1,
            "a new revision requires another explicit review and confirmation"
        );

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
            severity: Severity::Warning,
            title: "Acesso".into(),
            description: "Descrição longa ".repeat(100),
            guidance: None,
        });
        for severity in [Severity::Info, Severity::Warning, Severity::Error] {
            let fixture = IssueView {
                severity,
                title: "Diagnóstico".into(),
                description: "Sem credenciais ou dados pessoais".into(),
                guidance: None,
            };
            issue.set_issue(fixture.clone());
            assert_eq!(issue.state(), Some(fixture));
        }
        issue.present();
        issue.close();
    }
    gtk::glib::MainContext::default().block_on(async {
        let controller = DashboardController::new();
        let synced = Snapshot {
            overview: None,
            scope: "fixture-a".into(),
            status: StatusView::Synced,
            description: "All files are up to date.".into(),
            item: None,
            active: false,
            activity: None,
            attention: vec![],
        };
        let icon = controller.status.widget.first_child().unwrap();
        let icon_updates = std::rc::Rc::new(std::cell::Cell::new(0));
        icon.connect_notify_local(Some("css-classes"), {
            let icon_updates = icon_updates.clone();
            move |icon, _| {
                if icon.has_css_class("success") {
                    icon_updates.set(icon_updates.get() + 1);
                }
            }
        });
        controller.select(
            "fixture-a".into(),
            Box::new(FakeSyncBackend::new([synced.clone(), synced])),
        );
        gtk::glib::timeout_future(std::time::Duration::from_millis(20)).await;
        assert_eq!(controller.status.state(), StatusView::Synced);
        assert_eq!(icon_updates.get(), 1);

        let mut measured = activity();
        measured.name = "Relatório.docx".into();
        measured.progress = ProgressView::Bytes {
            transferred: 1,
            total: 4,
        };
        let snapshot = Snapshot {
            overview: None,
            scope: "fixture-a".into(),
            status: StatusView::Synchronizing,
            description: String::new(),
            item: None,
            active: true,
            activity: Some(measured.clone()),
            attention: vec![],
        };
        controller.select(
            "fixture-a".into(),
            Box::new(FakeSyncBackend::new([snapshot.clone()])),
        );
        gtk::glib::timeout_future(std::time::Duration::from_millis(20)).await;
        assert_eq!(controller.activity.state(), Some(measured));
        let card = controller
            .widget
            .first_child()
            .unwrap()
            .next_sibling()
            .unwrap();
        assert_eq!(card.widget_name(), "dashboard-synchronization-feedback");
        let details = card
            .first_child()
            .and_downcast::<gtk::ListBox>()
            .unwrap()
            .row_at_index(0)
            .unwrap();
        assert!(controller.activity.widget.is_ancestor(&details));
        let chevron = details.child().unwrap().last_child().unwrap();
        let activations = std::rc::Rc::new(std::cell::Cell::new(0));
        controller.status.connect_details({
            let activations = activations.clone();
            move || activations.set(activations.get() + 1)
        });
        assert!(!details.is_activatable());
        assert!(!chevron.is_visible());
        let progress = controller
            .activity
            .widget
            .last_child()
            .and_downcast::<gtk::ProgressBar>()
            .unwrap();
        assert!(progress.is_visible());
        assert_eq!(progress.fraction(), 0.25);
        let window = gtk::Window::builder()
            .default_width(360)
            .default_height(500)
            .child(&controller.widget)
            .build();
        window.present();
        gtk::glib::timeout_future(std::time::Duration::from_millis(60)).await;
        save_window(&window, "fixture-dashboard-pt-br.png");
        assert!(controller.activity.update_progress(
            "fixture-a",
            "download-1",
            ProgressView::Steps {
                completed: 1,
                total: 4
            },
        ));
        assert!(!progress.is_visible());
        let spinner = controller
            .activity
            .widget
            .prev_sibling()
            .and_downcast::<gtk::Spinner>()
            .unwrap();
        assert!(!spinner.is_mapped());
        controller.activity.clear();
        controller
            .status
            .set_state(StatusView::Checking, "Conferindo a última operação.");
        assert!(spinner.is_mapped());
        let item = StatusItem {
            name: "Relatório 日本語 🦀.docx".repeat(20),
            path: Some(format!(
                "Projetos/Documentos/{}",
                "Relatório 日本語 🦀.docx".repeat(20)
            )),
        };
        controller.status.render(
            StatusView::Finalizing,
            "Download concluído. Aguardando para conferir a cópia no Drive.",
            Some(&item),
            false,
            false,
        );
        let item_label = spinner.prev_sibling().and_downcast::<gtk::Label>().unwrap();
        assert_eq!(item_label.text(), item.name);
        assert_eq!(item_label.ellipsize(), gtk::pango::EllipsizeMode::Middle);
        assert_eq!(item_label.tooltip_text().as_deref(), item.path.as_deref());
        assert!(!spinner.is_mapped());
        assert!(!details.is_activatable());
        assert!(!chevron.is_visible());
        gtk::glib::timeout_future(std::time::Duration::from_millis(60)).await;
        save_window(&window, "fixture-dashboard-waiting-pt-br.png");
        controller
            .status
            .set_state(StatusView::Failed, "Folder unavailable");
        assert!(!spinner.is_mapped());
        assert!(details.is_activatable());
        assert!(chevron.is_visible());
        details.emit_by_name::<()>("activate", &[]);
        assert_eq!(activations.get(), 1);
        assert_eq!(controller.status.diagnostic().1, "Folder unavailable");
        let conflict = controller.present_conflict(
            ConflictView {
                id: "fixture-conflict".into(),
                revision: 1,
                name: "Relatório.docx".into(),
                local: "Metadados indisponíveis".into(),
                remote: "Metadados indisponíveis".into(),
                differences: "Conteúdos diferentes".into(),
            },
            |_| {},
        );
        conflict.window.present();
        gtk::glib::timeout_future(std::time::Duration::from_millis(60)).await;
        save_window(&conflict.window, "fixture-conflict-pt-br.png");
        conflict.later.emit_clicked();
        window.close();

        controller.select("fixture-b".into(), Box::new(FakeSyncBackend::new([])));
        assert!(!controller.apply(snapshot));
        assert_eq!(controller.activity.state(), None);
        assert!(!progress.is_visible());
        assert!(!details.is_activatable());
        let marker = std::rc::Rc::new(std::cell::Cell::new(false));
        struct OnDrop(std::rc::Rc<std::cell::Cell<bool>>);
        impl Drop for OnDrop {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }
        let slot = super::controller::SubscriptionTask::default();
        let observed = marker.clone();
        slot.set(gtk::glib::spawn_future_local(async move {
            let _guard = OnDrop(observed);
            std::future::pending::<()>().await;
        }));
        gtk::glib::timeout_future(std::time::Duration::from_millis(10)).await;
        drop(slot);
        gtk::glib::timeout_future(std::time::Duration::from_millis(10)).await;
        assert!(
            marker.get(),
            "destroying the owner must cancel pending listeners"
        );
        assert_transient_checks_do_not_replace_waiting_feedback(&controller).await;
        controller.reset();
    });
}

async fn assert_transient_checks_do_not_replace_waiting_feedback(controller: &DashboardController) {
    use std::{collections::VecDeque, time::Duration};

    struct TimedBackend(VecDeque<(u64, Snapshot)>);
    impl SyncBackend for TimedBackend {
        fn next(&mut self) -> NextSnapshot<'_> {
            Box::pin(async {
                let Some((delay, snapshot)) = self.0.pop_front() else {
                    return std::future::pending().await;
                };
                gtk::glib::timeout_future(Duration::from_millis(delay)).await;
                Some(snapshot)
            })
        }
    }

    let finalizing = Snapshot {
        overview: None,
        scope: "fixture-a".into(),
        status: StatusView::Finalizing,
        description: "Waiting to check the copy on Drive.".into(),
        item: None,
        active: false,
        activity: None,
        attention: vec![],
    };
    let checking = Snapshot {
        status: StatusView::Checking,
        description: "Analyzing changes…".into(),
        active: true,
        ..finalizing.clone()
    };
    let failed = Snapshot {
        status: StatusView::Failed,
        description: "Folder unavailable".into(),
        ..finalizing.clone()
    };
    controller.select(
        finalizing.scope.clone(),
        Box::new(TimedBackend(VecDeque::from([
            (0, finalizing.clone()),
            (10, checking.clone()),
            (110, finalizing.clone()),
            (10, checking),
            (40, failed),
        ]))),
    );
    gtk::glib::timeout_future(Duration::from_millis(70)).await;
    assert_eq!(controller.status.state(), StatusView::Finalizing);
    gtk::glib::timeout_future(Duration::from_millis(170)).await;
    assert_eq!(controller.status.state(), StatusView::Failed);
    gtk::glib::timeout_future(Duration::from_millis(420)).await;
    assert_eq!(controller.status.state(), StatusView::Failed);

    controller.select(
        finalizing.scope.clone(),
        Box::new(TimedBackend(VecDeque::from([
            (0, finalizing.clone()),
            (
                10,
                Snapshot {
                    status: StatusView::Checking,
                    active: true,
                    ..finalizing
                },
            ),
        ]))),
    );
    gtk::glib::timeout_future(Duration::from_millis(500)).await;
    assert_eq!(controller.status.state(), StatusView::Checking);
}

#[test]
fn public_status_requires_audit_and_keeps_operation_counts_out_of_bytes() {
    use drivesync_engine::*;
    let mut value = SynchronizationStatus {
        account_id: AccountId("fixture-a".into()),
        state: SynchronizationState::Idle,
        activity: SynchronizationActivity::None,
        current_item: None,
        current_operation: None,
        progress: None,
        verification: None,
        issue: None,
        updated_at: Timestamp { unix_millis: 0 },
    };
    assert_ne!(map_status(&value, false).status, StatusView::Synced);
    assert_eq!(map_status(&value, true).status, StatusView::Synced);
    use crate::ui::components::sync_overview::SyncOverviewState as Overview;
    assert!(matches!(
        map_status(&value, false).overview,
        Some(Overview::Inactive { .. })
    ));
    assert_eq!(
        map_status(&value, true).overview,
        Some(Overview::Completed { last_sync: None })
    );
    value.issue = Some(SynchronizationIssue {
        kind: SynchronizationIssueKind::RecoveryRequired,
        retryable: false,
    });
    assert_ne!(map_status(&value, true).status, StatusView::Synced);
    value.issue = None;
    value.state = SynchronizationState::Running;
    value.activity = SynchronizationActivity::Downloading;
    value.current_item = Some(SynchronizationItem {
        display_name: None,
        item_id: None,
        local_path: None,
        remote_id: Some(models::RemoteId("remote-1".into())),
    });
    value.current_operation = Some(SynchronizationOperation {
        id: "execution-1".into(),
        item_id: models::ItemId("item-1".into()),
        display_name: Some("Relatório.bin".into()),
        newly_observed_remote: false,
        newly_observed_local: false,
        updates_local_content: false,
        updates_remote_content: false,
        rename: None,
        movement: None,
        workspace_shortcut: None,
        directory_creation: None,
        trash: None,
    });
    value.progress = Some(SynchronizationProgress {
        completed: 1,
        total: 2,
    });
    assert_eq!(
        map_status(&value, false).overview,
        Some(Overview::Syncing {
            completed_items: None,
            total_items: None,
            remaining_items: None,
            remaining_bytes: None,
            progress: None,
        })
    );
    for (activity, kind, direction) in [
        (
            SynchronizationActivity::Downloading,
            ActivityKind::Download,
            Direction::RemoteToLocal,
        ),
        (
            SynchronizationActivity::Uploading,
            ActivityKind::Upload,
            Direction::LocalToRemote,
        ),
    ] {
        value.activity = activity;
        let current = map_status(&value, false).activity.unwrap();
        assert_eq!(current.operation_id.as_deref(), Some("execution-1"));
        assert_eq!(current.name, "Relatório.bin");
        assert_eq!(current.kind, kind);
        assert_eq!(current.direction, direction);
        assert_eq!(
            current.progress,
            ProgressView::Steps {
                completed: 1,
                total: 2
            }
        );
    }
    value
        .current_operation
        .as_mut()
        .unwrap()
        .updates_local_content = true;
    for (activity, phase) in [
        (SynchronizationActivity::Downloading, Phase::Running),
        (SynchronizationActivity::Applying, Phase::Confirmed),
    ] {
        value.activity = activity;
        let current = map_status(&value, false).activity.unwrap();
        assert_eq!(current.kind, ActivityKind::UpdateLocalContent);
        assert_eq!(current.direction, Direction::RemoteToLocal);
        assert_eq!(current.phase, phase);
        assert_eq!(
            current.progress,
            ProgressView::Steps {
                completed: 1,
                total: 2
            }
        );
    }
    value
        .current_operation
        .as_mut()
        .unwrap()
        .updates_local_content = false;
    value
        .current_operation
        .as_mut()
        .unwrap()
        .updates_remote_content = true;
    for (activity, phase) in [
        (SynchronizationActivity::Uploading, Phase::Running),
        (SynchronizationActivity::Applying, Phase::Confirmed),
    ] {
        value.activity = activity;
        let current = map_status(&value, false).activity.unwrap();
        assert_eq!(current.kind, ActivityKind::UpdateRemoteContent);
        assert_eq!(current.direction, Direction::LocalToRemote);
        assert_eq!(current.phase, phase);
        assert_eq!(
            current.progress,
            ProgressView::Steps {
                completed: 1,
                total: 2
            }
        );
    }
    value
        .current_operation
        .as_mut()
        .unwrap()
        .updates_remote_content = false;
    for (shortcut, kind) in [
        (
            WorkspaceShortcutActivity::Create,
            ActivityKind::CreateWorkspaceShortcut,
        ),
        (
            WorkspaceShortcutActivity::Update,
            ActivityKind::UpdateWorkspaceShortcut,
        ),
    ] {
        value.current_operation.as_mut().unwrap().workspace_shortcut = Some(shortcut);
        for (activity, phase) in [
            (SynchronizationActivity::ProjectingWorkspace, Phase::Running),
            (SynchronizationActivity::Applying, Phase::Confirmed),
        ] {
            value.activity = activity;
            let view = map_status(&value, false).activity.unwrap();
            assert_eq!(view.kind, kind);
            assert_eq!(view.direction, Direction::RemoteToLocal);
            assert_eq!(view.phase, phase);
            assert_eq!(view.progress, ProgressView::None);
        }
    }
    value.current_operation.as_mut().unwrap().workspace_shortcut = None;
    value.progress = None;
    value.activity = SynchronizationActivity::Downloading;
    value
        .current_operation
        .as_mut()
        .unwrap()
        .newly_observed_remote = true;
    let discovery = map_status(&value, false).activity.unwrap();
    assert_eq!(discovery.kind, ActivityKind::DownloadDiscoveredRemote);
    assert_eq!(discovery.operation_id.as_deref(), Some("execution-1"));
    assert_eq!(discovery.direction, Direction::RemoteToLocal);
    assert_eq!(discovery.progress, ProgressView::Indeterminate);
    value.activity = SynchronizationActivity::Applying;
    let confirmed = map_status(&value, false).activity.unwrap();
    assert_eq!(confirmed.kind, ActivityKind::DownloadDiscoveredRemote);
    assert_eq!(confirmed.phase, Phase::Confirmed);

    value.activity = SynchronizationActivity::Uploading;
    let operation = value.current_operation.as_mut().unwrap();
    operation.newly_observed_remote = false;
    operation.newly_observed_local = true;
    let discovery = map_status(&value, false).activity.unwrap();
    assert_eq!(discovery.kind, ActivityKind::UploadDiscoveredLocal);
    assert_eq!(discovery.operation_id.as_deref(), Some("execution-1"));
    assert_eq!(discovery.direction, Direction::LocalToRemote);
    assert_eq!(discovery.progress, ProgressView::Indeterminate);
    value.activity = SynchronizationActivity::Applying;
    let confirmed = map_status(&value, false).activity.unwrap();
    assert_eq!(confirmed.kind, ActivityKind::UploadDiscoveredLocal);
    assert_eq!(confirmed.phase, Phase::Confirmed);
    value
        .current_operation
        .as_mut()
        .unwrap()
        .newly_observed_local = false;

    for (origin, direction) in [
        (
            drivesync_engine::SynchronizationRenameOrigin::Local,
            Direction::LocalToRemote,
        ),
        (
            drivesync_engine::SynchronizationRenameOrigin::Remote,
            Direction::RemoteToLocal,
        ),
    ] {
        let operation = value.current_operation.as_mut().unwrap();
        operation.newly_observed_local = false;
        operation.display_name = Some("Relatório.bin".into());
        operation.rename = Some(drivesync_engine::SynchronizationRename {
            origin,
            previous_name: "Before.bin".into(),
            new_name: "Relatório.bin".into(),
        });
        for (transition, phase) in [
            (SynchronizationActivity::Renaming, Phase::Running),
            (SynchronizationActivity::Applying, Phase::Confirmed),
        ] {
            value.activity = transition;
            let rename = map_status(&value, false).activity.unwrap();
            assert_eq!(
                rename.kind,
                ActivityKind::Rename {
                    previous_name: "Before.bin".into()
                }
            );
            assert_eq!(rename.name, "Relatório.bin");
            assert_eq!(rename.direction, direction);
            assert_eq!(rename.phase, phase);
            assert_eq!(rename.progress, ProgressView::None);
        }
    }
}

#[test]
fn move_activity_reports_both_paths_without_transfer_progress() {
    use drivesync_engine::*;
    let mut value = SynchronizationStatus {
        account_id: AccountId("fixture-a".into()),
        state: SynchronizationState::Running,
        activity: SynchronizationActivity::Moving,
        current_item: Some(SynchronizationItem {
            display_name: None,
            item_id: None,
            local_path: None,
            remote_id: Some(models::RemoteId("drive-file".into())),
        }),
        current_operation: None,
        progress: None,
        verification: None,
        issue: None,
        updated_at: Timestamp { unix_millis: 0 },
    };
    for (origin, direction) in [
        (SynchronizationMoveOrigin::Local, Direction::LocalToRemote),
        (SynchronizationMoveOrigin::Remote, Direction::RemoteToLocal),
    ] {
        value.current_operation = Some(drivesync_engine::SynchronizationOperation {
            id: "move".into(),
            item_id: drivesync_engine::models::ItemId("file".into()),
            display_name: Some("fixture.bin".into()),
            newly_observed_remote: false,
            newly_observed_local: false,
            updates_local_content: false,
            updates_remote_content: false,
            rename: None,
            trash: None,
            workspace_shortcut: None,
            directory_creation: None,
            movement: Some(SynchronizationMove {
                origin,
                source_parent: "Origem".into(),
                destination_parent: "Projetos/Destino".into(),
            }),
        });
        for (activity, phase) in [
            (SynchronizationActivity::Moving, Phase::Running),
            (SynchronizationActivity::Applying, Phase::Confirmed),
        ] {
            value.activity = activity;
            let view = map_status(&value, false).activity.unwrap();
            assert_eq!(view.direction, direction);
            assert_eq!(view.name, "fixture.bin");
            assert_eq!(view.phase, phase);
            assert_eq!(view.progress, ProgressView::None);
            assert_eq!(
                view.kind,
                ActivityKind::Move {
                    source_parent: "Origem".into(),
                    destination_parent: "Projetos/Destino".into(),
                }
            );
        }
    }
}

#[test]
fn directory_creation_reports_real_phase_without_transfer_progress() {
    use drivesync_engine::*;

    for (destination, direction) in [
        (
            SynchronizationDirectoryDestination::Local,
            Direction::RemoteToLocal,
        ),
        (
            SynchronizationDirectoryDestination::Remote,
            Direction::LocalToRemote,
        ),
    ] {
        for (activity, phase) in [
            (SynchronizationActivity::CreatingDirectory, Phase::Running),
            (SynchronizationActivity::Applying, Phase::Confirmed),
        ] {
            let item_id = models::ItemId("empty-folder".into());
            let status = SynchronizationStatus {
                account_id: AccountId("fixture-a".into()),
                state: SynchronizationState::Running,
                activity,
                current_item: Some(SynchronizationItem {
                    display_name: None,
                    item_id: Some(item_id.clone()),
                    local_path: None,
                    remote_id: None,
                }),
                current_operation: Some(SynchronizationOperation {
                    id: "create-folder".into(),
                    item_id,
                    display_name: Some("Pasta vazia".into()),
                    newly_observed_remote: false,
                    newly_observed_local: false,
                    updates_local_content: false,
                    updates_remote_content: false,
                    rename: None,
                    movement: None,
                    trash: None,
                    workspace_shortcut: None,
                    directory_creation: Some(destination),
                }),
                progress: Some(SynchronizationProgress {
                    completed: 1,
                    total: 2,
                }),
                verification: None,
                issue: None,
                updated_at: Timestamp { unix_millis: 0 },
            };
            let view = map_status(&status, false).activity.unwrap();
            assert_eq!(view.kind, ActivityKind::CreateDirectory);
            assert_eq!(view.name, "Pasta vazia");
            assert_eq!(view.direction, direction);
            assert_eq!(view.phase, phase);
            assert_eq!(view.progress, ProgressView::None);
        }
    }
}

#[test]
#[ignore = "requires authorized Google account, new isolated event1 root and GTK display"]
fn real_event1_download_uses_public_backend_and_components() {
    init_fixture_locale();
    adw::init().unwrap();
    gtk::glib::MainContext::default().block_on(async {
        let env=|key:&str|std::env::var(key).expect(key);
        let local=std::path::PathBuf::from(env("DRIVESYNC_STAGE12_LOCAL_ROOT"));
        assert!(local.is_absolute() && local.file_name().unwrap().to_str().unwrap().starts_with("DriveSync-stage12-event1-"));
        assert!(!local.exists(),"each download acceptance requires a fresh empty root");
        std::fs::create_dir(&local).unwrap();
        let engine=drivesync_engine::Engine::open(drivesync_engine::EngineConfig {
            data_directory:env("DRIVESYNC_REAL_ACCOUNTS_DIRECTORY").into(), application_id:"com.rtosta.DriveSync.Devel".into(), google_oauth_file:Some(env("DRIVESYNC_REAL_OAUTH_FILE").into()),
        }).await.unwrap();
        let accounts=engine.list_accounts(drivesync_engine::ListAccountsRequest {provider:None}).await.unwrap();
        let matching=accounts.accounts.into_iter().filter(|a|a.email.as_deref()==Some("rtosta.coc@gmail.com")).collect::<Vec<_>>();
        assert_eq!(matching.len(),1); let account=matching[0].id.clone();
        engine.start_synchronization(drivesync_engine::StartSynchronizationRequest {account_id:account.clone(),local_root:local.clone(),remote_root_id:drivesync_engine::models::RemoteId(env("DRIVESYNC_STAGE12_REMOTE_ROOT"))}).await.unwrap();
        let controller=DashboardController::new();
        let window=gtk::Window::builder().default_width(640).default_height(420).child(&controller.widget).build(); window.present();
        controller.select(account.0.clone(),Box::new(EngineAdapter::new(engine.clone(),account.clone())));
        let mut observed_operation = None;
        let mut saw_download=false; let mut completed=false;
        for _ in 0..1800 {
            if let Some(activity)=controller.activity.state() {
                if matches!(activity.kind, ActivityKind::Download | ActivityKind::DownloadDiscoveredRemote) {
                    assert_eq!(activity.direction,Direction::RemoteToLocal); assert_eq!(activity.progress,ProgressView::Indeterminate);
                    assert!(activity.operation_id.is_some()); assert_eq!(activity.name,"Relatório-evento-1.bin");
                    if !saw_download { println!("real GUI download: {activity:?}");
                        observed_operation = activity.operation_id.clone();
                        save_window(&window, "real-download.png"); }
                    saw_download=true;
                }
            }
            if controller.status.state()==StatusView::Synced {completed=true;break;}
            gtk::glib::timeout_future(std::time::Duration::from_millis(100)).await;
        }
        println!("real GUI event1: download_visible={saw_download}, audit_gated_completion={completed}, final_status={:?}",controller.status.state());
        save_window(&window, "real-completed.png");
        if let Ok(output) = std::env::var("DRIVESYNC_STAGE12_EVIDENCE") {
            std::fs::write(std::path::Path::new(&output).join("gui-event1.json"), serde_json::to_vec_pretty(&serde_json::json!({"scope":account.0,"operation_id":observed_operation,"download_visible":saw_download,"audit_gated_completion":completed,"environment":"GNOME SDK 50 GTK Broadway; not installed Flatpak"})).unwrap()).unwrap();
        }
        engine.stop_synchronization(account).await.unwrap(); controller.reset(); window.close();
        assert!(completed,"positive public audit must precede completion");
        assert!(saw_download,"coalescing may omit activity; that is not GUI event acceptance");
        assert_eq!(std::fs::metadata(local.join("Relatório-evento-1.bin")).unwrap().len(),16777216);
    });
}

fn save_window(window: &gtk::Window, name: &str) {
    let Ok(output) = std::env::var("DRIVESYNC_STAGE12_EVIDENCE") else {
        return;
    };
    let child = window.child().unwrap();
    child.allocate(window.width(), window.height(), -1, None);
    let snapshot = gtk::Snapshot::new();
    window.snapshot_child(&child, &snapshot);
    let content = snapshot.to_node().expect("fixture must render");
    let background = gtk::Snapshot::new();
    background.append_color(
        &gtk::gdk::RGBA::new(0.98, 0.98, 0.98, 1.0),
        &gtk::graphene::Rect::new(0.0, 0.0, window.width() as f32, window.height() as f32),
    );
    background.append_node(&content);
    let node = background.to_node().unwrap();
    window
        .renderer()
        .unwrap()
        .render_texture(&node, None)
        .save_to_png(std::path::Path::new(&output).join(name))
        .unwrap();
}

fn init_fixture_locale() {
    if let Ok(locale) = std::env::var("DRIVESYNC_STAGE12_TEST_LOCALE") {
        unsafe {
            gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "");
        }
        gettextrs::bindtextdomain("drivesync", locale).unwrap();
        gettextrs::textdomain("drivesync").unwrap();
        assert_eq!(gettextrs::gettext("Synchronizing…"), "Sincronizando…");
    }
}

#[test]
fn trash_activity_and_attention_follow_public_evidence() {
    use drivesync_engine::{
        models::{AccountId, ItemId},
        SynchronizationActivity as A, SynchronizationIssue, SynchronizationIssueKind as I,
        SynchronizationItem, SynchronizationOperation, SynchronizationState as S,
        SynchronizationStatus, SynchronizationTrashDestination as D, Timestamp,
    };
    let mut status = SynchronizationStatus {
        account_id: AccountId("account".into()),
        state: S::Running,
        activity: A::TrashingLocal,
        current_item: Some(SynchronizationItem {
            display_name: None,
            item_id: Some(ItemId("item".into())),
            local_path: Some("fixture.bin".into()),
            remote_id: None,
        }),
        current_operation: Some(SynchronizationOperation {
            id: "trash".into(),
            item_id: ItemId("item".into()),
            display_name: Some("fixture.bin".into()),
            newly_observed_remote: false,
            newly_observed_local: false,
            updates_local_content: false,
            updates_remote_content: false,
            rename: None,
            movement: None,
            workspace_shortcut: None,
            directory_creation: None,
            trash: Some(D::Local),
        }),
        progress: None,
        verification: None,
        issue: None,
        updated_at: Timestamp { unix_millis: 0 },
    };
    let running = super::backend::map_status(&status, false).activity.unwrap();
    assert_eq!(running.kind, ActivityKind::Trash);
    assert_eq!(running.direction, Direction::RemoteToLocal);
    assert_eq!(running.phase, Phase::Running);
    assert_eq!(running.progress, ProgressView::None);
    status.activity = A::Applying;
    status.current_operation.as_mut().unwrap().trash = Some(D::Remote);
    let verified = super::backend::map_status(&status, false).activity.unwrap();
    assert_eq!(verified.phase, Phase::Confirmed);
    assert_eq!(verified.direction, Direction::LocalToRemote);
    status.state = S::Waiting;
    status.current_operation = None;
    for kind in [I::TrashRefused, I::TrashRecoveryRequired] {
        status.issue = Some(SynchronizationIssue {
            kind,
            retryable: false,
        });
        let snapshot = super::backend::map_status(&status, true);
        assert!(snapshot.activity.is_none());
        assert_eq!(snapshot.status, StatusView::Attention);
        assert_eq!(snapshot.item.as_ref().unwrap().name, "fixture.bin");
        assert!(matches!(
            snapshot.attention[0].action,
            AttentionAction::OpenIssue(_)
        ));
    }
}

#[test]
fn recovery_snapshots_only_report_the_observed_current_state() {
    use drivesync_engine::*;
    let mut status = SynchronizationStatus {
        account_id: AccountId("fixture".into()),
        state: SynchronizationState::Running,
        activity: SynchronizationActivity::Recovering,
        current_item: Some(SynchronizationItem {
            display_name: None,
            item_id: Some(models::ItemId("item".into())),
            local_path: Some("resume.bin".into()),
            remote_id: None,
        }),
        current_operation: None,
        progress: None,
        verification: None,
        issue: None,
        updated_at: Timestamp { unix_millis: 0 },
    };
    let checking = map_status(&status, true);
    assert_eq!(checking.status, StatusView::Recovering);
    let activity = checking.activity.unwrap();
    assert_eq!(activity.kind, ActivityKind::Recover);
    assert_eq!(activity.name, "resume.bin");
    assert_eq!(activity.operation_id, None);
    assert_eq!(activity.progress, ProgressView::None);
    assert!(checking.attention.is_empty());

    status.state = SynchronizationState::Waiting;
    status.activity = SynchronizationActivity::None;
    status.issue = Some(SynchronizationIssue {
        kind: SynchronizationIssueKind::RecoveryRequired,
        retryable: false,
    });
    let blocked = map_status(&status, true);
    assert_eq!(blocked.status, StatusView::Attention);
    assert!(blocked.activity.is_none());
    assert_eq!(blocked.attention.len(), 1);
    assert_eq!(blocked.item.as_ref().unwrap().name, "resume.bin");
    assert_eq!(
        blocked.attention[0].action,
        AttentionAction::OpenIssue("recovery-blocker".into())
    );
}

#[test]
fn checks_are_visible_until_audited_and_waits_explain_what_is_missing() {
    use drivesync_engine::{
        models::{AccountId, RemoteId, Timestamp},
        SynchronizationActivity, SynchronizationIssue, SynchronizationIssueKind,
        SynchronizationItem, SynchronizationState, SynchronizationStatus, SynchronizationWait,
    };
    let mut status = SynchronizationStatus {
        account_id: AccountId("account".into()),
        state: SynchronizationState::Running,
        activity: SynchronizationActivity::Analyzing,
        current_item: Some(SynchronizationItem {
            item_id: None,
            local_path: None,
            display_name: Some("Relatório.docx".into()),
            remote_id: Some(RemoteId("opaque-drive-id".into())),
        }),
        current_operation: None,
        progress: None,
        verification: None,
        issue: None,
        updated_at: Timestamp { unix_millis: 0 },
    };
    let mut idle = status.clone();
    idle.state = SynchronizationState::Idle;
    idle.activity = SynchronizationActivity::None;
    idle.current_item = None;
    assert_eq!(
        map_status(&idle, false).status,
        StatusView::VerificationIncomplete
    );
    let synced = map_status(&idle, true);
    for (activity, message) in [
        (SynchronizationActivity::Analyzing, "Analyzing changes…"),
        (SynchronizationActivity::Scanning, "Checking local files…"),
        (
            SynchronizationActivity::ScanningRemote,
            "Reading files and folders on Drive…",
        ),
        (
            SynchronizationActivity::Observing,
            "Checking local files and Drive for changes…",
        ),
    ] {
        status.activity = activity;
        let checking = map_status(&status, false);
        assert_eq!(checking.status, StatusView::Checking);
        assert!(checking.description.contains(&gettextrs::gettext(message)));
        assert!(checking.activity.is_none());
        assert!(checking.attention.is_empty());
        assert_eq!(map_status(&status, true).status, synced.status);
    }

    let mut observing = status.clone();
    observing.state = SynchronizationState::Waiting;
    observing.current_item = None;
    assert_eq!(map_status(&observing, true).status, StatusView::Synced);
    assert_eq!(map_status(&observing, false).status, StatusView::Checking);

    observing.issue = Some(SynchronizationIssue {
        kind: SynchronizationIssueKind::DeferredFor(SynchronizationWait::RemoteObservation),
        retryable: true,
    });
    let retry = map_status(&observing, true);
    assert_eq!(retry.status, StatusView::WaitingForEvidence);
    assert_eq!(
        retry.description,
        gettextrs::gettext("The Drive scan did not finish. DriveSync will try again.")
    );

    status.state = SynchronizationState::Waiting;
    status.activity = SynchronizationActivity::None;
    status.issue = Some(SynchronizationIssue {
        kind: SynchronizationIssueKind::DeferredFor(SynchronizationWait::RemoteObservation),
        retryable: true,
    });
    let waiting = map_status(&status, true);
    assert_eq!(waiting.status, StatusView::WaitingForEvidence);
    assert!(waiting.activity.is_none());
    assert_eq!(waiting.item.as_ref().unwrap().name, "Relatório.docx");
    assert!(!waiting.description.contains("Relatório.docx"));
    assert!(!waiting.active);
    assert!(waiting
        .description
        .contains(&gettextrs::gettext("Waiting for a complete scan on Drive.")));
    assert!(!waiting.description.contains("opaque-drive-id"));
    assert!(waiting.attention.is_empty());

    status.issue.as_mut().unwrap().kind =
        SynchronizationIssueKind::DeferredFor(SynchronizationWait::ItemVerification {
            source: drivesync_engine::SynchronizationVerificationSource::Remote,
            confirmed_action: Some(drivesync_engine::SynchronizationConfirmedAction::Download),
        });
    let finalizing = map_status(&status, false);
    assert_eq!(finalizing.status, StatusView::Finalizing);
    assert!(!finalizing.active);
    assert!(finalizing.activity.is_none());
    assert!(finalizing
        .description
        .contains(&gettextrs::gettext("Download confirmed.")));
    assert_eq!(finalizing.item.unwrap().name, "Relatório.docx");

    status.issue.as_mut().unwrap().kind =
        SynchronizationIssueKind::DeferredFor(SynchronizationWait::FreshObservations);
    for (local_read, remote_read, text) in [
        (
            false,
            false,
            "Checking the computer and Drive again after the last operation.",
        ),
        (true, false, "Computer scan complete. Checking Drive…"),
        (false, true, "Drive scan complete. Checking the computer…"),
        (
            true,
            true,
            "Scans complete. Checking the result of the last operation…",
        ),
    ] {
        status.verification = Some(drivesync_engine::SynchronizationVerification {
            local_read,
            remote_read,
        });
        let confirming = map_status(&status, false);
        assert_eq!(confirming.status, StatusView::Checking);
        assert_eq!(confirming.item.as_ref().unwrap().name, "Relatório.docx");
        assert!(!confirming.description.contains("Relatório.docx"));
        assert!(confirming.description.contains(&gettextrs::gettext(text)));
    }

    status.state = SynchronizationState::Running;
    status.activity = SynchronizationActivity::Applying;
    status.issue = None;
    status.verification.as_mut().unwrap().remote_read = false;
    let applying = map_status(&status, false);
    assert_eq!(applying.status, StatusView::Checking);
    assert_eq!(
        applying.description,
        gettextrs::gettext("Computer scan complete. Checking Drive…")
    );
}

#[test]
#[ignore = "requires an isolated GTK display"]
fn overview_follows_scoped_subscription_and_attention_action() {
    adw::init().unwrap();
    let controller = DashboardController::new();
    use crate::ui::components::sync_overview::SyncOverviewState as Overview;
    let snapshot = Snapshot {
        scope: "account-a".into(),
        status: StatusView::Attention,
        description: "A real issue supplied by the adapter".into(),
        item: None,
        active: false,
        activity: None,
        attention: vec![],
        overview: Some(Overview::Attention {
            pending_items: None,
        }),
    };
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let received = calls.clone();
    controller
        .status
        .connect_details(move || received.set(received.get() + 1));
    gtk::glib::MainContext::default().block_on(async {
        controller.select(
            "account-a".into(),
            Box::new(FakeSyncBackend::new([snapshot.clone()])),
        );
        gtk::glib::timeout_future(std::time::Duration::from_millis(20)).await;
        assert!(controller.overview.widget().is_visible());
        let body = controller.overview.widget().first_child().unwrap();
        body.last_child()
            .unwrap()
            .downcast::<gtk::Button>()
            .unwrap()
            .emit_clicked();
        assert_eq!(calls.get(), 1);
        controller.select("account-b".into(), Box::new(FakeSyncBackend::new([])));
        assert!(!controller.overview.widget().is_visible());
        assert!(!controller.apply(snapshot));
        assert!(!controller.overview.widget().is_visible());
        let mut snapshot = map_status(
            &drivesync_engine::SynchronizationStatus {
                account_id: drivesync_engine::AccountId("account-b".into()),
                state: drivesync_engine::SynchronizationState::Running,
                activity: drivesync_engine::SynchronizationActivity::Downloading,
                current_item: None,
                current_operation: None,
                progress: None,
                verification: None,
                issue: None,
                updated_at: drivesync_engine::Timestamp { unix_millis: 0 },
            },
            false,
        );
        assert!(controller.apply(snapshot.clone()));
        assert!(controller.overview.widget().is_visible());
        snapshot.overview = None;
        assert!(controller.apply(snapshot));
        assert!(!controller.overview.widget().is_visible());
        controller.reset();
    });
}

#[test]
#[ignore = "requires the explicitly isolated real test-account profile"]
fn overview_reads_real_engine_profile_without_starting_synchronization() {
    let directory = std::path::PathBuf::from(
        std::env::var("DRIVESYNC_REAL_ACCOUNTS_DIRECTORY")
            .expect("explicit isolated account profile required"),
    );
    assert!(directory
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("stage"));
    gtk::glib::MainContext::default().block_on(async {
        let engine = drivesync_engine::Engine::open(drivesync_engine::EngineConfig {
            data_directory: directory,
            application_id: "com.rtosta.DriveSync.Devel".into(),
            google_oauth_file: std::env::var_os("DRIVESYNC_REAL_OAUTH_FILE").map(Into::into),
        })
        .await
        .unwrap();
        let accounts = engine
            .list_accounts(drivesync_engine::ListAccountsRequest { provider: None })
            .await
            .unwrap()
            .accounts;
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].email.as_deref(), Some("rtosta.coc@gmail.com"));
        let account = accounts[0].id.clone();
        let status = engine
            .synchronization_status(account.clone())
            .await
            .unwrap();
        assert_eq!(
            status.state,
            drivesync_engine::SynchronizationState::Stopped
        );
        let snapshot = EngineAdapter::new(engine, account.clone())
            .next()
            .await
            .unwrap();
        assert_eq!(snapshot.scope, account.0);
        assert_eq!(snapshot.status, StatusView::Stopped);
        assert!(matches!(
            snapshot.overview,
            Some(crate::ui::components::sync_overview::SyncOverviewState::Inactive { .. })
        ));
    });
}

#[test]
#[ignore = "requires an isolated GTK display"]
fn overview_remains_visible_through_all_account_lifecycle_states() {
    adw::init().unwrap();
    let controller = DashboardController::new();
    controller.select("lifecycle".into(), Box::new(FakeSyncBackend::new([])));
    let status = drivesync_engine::SynchronizationStatus {
        account_id: drivesync_engine::AccountId("lifecycle".into()),
        state: drivesync_engine::SynchronizationState::Idle,
        activity: drivesync_engine::SynchronizationActivity::None,
        current_item: None,
        current_operation: None,
        progress: None,
        verification: None,
        issue: None,
        updated_at: drivesync_engine::Timestamp { unix_millis: 0 },
    };
    let mut snapshot = map_status(&status, false);
    for state in [
        StatusView::Initializing,
        StatusView::Checking,
        StatusView::Synchronizing,
        StatusView::Finalizing,
        StatusView::WaitingForEvidence,
        StatusView::VerificationIncomplete,
        StatusView::Recovering,
        StatusView::Attention,
        StatusView::Failed,
        StatusView::Stopped,
        StatusView::Offline,
        StatusView::Synced,
    ] {
        snapshot.status = state.clone();
        snapshot.overview = super::backend::overview_state(&snapshot);
        assert!(snapshot.overview.is_some(), "{state:?}");
        assert!(controller.apply(snapshot.clone()));
        assert!(controller.overview.widget().is_visible(), "{state:?}");
    }
    controller.reset();
    assert!(!controller.overview.widget().is_visible());
}
