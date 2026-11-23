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
        let window = gtk::Window::builder()
            .default_width(360)
            .default_height(500)
            .child(&controller.widget)
            .build();
        window.present();
        gtk::glib::timeout_future(std::time::Duration::from_millis(60)).await;
        save_window(&window, "fixture-dashboard-pt-br.png");
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
        controller.reset();
    });
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
        issue: None,
        updated_at: Timestamp { unix_millis: 0 },
    };
    assert_ne!(map_status(&value, false).status, StatusView::Synced);
    assert_eq!(map_status(&value, true).status, StatusView::Synced);
    value.issue = Some(SynchronizationIssue {
        kind: SynchronizationIssueKind::RecoveryRequired,
        retryable: false,
    });
    assert_ne!(map_status(&value, true).status, StatusView::Synced);
    value.issue = None;
    value.state = SynchronizationState::Running;
    value.activity = SynchronizationActivity::Downloading;
    value.current_item = Some(SynchronizationItem {
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
    });
    value.progress = Some(SynchronizationProgress {
        completed: 1,
        total: 2,
    });
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
        assert_eq!(current.progress, ProgressView::Indeterminate);
    }
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

    value.activity = SynchronizationActivity::Uploading;
    let operation = value.current_operation.as_mut().unwrap();
    operation.newly_observed_remote = false;
    operation.newly_observed_local = true;
    let discovery = map_status(&value, false).activity.unwrap();
    assert_eq!(discovery.kind, ActivityKind::UploadDiscoveredLocal);
    assert_eq!(discovery.operation_id.as_deref(), Some("execution-1"));
    assert_eq!(discovery.direction, Direction::LocalToRemote);
    assert_eq!(discovery.progress, ProgressView::Indeterminate);
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
