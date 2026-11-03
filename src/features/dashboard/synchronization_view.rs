//! Automatic and explicit downloads observing application-owned account jobs.
use adw::{gtk, prelude::*};
#[cfg(test)]
use drivesync_engine::{DownloadProgress, DownloadStage};
use drivesync_engine::{DownloadReport, DriveSyncEngine, EngineError, RestoredAccount};
#[cfg(test)]
use std::{cell::Cell, rc::Rc};
use std::{sync::Arc, time::Duration};

/// Weak handles let navigation dispose account pages while cancellation propagates.
#[derive(Clone)]
struct Widgets {
    card: Option<crate::features::synchronization::components::progress_card::SyncProgressCard>,
    attention: Option<super::components::attention_summary::AttentionSummary>,
    start: gtk::glib::WeakRef<gtk::Button>,
    header: super::components::account_header::AccountHeader,
    folders: gtk::glib::WeakRef<gtk::Box>,
    account_row: gtk::glib::WeakRef<adw::ActionRow>,
    issues: gtk::glib::WeakRef<gtk::Box>,
    activity: gtk::glib::WeakRef<gtk::Box>,
    skipped: gtk::glib::WeakRef<gtk::Label>,
    preserved: gtk::glib::WeakRef<gtk::Label>,
}
impl Widgets {
    fn clear_issues(&self) {
        if let Some(summary) = &self.attention {
            summary.clear();
        }
        if let Some(container) = self.issues.upgrade() {
            while let Some(child) = container.first_child() {
                container.remove(&child);
            }
            container.set_visible(false);
        }
    }
    fn show_issues(&self, report: &DownloadReport) {
        let attention = super::presentation::Attention::from_report(report);
        let title = if attention.any() {
            format!(
                "Needs attention · {} conflicts · {} problems",
                attention.conflicts, attention.problems
            )
        } else {
            "Needs attention".into()
        };
        self.show_issues_title(report, &title);
    }
    fn show_issues_title(&self, report: &DownloadReport, title: &str) {
        self.clear_issues();
        if let Some(summary) = &self.attention {
            summary.present(report);
        }
        if let Some(container) = self.issues.upgrade() {
            let group = adw::PreferencesGroup::builder().title(title).build();
            let attention = report.attention();
            let root_problem = report
                .issues
                .iter()
                .any(|issue| matches!(issue.error, EngineError::RootSuspended(_)));
            for issue in attention.iter().take(5) {
                let path = issue.relative_path.as_deref();
                let name = path
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| {
                        if root_problem {
                            "Synchronization folders".into()
                        } else {
                            "Item".into()
                        }
                    });
                let row = adw::ActionRow::builder()
                    .title(&name)
                    .subtitle(&issue.message)
                    .use_markup(false)
                    .build();
                row.set_title_lines(0);
                row.set_subtitle_lines(if root_problem && path.is_none() { 0 } else { 3 });
                row.set_tooltip_text(Some(&format!(
                    "{}\n{}",
                    path.map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "Item".into()),
                    issue.message
                )));
                let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
                icon.add_css_class("warning");
                row.add_prefix(&icon);
                group.add(&row);
            }
            if attention.len() > 5 {
                group.add(
                    &adw::ActionRow::builder()
                        .title(format!("{} more items need attention", attention.len() - 5))
                        .build(),
                );
            }
            container.append(&group);
            container.set_visible(!attention.is_empty());
        }
    }
    fn show_activity(&self, report: &DownloadReport) {
        if let Some(container) = self.activity.upgrade() {
            super::components::recent_activity::present(&container, report);
        }
    }
    fn begin(&self) {
        if let Some(button) = self.start.upgrade() {
            button.set_label("Pause");
            button.set_sensitive(true);
            button.remove_css_class("suggested-action");
        }
        if let Some(folders) = self.folders.upgrade() {
            folders.set_sensitive(false);
        }

        if let Some(preserved) = self.preserved.upgrade() {
            preserved.set_visible(false);
        }
        if let Some(skipped) = self.skipped.upgrade() {
            skipped.set_visible(false);
        }
    }
    /// A single feedback card shows live work or the last result. Review cards
    /// remain hidden during checks and transfers without discarding observations.
    fn present_job(
        &self,
        snapshot: &crate::features::synchronization::automatic::Snapshot,
        previous: Option<crate::features::synchronization::automatic::Phase>,
    ) {
        self.present_state(snapshot, previous);
        let processing =
            snapshot.phase == crate::features::synchronization::automatic::Phase::Running;
        if let Some(summary) = &self.attention {
            summary.set_processing(processing);
        }
        if processing {
            if let Some(issues) = self.issues.upgrade() {
                issues.set_visible(false);
            }
        }
        if let Some(card) = &self.card {
            card.present(snapshot);
        }
    }
    fn present_state(
        &self,
        snapshot: &crate::features::synchronization::automatic::Snapshot,
        previous: Option<crate::features::synchronization::automatic::Phase>,
    ) {
        use crate::features::synchronization::automatic::Phase;
        self.header.present(snapshot);
        let sync_status = super::presentation::status(snapshot);
        if let Some(row) = self.account_row.upgrade() {
            row.set_subtitle(sync_status);
        }
        if snapshot.phase == Phase::Running {
            if previous != Some(Phase::Running) {
                self.begin();
            }
            if snapshot.pausing {
                if let Some(button) = self.start.upgrade() {
                    button.set_label("Pausing…");
                    button.set_sensitive(false);
                }
            }
            return;
        }
        if let Some(result) = &snapshot.result {
            self.finish(result);
        } else if previous.is_some_and(|phase| phase != snapshot.phase) {
            self.idle();
            if matches!(snapshot.phase, Phase::Waiting | Phase::Unconfigured) {
                // Reconfiguration discards the previous result in the controller.
                // Do not keep presenting that result as a failure of the new scope.
                self.clear_issues();
                if let Some(activity) = self.activity.upgrade() {
                    activity.set_visible(false);
                }
                if let Some(skipped) = self.skipped.upgrade() {
                    skipped.set_visible(false);
                }
                if let Some(preserved) = self.preserved.upgrade() {
                    preserved.set_visible(false);
                }
            }
            if snapshot.phase == Phase::Unconfigured {
                if let Some(button) = self.start.upgrade() {
                    button.set_sensitive(false);
                }
            }
        }
        if snapshot.phase == Phase::Paused {
            if let Some(button) = self.start.upgrade() {
                button.set_label("Resume");
            }
        }
    }
    fn idle(&self) {
        if let Some(start) = self.start.upgrade() {
            start.set_label("Synchronize Now");
            start.add_css_class("suggested-action");
            start.set_sensitive(true);
        }
        if let Some(folders) = self.folders.upgrade() {
            folders.set_sensitive(true);
        }
    }
    fn finish(&self, result: &Result<DownloadReport, EngineError>) {
        self.idle();
        match result {
            Ok(report) => {
                if let Some(preserved) = self.preserved.upgrade() {
                    preserved.set_label(&preserved_text(report));
                    preserved.set_visible(
                        !report.preserved_copies.is_empty() || !report.conflict_copies.is_empty(),
                    );
                }
                self.show_issues(report);
                self.show_activity(report);
                if let Some(skipped) = self.skipped.upgrade() {
                    skipped.set_label(&if report.skipped == 1 {
                        "1 item skipped: unsupported type or download restriction.".into()
                    } else {
                        format!(
                            "{} items skipped: unsupported types or download restrictions.",
                            report.skipped
                        )
                    });
                    skipped.set_visible(report.skipped > 0);
                }
            }
            Err(error) => {
                if error == &EngineError::InvalidFolder {
                    if let Some(start) = self.start.upgrade() {
                        start.set_sensitive(false);
                    }
                }
                let cancelled = matches!(
                    error,
                    EngineError::Authentication(drivesync_engine::AuthError::Cancelled)
                );
                let report = DownloadReport {
                    issues: if cancelled {
                        vec![]
                    } else {
                        vec![drivesync_engine::DownloadIssue {
                            relative_path: None,
                            error: error.clone(),
                        }]
                    },
                    ..Default::default()
                };
                self.show_issues(&report);
                if let Some(activity) = self.activity.upgrade() {
                    activity.set_visible(false);
                }
            }
        }
    }
}
/// Shows bounded relative paths without implying old versions were deleted.
fn preserved_text(report: &DownloadReport) -> String {
    let total = report.preserved_copies.len() + report.conflict_copies.len();
    if total == 0 {
        return String::new();
    }
    let mut text = String::from("Recoverable copies kept beside the original files:\n");
    let paths = report
        .preserved_copies
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .chain(report.conflict_copies.iter().map(|copy| {
            format!(
                "{} (Drive version {} of {})",
                copy.copy_path.display(),
                copy.remote_version,
                copy.original_path.display()
            )
        }));
    text.push_str(&paths.take(3).collect::<Vec<_>>().join("\n"));
    if total > 3 {
        text.push_str(&format!("\n{} additional copies kept.", total - 3));
    }
    text
}
/// Existing account controls observed by the synchronization footer.
pub(super) struct Controls<'a> {
    pub folders: &'a gtk::Box,
    pub start: &'a gtk::Button,
    pub row: &'a adw::ActionRow,
    pub status: &'a gtk::Label,
    pub header: &'a super::components::account_header::AccountHeader,
    pub account_row: &'a adw::ActionRow,
    pub recovery: Option<&'a gtk::Button>,
}

/// Observes the shared account job. Page disposal stops observation only; the
/// application-owned worker continues while hidden or while pages are rebuilt.
#[cfg(test)]
pub(super) fn build(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    controls: Controls<'_>,
) -> gtk::Box {
    let job =
        crate::features::synchronization::automatic::Controller::attach(engine.clone(), saved);
    build_for_job(engine, saved, controls, job)
}

/// Uses the same controller as both account menus, including isolated fixtures.
pub(super) fn build_for_job(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    controls: Controls<'_>,
    job: crate::features::synchronization::automatic::Controller,
) -> gtk::Box {
    let Controls {
        folders,
        start,
        row,
        status,
        header,
        account_row,
        recovery,
    } = controls;
    let footer = gtk::Box::new(gtk::Orientation::Vertical, 12);
    // Folder editors still own these feedback controls. Synchronization feedback
    // is presented by the card, activity and issue components below.
    row.set_visible(false);
    status.set_visible(false);
    footer.append(row);
    footer.append(status);
    let card = crate::features::synchronization::components::progress_card::SyncProgressCard::new();
    card.widget().set_tooltip_text(Some("Automatic checks wait while synchronization is paused or blocked. Up-to-date files are not transferred again. Existing local edits and previous copies are preserved. Cycle progress counts processed files and folders, including skipped items and conflicts; it does not represent transferred bytes."));
    footer.prepend(card.widget());
    let skipped = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .visible(false)
        .css_classes(["dim-label"])
        .build();
    skipped.set_widget_name("download-skipped");
    footer.append(&skipped);
    let preserved = gtk::Label::builder()
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .xalign(0.0)
        .visible(false)
        .css_classes(["dim-label"])
        .build();
    preserved.set_widget_name("download-preserved");
    footer.append(&preserved);
    let issues = gtk::Box::new(gtk::Orientation::Vertical, 0);
    issues.set_visible(false);
    issues.set_widget_name("download-issues");
    footer.append(&issues);
    let activity = gtk::Box::new(gtk::Orientation::Vertical, 0);
    activity.set_visible(false);
    activity.set_widget_name("download-history");
    footer.append(&activity);
    // Visible synchronization actions belong to the header and account menus.
    start.set_visible(false);
    footer.append(start);
    let recovery = recovery.cloned().unwrap_or_else(|| {
        super::dialogs::state_recovery::button(engine.clone(), saved, status, job.clone())
    });
    recovery.set_visible(false);
    footer.append(&recovery);
    let conflicts = crate::features::conflicts::button_for(
        engine.clone(),
        saved,
        job.clone(),
        crate::features::conflicts::ReviewKind::Conflicts,
    );
    let problems = crate::features::conflicts::button_for(
        engine.clone(),
        saved,
        job.clone(),
        crate::features::conflicts::ReviewKind::Problems,
    );
    let attention =
        super::components::attention_summary::AttentionSummary::new(&conflicts, &problems);
    footer.insert_child_after(attention.widget(), Some(card.widget()));
    let ui = Widgets {
        card: Some(card),
        attention: Some(attention),
        start: start.downgrade(),
        header: header.clone(),
        folders: folders.downgrade(),
        account_row: account_row.downgrade(),
        issues: issues.downgrade(),
        activity: activity.downgrade(),
        skipped: skipped.downgrade(),
        preserved: preserved.downgrade(),
    };
    let restore = if saved.folder.is_some() {
        let (send, receive) = std::sync::mpsc::channel();
        let engine = engine.clone();
        let id = saved.account.id.clone();
        std::thread::spawn(move || {
            let _ = send.send(engine.saved_reconciliation(&id));
        });
        Some(receive)
    } else {
        None
    };
    footer.append(&super::dialogs::bulk_review::button(job.clone()));
    if let Some(receive) = restore {
        let restore_ui = ui.clone();
        let restore_job = job.clone();
        let folder = saved.folder.clone();
        gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
            if restore_ui.start.upgrade().is_none() || restore_job.folder() != folder {
                return gtk::glib::ControlFlow::Break;
            }
            match receive.try_recv() {
                Ok(Ok(entries)) => {
                    let snapshot = restore_job.snapshot();
                    if snapshot.result.is_none()
                        && snapshot.progress.is_none()
                        && !restore_job.running()
                    {
                        restore_ui.show_issues_title(
                            &DownloadReport {
                                reconciliation: entries,
                                ..Default::default()
                            },
                            "Previous scan — awaiting a new check",
                        );
                    }
                    gtk::glib::ControlFlow::Break
                }
                Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    gtk::glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
            }
        });
    }
    {
        let button = start;
        let click_job = job.clone();
        let click_ui = ui.clone();
        button.connect_clicked(move |_| {
            let previous = click_job.snapshot().phase;
            if click_job.running() {
                click_job.pause();
            } else {
                click_job.request();
            }
            click_ui.present_job(&click_job.snapshot(), Some(previous));
        });
    }
    let click_job = job.clone();
    header.connect_toggle(move || {
        if click_job.running() {
            click_job.pause();
        } else {
            click_job.request();
        }
    });
    let initial = job.snapshot();
    ui.present_job(&initial, None);
    let mut revision = initial.revision;
    let mut phase = Some(initial.phase);
    gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
        if ui.start.upgrade().is_none() {
            return gtk::glib::ControlFlow::Break;
        }
        if job.revision() != revision {
            let snapshot = job.snapshot();
            ui.present_job(&snapshot, phase);
            revision = snapshot.revision;
            phase = Some(snapshot.phase);
        }
        gtk::glib::ControlFlow::Continue
    });
    footer
}
/// Exercises the worker failure path without a configured root, vault or network.
#[cfg(test)]
pub(super) fn verify_worker_failure() {
    use drivesync_engine::{AccountId, AccountSummary};
    let saved = RestoredAccount {
        account: AccountSummary {
            id: AccountId("google:download-ui-fixture".into()),
            provider_subject: "download-ui-fixture".into(),
            email: None,
        },
        error: None,
        folder: Some(drivesync_engine::FolderConfiguration {
            account_id: AccountId("google:download-ui-fixture".into()),
            remote: drivesync_engine::DriveFolder {
                id: "fixture".into(),
                name: "Fixture".into(),
            },
            local: "/tmp/drivesync-ui-missing".into(),
            local_display: "/tmp/drivesync-ui-missing".into(),
        }),
    };
    let folders = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let start = gtk::Button::with_label("Synchronize Now");
    let row = adw::ActionRow::new();
    let account_row = adw::ActionRow::new();
    let status = gtk::Label::new(None);
    let header = super::components::account_header::AccountHeader::new("Fixture", "Connected");

    let footer = build(
        Arc::new(DriveSyncEngine::new()),
        &saved,
        Controls {
            folders: &folders,
            start: &start,
            row: &row,
            status: &status,
            header: &header,
            account_row: &account_row,
            recovery: None,
        },
    );
    start.emit_clicked();
    assert_eq!(start.label().as_deref(), Some("Pause"));
    assert!(!folders.is_sensitive());

    let wait = gtk::glib::MainLoop::new(None, false);
    let finish = wait.clone();
    let weak_start = start.downgrade();
    let count = Rc::new(Cell::new(0));
    gtk::glib::timeout_add_local(Duration::from_millis(25), move || {
        count.set(count.get() + 1);
        if weak_start
            .upgrade()
            .is_some_and(|b| b.label().as_deref() == Some("Synchronize Now"))
            || count.get() > 100
        {
            finish.quit();
            gtk::glib::ControlFlow::Break
        } else {
            gtk::glib::ControlFlow::Continue
        }
    });
    wait.run();
    assert_eq!(account_row.subtitle().as_deref(), Some("Needs attention"));
    assert!(!start.is_sensitive());
    assert!(folders.is_sensitive());

    assert!(footer.first_child().is_some());
}

/// Verifies real native controls in active, verification and completion states.
#[cfg(test)]
pub(super) fn verify_progress_presentations(mut capture: impl FnMut(&adw::Window, &str)) {
    use crate::features::synchronization::automatic::{Phase, Snapshot};
    use drivesync_engine::{AccountId, AccountSummary, DownloadItemProgress};
    for width in [720, 360] {
        let saved = RestoredAccount {
            account: AccountSummary {
                id: AccountId("google:progress-fixture".into()),
                provider_subject: "progress-fixture".into(),
                email: None,
            },
            error: None,
            folder: None,
        };
        let start = gtk::Button::new();
        let folders = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let row = adw::ActionRow::new();
        let account_row = adw::ActionRow::new();
        let status = gtk::Label::new(None);
        let header = super::components::account_header::AccountHeader::new("Fixture", "Connected");
        let account_status = header.status_for_test();
        let footer = build(
            Arc::new(DriveSyncEngine::new()),
            &saved,
            Controls {
                folders: &folders,
                start: &start,
                row: &row,
                status: &status,
                header: &header,
                account_row: &account_row,
                recovery: None,
            },
        );
        fn named(widget: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
            if widget.widget_name() == name {
                return Some(widget.clone());
            }
            let mut child = widget.first_child();
            while let Some(current) = child {
                if let Some(found) = named(&current, name) {
                    return Some(found);
                }
                child = current.next_sibling();
            }
            None
        }
        assert!(named(footer.upcast_ref(), "sync-details").is_none());
        assert!(named(footer.upcast_ref(), "download-progress").is_none());
        assert!(named(footer.upcast_ref(), "download-activity").is_none());
        assert!(!row.is_visible());
        let issues = named(footer.upcast_ref(), "download-issues")
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        let activity = named(footer.upcast_ref(), "download-history")
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        let skipped = named(footer.upcast_ref(), "download-skipped")
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();
        let preserved = named(footer.upcast_ref(), "download-preserved")
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();
        let card =
            crate::features::synchronization::components::progress_card::SyncProgressCard::new();
        footer.remove(&footer.first_child().unwrap());
        footer.prepend(card.widget());
        let attention = super::components::attention_summary::AttentionSummary::new(
            &gtk::Button::new(),
            &gtk::Button::new(),
        );
        footer.remove(&named(footer.upcast_ref(), "attention-summary").unwrap());
        footer.insert_child_after(attention.widget(), Some(card.widget()));
        let ui = Widgets {
            card: Some(card),
            attention: Some(attention),
            start: start.downgrade(),
            header,
            folders: folders.downgrade(),
            account_row: account_row.downgrade(),
            issues: issues.downgrade(),
            activity: activity.downgrade(),
            skipped: skipped.downgrade(),
            preserved: preserved.downgrade(),
        };
        let content = crate::ui::content();
        content.append(&footer);
        let window = adw::Window::builder()
            .default_width(width)
            .default_height(620)
            .content(&crate::ui::scroll(&content, 760))
            .build();
        window.present();
        let settle = || {
            let wait = gtk::glib::MainLoop::new(None, false);
            let finish = wait.clone();
            gtk::glib::timeout_add_local_once(Duration::from_millis(100), move || finish.quit());
            wait.run();
        };
        let mut snapshot = Snapshot {
            revision: 1,
            phase: Phase::Running,
            pausing: false,
            progress: Some(DownloadProgress {
                preparation: None,
                completed_items: 7,
                total_items: 20,
                current: Some(DownloadItemProgress {
                    relative_path: "Photos/Example.jpg".into(),
                    stage: DownloadStage::Downloading,
                    received: 2_500_000,
                    total: Some(3_400_000),
                }),
            }),
            result: None,
        };
        ui.present_job(&snapshot, None);
        let bar = named(footer.upcast_ref(), "sync-card-progress")
            .unwrap()
            .downcast::<gtk::ProgressBar>()
            .unwrap();
        assert_eq!(bar.fraction(), 0.35);
        assert!(!issues.is_visible());
        let path = named(footer.upcast_ref(), "sync-current-path")
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();
        assert_eq!(path.label(), "Photos/Example.jpg");
        assert!(path.is_visible());
        snapshot.phase = Phase::Paused;
        ui.present_job(&snapshot, Some(Phase::Running));
        assert_eq!(bar.fraction(), 0.35);
        assert!(path.is_visible());
        snapshot.phase = Phase::Running;
        ui.present_job(&snapshot, Some(Phase::Paused));
        settle();
        capture(&window, &format!("download-progress-{width}"));
        snapshot.progress.as_mut().unwrap().preparation =
            Some(drivesync_engine::PreparationProgress::Scanning {
                files: 320,
                folders: 42,
                current_path: "Documentos/Projetos".into(),
            });
        ui.present_job(&snapshot, Some(Phase::Running));
        assert!(!bar.is_visible());
        assert_eq!(path.label(), "Documentos/Projetos");
        settle();
        capture(&window, &format!("download-analysis-{width}"));
        snapshot.phase = Phase::Attention;
        snapshot.progress = None;
        snapshot.result = Some(Ok(DownloadReport {
            updated: 2,
            recovered: 1,
            created_remote_folders: 3,
            linked: 1,
            skipped: 1,
            preserved_copies: vec!["Photos/DriveSync backup example.bak".into()],
            issues: vec![drivesync_engine::DownloadIssue {
                relative_path: Some("Photos/Example.jpg".into()),
                error: EngineError::DownloadConflict,
            }],
            ..Default::default()
        }));
        ui.present_job(&snapshot, Some(Phase::Running));
        assert!(issues.is_visible());
        assert!(activity.is_visible());
        assert!(skipped.is_visible());
        assert!(preserved.is_visible());
        assert!(!path.is_visible());
        assert!(!bar.is_visible());
        settle();
        capture(&window, &format!("download-result-{width}"));
        for reason in [
            drivesync_engine::RootSuspension::LocalUnavailable,
            drivesync_engine::RootSuspension::RemoteAccessDenied,
        ] {
            snapshot.phase = Phase::Blocked;
            snapshot.result = Some(Err(EngineError::RootSuspended(reason)));
            ui.present_job(&snapshot, Some(Phase::Attention));
            assert!(issues.is_visible());
            assert_eq!(account_status.label(), "Folder unavailable");
            assert!(!activity.is_visible());
            settle();
            capture(&window, &format!("root-unavailable-{width}"));
        }
        assert!(window.width() <= width + 10);
        window.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an isolated GTK display"]
    fn reconfigured_waiting_clears_the_previous_cycle_error() {
        adw::init().unwrap();
        let start = gtk::Button::new();
        let issues = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let activity = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let skipped = gtk::Label::new(None);
        let preserved = gtk::Label::new(None);
        let review = gtk::Button::new();
        let attention =
            crate::features::dashboard::components::attention_summary::AttentionSummary::new(
                &review,
                &gtk::Button::new(),
            );
        let ui = Widgets {
            card: Some(
                crate::features::synchronization::components::progress_card::SyncProgressCard::new(
                ),
            ),
            attention: Some(attention.clone()),
            start: start.downgrade(),
            header: crate::features::dashboard::components::account_header::AccountHeader::new(
                "Fixture",
                "Connected",
            ),
            folders: Default::default(),

            account_row: Default::default(),
            issues: issues.downgrade(),
            activity: activity.downgrade(),
            skipped: skipped.downgrade(),
            preserved: preserved.downgrade(),
        };
        for previous in [
            crate::features::synchronization::automatic::Phase::Offline,
            crate::features::synchronization::automatic::Phase::Blocked,
            crate::features::synchronization::automatic::Phase::Running,
        ] {
            ui.present_job(
                &crate::features::synchronization::automatic::Snapshot {
                    revision: 1,
                    pausing: false,
                    phase: previous,
                    progress: None,
                    result: Some(Err(EngineError::RootSuspended(
                        drivesync_engine::RootSuspension::Offline,
                    ))),
                },
                None,
            );
            if previous != crate::features::synchronization::automatic::Phase::Running {
                assert!(issues.is_visible());
            }
            activity.set_visible(true);
            skipped.set_visible(true);
            preserved.set_visible(true);
            ui.present_job(
                &crate::features::synchronization::automatic::Snapshot {
                    revision: 2,
                    pausing: false,
                    phase: crate::features::synchronization::automatic::Phase::Waiting,
                    progress: None,
                    result: None,
                },
                Some(previous),
            );
            assert!(!issues.is_visible());
            assert!(issues.first_child().is_none());
            assert!(!activity.is_visible());
            assert!(!skipped.is_visible());
            assert!(!preserved.is_visible());
        }
        ui.present_job(
            &crate::features::synchronization::automatic::Snapshot {
                revision: 3,
                pausing: false,
                phase: crate::features::synchronization::automatic::Phase::UpToDate,
                progress: None,
                result: None,
            },
            Some(crate::features::synchronization::automatic::Phase::Waiting),
        );
        let mut snapshot = crate::features::synchronization::automatic::Snapshot {
            revision: 3,
            pausing: false,
            phase: crate::features::synchronization::automatic::Phase::Attention,
            progress: None,
            result: Some(Ok(DownloadReport {
                issues: vec![drivesync_engine::DownloadIssue {
                    relative_path: Some("pending.txt".into()),
                    error: EngineError::DownloadConflict,
                }],
                ..Default::default()
            })),
        };
        ui.present_job(
            &snapshot,
            Some(crate::features::synchronization::automatic::Phase::Waiting),
        );
        assert!(attention.widget().is_visible());
        assert!(issues.is_visible());
        let pending_result = snapshot.result.clone();
        snapshot.phase = crate::features::synchronization::automatic::Phase::Running;
        snapshot.result = None;
        ui.present_job(
            &snapshot,
            Some(crate::features::synchronization::automatic::Phase::Attention),
        );
        assert!(
            !attention.widget().is_visible(),
            "checks and transfers hide previous observations until the cycle stops"
        );
        assert!(!issues.is_visible());
        for stage in [
            drivesync_engine::PreparationProgress::ReadingChanges,
            drivesync_engine::PreparationProgress::CheckingLocal { checked: 7 },
        ] {
            snapshot.progress = Some(DownloadProgress {
                preparation: Some(stage),
                completed_items: 0,
                total_items: 0,
                current: None,
            });
            ui.present_job(&snapshot, Some(snapshot.phase));
            assert!(!attention.widget().is_visible());
            assert!(!issues.is_visible());
            assert!(ui.card.as_ref().unwrap().widget().is_visible());
        }
        snapshot.phase = crate::features::synchronization::automatic::Phase::Attention;
        snapshot.progress = None;
        snapshot.result = pending_result;
        ui.present_job(
            &snapshot,
            Some(crate::features::synchronization::automatic::Phase::Running),
        );
        assert!(attention.widget().is_visible());
        assert!(issues.is_visible());
        assert!(ui.card.as_ref().unwrap().widget().is_visible());
        snapshot.result = None;
        snapshot.phase = crate::features::synchronization::automatic::Phase::Waiting;
        ui.present_job(
            &snapshot,
            Some(crate::features::synchronization::automatic::Phase::Running),
        );
        assert!(
            !attention.widget().is_visible(),
            "a new configuration discards previous-scope observations"
        );
    }

    #[test]
    fn preserved_paths_remain_visible() {
        let report = DownloadReport {
            preserved_copies: vec!["Folder/DriveSync backup example.bak".into()],
            ..Default::default()
        };
        assert!(preserved_text(&report).contains("Folder/DriveSync backup example.bak"));
        assert!(preserved_text(&DownloadReport::default()).is_empty());
    }
    #[test]
    fn recovered_conflict_copy_is_visible_without_a_current_execution_issue() {
        let report = DownloadReport {
            conflict_copies: vec![drivesync_engine::ConflictCopy {
                original_path: "Folder/sheet.xlsx".into(),
                copy_path: "Folder/DriveSync backup conflict-example.bak".into(),
                remote_id: "remote-id".into(),
                remote_version: 7,
            }],
            ..Default::default()
        };
        let text = preserved_text(&report);
        assert!(text.contains("Folder/DriveSync backup conflict-example.bak"));
        assert!(text.contains("Drive version 7 of Folder/sheet.xlsx"));
    }
}
