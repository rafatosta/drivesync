//! Automatic and explicit downloads observing application-owned account jobs.
use super::presentation::preparation_text;
use adw::{gtk, prelude::*};
use drivesync_engine::{
    DownloadProgress, DownloadReport, DownloadStage, DriveSyncEngine, EngineError, RestoredAccount,
};
#[cfg(test)]
use std::{cell::Cell, rc::Rc};
use std::{sync::Arc, time::Duration};

/// Weak handles let navigation dispose account pages while cancellation propagates.
#[derive(Clone)]
struct Widgets {
    card: Option<crate::sync_progress_card::SyncProgressCard>,
    attention: Option<super::attention_summary::AttentionSummary>,
    start: gtk::glib::WeakRef<gtk::Button>,
    header: super::account_header::AccountHeader,
    folders: gtk::glib::WeakRef<gtk::Box>,
    row: gtk::glib::WeakRef<adw::ActionRow>,
    account_row: gtk::glib::WeakRef<adw::ActionRow>,
    status: gtk::glib::WeakRef<gtk::Label>,
    progress: gtk::glib::WeakRef<gtk::ProgressBar>,
    issues: gtk::glib::WeakRef<gtk::Box>,
    activity: gtk::glib::WeakRef<gtk::Box>,
    skipped: gtk::glib::WeakRef<gtk::Label>,
    preserved: gtk::glib::WeakRef<gtk::Label>,
    spinner: gtk::glib::WeakRef<gtk::Spinner>,
}
impl Widgets {
    fn progress(&self, event: &DownloadProgress) {
        if let Some(stage) = &event.preparation {
            let (title, detail) = preparation_text(stage);
            if let Some(row) = self.row.upgrade() {
                row.set_title(title);
                if let drivesync_engine::PreparationProgress::Scanning { current_path, .. } = stage
                {
                    let path = if current_path.as_os_str().is_empty() {
                        "Selected folder".into()
                    } else {
                        current_path.to_string_lossy()
                    };
                    row.set_subtitle(&format!("Reading folder: {path}"));
                } else {
                    row.set_subtitle(&detail);
                }
            }
            if let Some(status) = self.status.upgrade() {
                status.set_label(&detail);
            }
            if let Some(bar) = self.progress.upgrade() {
                bar.set_visible(false);
            }
            return;
        }
        if let Some(bar) = self.progress.upgrade() {
            bar.set_visible(true);
            bar.set_fraction(cycle_fraction(event));
            bar.set_text(Some(&format!(
                "Cycle: {} of {} items processed",
                event.completed_items, event.total_items
            )));
        }
        if let Some(current) = &event.current {
            if let Some(row) = self.row.upgrade() {
                row.set_title(match current.stage {
                    DownloadStage::Checking => "Checking item",
                    DownloadStage::Downloading => "Downloading file",
                    DownloadStage::Uploading => "Uploading new file",
                    DownloadStage::Verifying => "Verifying file integrity",
                    DownloadStage::Installing => "Saving verified file",
                    DownloadStage::Linking => "Creating browser shortcut",
                });
                row.set_subtitle(&current.relative_path.to_string_lossy());
            }
        }
        if event.current.is_none() {
            if let Some(row) = self.row.upgrade() {
                row.set_title(if event.completed_items == event.total_items {
                    "Finalizing cycle"
                } else {
                    "Processing files and folders"
                });
                row.set_subtitle(
                    "Items include folders, unchanged files, skipped items and conflicts.",
                );
            }
        }
        if let Some(status) = self.status.upgrade() {
            status.set_label(&progress_text(event));
        }
    }
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
            super::recent_activity::present(&container, report);
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

        if let Some(row) = self.row.upgrade() {
            row.set_title("Checking for changes");
            row.set_subtitle("Checking Google Drive changes and existing local copies…");
        }
        if let Some(status) = self.status.upgrade() {
            status.set_label("Connecting to Google Drive. Analysis progress will appear here…");
        }
        if let Some(preserved) = self.preserved.upgrade() {
            preserved.set_visible(false);
        }
        if let Some(skipped) = self.skipped.upgrade() {
            skipped.set_visible(false);
        }
        if let Some(spinner) = self.spinner.upgrade() {
            spinner.set_visible(true);
            spinner.start();
        }
        if let Some(bar) = self.progress.upgrade() {
            bar.set_fraction(0.0);
            bar.set_visible(false);
        }
    }
    /// Scheduling state and transfer results have separate meanings. Keep issue
    /// details visible while showing offline/retry/pause as the account status.
    fn present_job(
        &self,
        snapshot: &crate::automatic::Snapshot,
        previous: Option<crate::automatic::Phase>,
    ) {
        self.present_details(snapshot, previous);
        if let Some(card) = &self.card {
            card.present(snapshot);
        }
    }
    fn present_details(
        &self,
        snapshot: &crate::automatic::Snapshot,
        previous: Option<crate::automatic::Phase>,
    ) {
        use crate::automatic::Phase;
        self.header.present(snapshot);
        let root_suspended = snapshot
            .result
            .as_ref()
            .is_some_and(|result| matches!(result, Err(EngineError::RootSuspended(_))));
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
                if let Some(row) = self.row.upgrade() {
                    row.set_title("Pausing synchronization");
                    row.set_subtitle("The current operation will finish or stop at a safe point…");
                }
            } else if let Some(progress) = &snapshot.progress {
                self.progress(progress);
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
                if let Some(status) = self.status.upgrade() {
                    status.set_label("");
                }
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
        if let Some(row) = self.row.upgrade() {
            let (title, subtitle) = match snapshot.phase {
                Phase::Unconfigured => return,
                Phase::Waiting => (
                    "Waiting for automatic synchronization",
                    "Configured accounts are checked automatically.",
                ),
                Phase::UpToDate if !super::presentation::confirmed(snapshot) => (
                    "Needs attention",
                    "Completion could not be confirmed. Review the synchronization details.",
                ),
                Phase::UpToDate => (
                    "Up to date",
                    "The latest check completed without pending items.",
                ),
                Phase::Attention => (
                    "Some files could not be updated",
                    "Local files were preserved. Automatic checks continue.",
                ),
                Phase::Offline => (
                    "Unable to connect",
                    "Waiting for a connection · automatic retry enabled",
                ),
                Phase::Retrying => (
                    "Retrying automatically",
                    "Temporary failure · retry interval increases up to five minutes",
                ),
                Phase::Paused => (
                    "Synchronization paused",
                    "Completed work is saved safely. Resume to continue synchronization.",
                ),
                Phase::Blocked if root_suspended => (
                    "Synchronization suspended",
                    "Restore folder access, then try synchronization again.",
                ),
                Phase::Blocked => (
                    "Needs attention",
                    "Fix the problem, then start synchronization to retry.",
                ),
                Phase::Running => unreachable!(),
            };
            row.set_title(title);
            row.set_subtitle(subtitle);
        }
        if snapshot.phase == Phase::UpToDate && !super::presentation::confirmed(snapshot) {
            if let Some(status) = self.status.upgrade() {
                status.set_label(
                    "Completion could not be confirmed. Review the synchronization details.",
                );
            }
        }
        if snapshot.phase == Phase::Paused {
            if let Some(button) = self.start.upgrade() {
                button.set_label("Resume");
            }
            if let Some(status) = self.status.upgrade() {
                status.set_label(
                    "Synchronization paused safely. Completed files were preserved; unfinished work can resume.",
                );
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
        if let Some(bar) = self.progress.upgrade() {
            bar.set_visible(false);
        }
        if let Some(spinner) = self.spinner.upgrade() {
            spinner.stop();
            spinner.set_visible(false);
        }
    }
    fn finish(&self, result: &Result<DownloadReport, EngineError>) {
        self.idle();
        match result {
            Ok(report) => {
                if let Some(status) = self.status.upgrade() {
                    status.set_label(&summary(report));
                }
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
                if let Some(status) = self.status.upgrade() {
                    status.set_label(if cancelled {
                        "Download cancelled. Completed files were preserved."
                    } else {
                        "The cycle could not finish."
                    });
                }
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
/// Item-count progress advances only after an item finishes checking/installation.
fn cycle_fraction(event: &DownloadProgress) -> f64 {
    if event.total_items == 0 {
        0.0
    } else {
        (event.completed_items as f64 / event.total_items as f64).clamp(0.0, 1.0)
    }
}
fn progress_text(event: &DownloadProgress) -> String {
    let Some(item) = &event.current else {
        return if event.completed_items == event.total_items {
            "All items processed. Preparing the result…".into()
        } else {
            "The cycle bar counts checked files and folders, not bytes.".into()
        };
    };
    match item.stage {
        DownloadStage::Checking => "Checking this item and its existing local copy…".into(),
        DownloadStage::Verifying => "Download received. Verifying this file before saving…".into(),
        DownloadStage::Installing => "Integrity verified. Saving this file safely…".into(),
        DownloadStage::Linking => {
            "Creating a local link to open this Google document in your browser…".into()
        }
        DownloadStage::Downloading | DownloadStage::Uploading => match item.total {
            Some(total) if total > 0 => format!(
                "Current file: {} of {} ({:.0}%)",
                gtk::glib::format_size(item.received),
                gtk::glib::format_size(total),
                100.0 * (item.received as f64 / total as f64).clamp(0.0, 1.0)
            ),
            _ => format!(
                "Current file: {} received",
                gtk::glib::format_size(item.received)
            ),
        },
    }
}
fn summary(report: &DownloadReport) -> String {
    let mut parts = Vec::new();
    if report.linked > 0 {
        let label = if report.linked == 1 {
            "browser shortcut"
        } else {
            "browser shortcuts"
        };
        parts.push(format!("{} {label} (Docs, Sheets and Slides: browser only; no offline editing, export or conversion)", report.linked));
    }
    for (count, label) in [
        (report.downloaded, "downloaded"),
        (report.uploaded, "uploaded"),
        (report.created_remote_folders, "folders created in Drive"),
        (report.updated, "updated"),
        (report.recovered, "recovered"),
        (report.skipped, "skipped"),
        (report.attention().len(), "need attention"),
    ] {
        if count > 0 {
            parts.push(format!("{count} {label}"));
        }
    }
    if parts.is_empty() {
        "All files are up to date.".into()
    } else {
        parts.join(" · ")
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
    pub header: &'a super::account_header::AccountHeader,
    pub account_row: &'a adw::ActionRow,
}

/// Observes the shared account job. Page disposal stops observation only; the
/// application-owned worker continues while hidden or while pages are rebuilt.
#[cfg(test)]
pub(super) fn build(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    controls: Controls<'_>,
) -> gtk::Box {
    let job = crate::automatic::Controller::attach(engine.clone(), saved);
    build_for_job(engine, saved, controls, job)
}

/// Uses the same controller as both account menus, including isolated fixtures.
pub(super) fn build_for_job(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    controls: Controls<'_>,
    job: crate::automatic::Controller,
) -> gtk::Box {
    let Controls {
        folders,
        start,
        row,
        status,
        header,
        account_row,
    } = controls;
    let footer = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let note = gtk::Label::builder()
        .label("Automatic checks wait while synchronization is paused or blocked. Up-to-date files are not transferred again. Existing local edits and previous copies are preserved.")
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    let diagnostics = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let diagnostic_group = adw::PreferencesGroup::new();
    let diagnostic_row = adw::ExpanderRow::builder()
        .title(gettextrs::gettext("Synchronization details"))
        .build();
    diagnostic_row.set_widget_name("sync-details");
    diagnostic_row.add_row(&diagnostics);
    diagnostic_group.add(&diagnostic_row);
    let legacy_state = adw::PreferencesGroup::new();
    legacy_state.add(row);
    diagnostics.append(&legacy_state);
    diagnostics.append(&note);
    let spinner = gtk::Spinner::new();
    spinner.set_widget_name("download-activity");
    spinner.set_halign(gtk::Align::Start);
    spinner.set_visible(false);
    diagnostics.append(&spinner);
    let progress = gtk::ProgressBar::new();
    progress.set_widget_name("download-progress");
    progress.set_show_text(true);
    progress.set_tooltip_text(Some("Cycle progress counts all processed files and folders, including skipped items and conflicts. It does not show downloaded bytes."));
    progress.set_visible(false);
    let card = crate::sync_progress_card::SyncProgressCard::new();
    diagnostics.append(&progress);
    footer.prepend(card.widget());
    diagnostics.append(status);
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
    diagnostics.append(&issues);
    let activity = gtk::Box::new(gtk::Orientation::Vertical, 0);
    activity.set_visible(false);
    activity.set_widget_name("download-history");
    footer.append(&activity);
    footer.append(start);
    let recovery = super::state_recovery::button(engine.clone(), saved, status, job.clone());
    let weak_diagnostics = diagnostic_row.downgrade();
    recovery.connect_clicked(move |_| {
        if let Some(row) = weak_diagnostics.upgrade() {
            row.set_expanded(true);
        }
    });
    diagnostics.append(&recovery);
    let review = super::problems::button(engine.clone(), saved, job.clone());
    let attention = super::attention_summary::AttentionSummary::new(&review);
    footer.insert_child_after(attention.widget(), Some(card.widget()));
    footer.append(&review);
    footer.append(&diagnostic_group);
    let ui = Widgets {
        card: Some(card),
        attention: Some(attention),
        start: start.downgrade(),
        header: header.clone(),
        folders: folders.downgrade(),
        row: row.downgrade(),
        account_row: account_row.downgrade(),
        status: status.downgrade(),
        progress: progress.downgrade(),
        issues: issues.downgrade(),
        activity: activity.downgrade(),
        skipped: skipped.downgrade(),
        preserved: preserved.downgrade(),
        spinner: spinner.downgrade(),
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
    footer.append(&super::bulk_review::button(job.clone()));
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
    let header = super::account_header::AccountHeader::new("Fixture", "Connected");

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
    assert_eq!(row.title(), "Needs attention");
    assert!(!start.is_sensitive());
    assert!(folders.is_sensitive());

    assert!(footer.first_child().is_some());
}

/// Verifies real native controls in active, verification and completion states.
#[cfg(test)]
pub(super) fn verify_progress_presentations(mut capture: impl FnMut(&adw::Window, &str)) {
    use drivesync_engine::{AccountId, AccountSummary, DownloadIssue, DownloadItemProgress};
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
        let start = gtk::Button::with_label("Pause");
        start.set_halign(gtk::Align::End);
        let folders = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let row = adw::ActionRow::builder()
            .title("Synchronization")
            .use_markup(false)
            .build();
        let account_row = adw::ActionRow::new();
        let header = super::account_header::AccountHeader::new("Fixture", "Connected");
        let account_status = header.status_for_test();
        let status = gtk::Label::builder().wrap(true).xalign(0.0).build();

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
            },
        );
        fn collect(
            widget: &gtk::Widget,
            controls: &mut std::collections::HashMap<String, gtk::Widget>,
        ) {
            controls.insert(widget.widget_name().to_string(), widget.clone());
            let mut child = widget.first_child();
            while let Some(current) = child {
                collect(&current, controls);
                child = current.next_sibling();
            }
        }
        let mut controls = std::collections::HashMap::new();
        collect(footer.upcast_ref(), &mut controls);
        let bar = controls["download-progress"]
            .clone()
            .downcast::<gtk::ProgressBar>()
            .unwrap();
        let spinner = controls["download-activity"]
            .clone()
            .downcast::<gtk::Spinner>()
            .unwrap();
        let issues = controls["download-issues"]
            .clone()
            .downcast::<gtk::Box>()
            .unwrap();
        let activity = controls["download-history"]
            .clone()
            .downcast::<gtk::Box>()
            .unwrap();
        let skipped = controls["download-skipped"]
            .clone()
            .downcast::<gtk::Label>()
            .unwrap();
        let preserved = controls["download-preserved"]
            .clone()
            .downcast::<gtk::Label>()
            .unwrap();
        let test_card = crate::sync_progress_card::SyncProgressCard::new();
        footer.remove(&footer.first_child().unwrap());
        footer.prepend(test_card.widget());
        let review = controls["problem-center"]
            .clone()
            .downcast::<gtk::Button>()
            .unwrap();
        let test_attention = super::attention_summary::AttentionSummary::new(&review);
        footer.remove(&controls["attention-summary"]);
        footer.insert_child_after(test_attention.widget(), Some(test_card.widget()));
        let ui = Widgets {
            card: Some(test_card),
            attention: Some(test_attention),
            start: start.downgrade(),
            header: header.clone(),
            folders: folders.downgrade(),
            row: row.downgrade(),
            account_row: account_row.downgrade(),
            status: status.downgrade(),
            progress: bar.downgrade(),
            issues: issues.downgrade(),
            activity: activity.downgrade(),
            skipped: skipped.downgrade(),
            preserved: preserved.downgrade(),
            spinner: spinner.downgrade(),
        };
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        content.append(&footer);
        let window = adw::Window::builder()
            .default_width(width)
            .default_height(520)
            .content(&content)
            .build();
        window.present();
        spinner.set_visible(true);
        spinner.start();
        let present_progress = |event: &DownloadProgress| {
            ui.present_job(
                &crate::automatic::Snapshot {
                    revision: 1,
                    pausing: false,
                    phase: crate::automatic::Phase::Running,
                    progress: Some(event.clone()),
                    result: None,
                },
                Some(crate::automatic::Phase::Running),
            );
        };
        for stage in [
            drivesync_engine::PreparationProgress::Connecting,
            drivesync_engine::PreparationProgress::Scanning {
                files: 320,
                folders: 42,
                current_path: "Documentos/Projetos".into(),
            },
            drivesync_engine::PreparationProgress::ReadingChanges,
            drivesync_engine::PreparationProgress::CheckingLocal { checked: 128 },
            drivesync_engine::PreparationProgress::SavingState,
        ] {
            present_progress(&DownloadProgress {
                preparation: Some(stage),
                completed_items: 0,
                total_items: 0,
                current: None,
            });
            assert!(!bar.is_visible());
            assert!(spinner.is_spinning());
            if row.title() == "Analyzing Google Drive" {
                assert!(status.text().contains("320 files and 42 folders"));
                assert!(row.subtitle().unwrap().contains("Documentos/Projetos"));
                capture(&window, &format!("download-analysis-{width}"));
                assert!(window.width() <= width + 10);
            }
        }
        let mut event = DownloadProgress {
            preparation: None,
            completed_items: 7,
            total_items: 20,
            current: Some(DownloadItemProgress {
                relative_path: "Photos/Example.jpg".into(),
                stage: DownloadStage::Downloading,
                received: 2_500_000,
                total: Some(3_400_000),
            }),
        };
        present_progress(&event);
        assert_eq!(bar.fraction(), 0.35);
        assert!(bar.text().unwrap().contains("7 of 20"));
        assert!(status.text().starts_with("Current file:"));
        capture(&window, &format!("download-progress-{width}"));
        assert!(window.width() <= width + 10);
        event.current.as_mut().unwrap().stage = DownloadStage::Uploading;
        present_progress(&event);
        assert_eq!(row.title(), "Uploading new file");
        capture(&window, &format!("upload-progress-{width}"));
        event.current.as_mut().unwrap().stage = DownloadStage::Verifying;
        present_progress(&event);
        assert_eq!(row.title(), "Verifying file integrity");
        assert_eq!(bar.fraction(), 0.35);
        let completed = |report, phase| {
            ui.present_job(
                &crate::automatic::Snapshot {
                    revision: 1,
                    pausing: false,
                    phase,
                    progress: None,
                    result: Some(Ok(report)),
                },
                Some(crate::automatic::Phase::Running),
            )
        };
        completed(
            DownloadReport {
                skipped: 1,
                ..Default::default()
            },
            crate::automatic::Phase::Attention,
        );
        assert_eq!(row.title(), "Some files could not be updated");
        assert!(!issues.is_visible());
        assert!(skipped.is_visible());
        assert!(!skipped.has_css_class("error"));
        completed(
            DownloadReport {
                unchanged: 5,
                updated: 2,
                preserved_copies: vec!["Reports/DriveSync backup example.bak".into()],
                skipped: 1,
                issues: vec![DownloadIssue {
                    relative_path: Some("Reports/Report.pdf".into()),
                    error: EngineError::RemoteChanged,
                }],
                ..Default::default()
            },
            crate::automatic::Phase::Retrying,
        );
        assert!(preserved.is_visible());
        assert!(preserved.text().contains("DriveSync backup example.bak"));
        assert!(issues.is_visible());
        assert!(!bar.is_visible());
        assert!(!spinner.is_spinning());
        capture(&window, &format!("download-result-{width}"));
        assert!(window.width() <= width + 10);
        let reconciliation_report = DownloadReport {
            reconciliation: vec![
                drivesync_engine::ReconciliationEntry {
                    relative_path: Some("Reports/draft.part".into()),
                    remote_id: None,
                    kind: drivesync_engine::ChangeKind::Temporary,
                },
                drivesync_engine::ReconciliationEntry {
                    relative_path: Some("Reports/writing.txt".into()),
                    remote_id: None,
                    kind: drivesync_engine::ChangeKind::Writing,
                },
                drivesync_engine::ReconciliationEntry {
                    relative_path: Some("Reports/concurrent.txt".into()),
                    remote_id: Some("concurrent".into()),
                    kind: drivesync_engine::ChangeKind::ConcurrentChanges,
                },
                drivesync_engine::ReconciliationEntry {
                    relative_path: Some("Reports/equivalent.txt".into()),
                    remote_id: Some("equivalent".into()),
                    kind: drivesync_engine::ChangeKind::Equivalent,
                },
            ],
            ..Default::default()
        };
        completed(reconciliation_report, crate::automatic::Phase::Attention);
        assert!(issues.is_visible());
        assert!(activity.is_visible());
        assert_eq!(status.text(), "3 need attention");
        capture(&window, &format!("reconciliation-pending-{width}"));
        assert!(window.width() <= width + 10);
        let history_report = DownloadReport {
            unchanged: 8,
            previous_paths: vec![
                drivesync_engine::PreviousPath {
                    previous_path: Some("pasta-a/mesmo-nome.txt".into()),
                    current_path: Some("pasta-a-renomeada/mesmo-nome.txt".into()),
                },
                drivesync_engine::PreviousPath {
                    previous_path: Some("pasta-b/mesmo-nome.txt".into()),
                    current_path: None,
                },
            ],
            ..Default::default()
        };
        completed(history_report.clone(), crate::automatic::Phase::UpToDate);
        assert_eq!(row.title(), "Up to date");
        assert_eq!(status.text(), "All files are up to date.");
        assert!(!issues.is_visible());
        assert!(activity.is_visible());
        let mut pending = vec![activity.clone().upcast::<gtk::Widget>()];
        let mut expander = None;
        while let Some(widget) = pending.pop() {
            if let Ok(found) = widget.clone().downcast::<adw::ExpanderRow>() {
                expander = Some(found);
                break;
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                pending.push(widget);
            }
        }
        let expander = expander.expect("native activity expander");
        assert!(!expander.is_expanded());
        capture(&window, &format!("download-history-collapsed-{width}"));
        expander.set_expanded(true);
        capture(&window, &format!("download-history-expanded-{width}"));
        assert!(window.width() <= width + 10);
        let mut conflict = history_report;
        conflict.issues.push(DownloadIssue {
            relative_path: Some("current/local-edit.txt".into()),
            error: EngineError::DownloadConflict,
        });
        completed(conflict, crate::automatic::Phase::Attention);
        assert!(issues.is_visible());
        assert!(activity.is_visible());
        assert_eq!(row.title(), "Some files could not be updated");
        for (phase, title, name) in [
            (crate::automatic::Phase::UpToDate, "Up to date", "current"),
            (
                crate::automatic::Phase::Offline,
                "Unable to connect",
                "offline",
            ),
            (
                crate::automatic::Phase::Retrying,
                "Retrying automatically",
                "retry",
            ),
            (
                crate::automatic::Phase::Paused,
                "Synchronization paused",
                "paused",
            ),
            (
                crate::automatic::Phase::Blocked,
                "Needs attention",
                "blocked",
            ),
        ] {
            ui.present_job(
                &crate::automatic::Snapshot {
                    revision: 1,
                    pausing: false,
                    phase,
                    progress: None,
                    result: if phase == crate::automatic::Phase::UpToDate {
                        Some(Ok(DownloadReport {
                            unchanged: 8,
                            ..Default::default()
                        }))
                    } else {
                        None
                    },
                },
                None,
            );
            assert_eq!(row.title(), title);
            if phase == crate::automatic::Phase::Paused {
                assert_eq!(start.label().as_deref(), Some("Resume"));
            }
            capture(&window, &format!("automatic-{name}-{width}"));
            assert!(window.width() <= width + 10);
        }
        for (reason, name) in [
            (
                drivesync_engine::RootSuspension::LocalUnavailable,
                "local-unavailable",
            ),
            (
                drivesync_engine::RootSuspension::IdentityUnverified,
                "identity-unverified",
            ),
            (
                drivesync_engine::RootSuspension::RemoteUnavailable,
                "remote-unavailable",
            ),
            (
                drivesync_engine::RootSuspension::RemoteAccessDenied,
                "access-denied",
            ),
        ] {
            let error = EngineError::RootSuspended(reason);
            ui.present_job(
                &crate::automatic::Snapshot {
                    revision: 2,
                    pausing: false,
                    phase: crate::automatic::Phase::Blocked,
                    progress: None,
                    result: Some(Err(error.clone())),
                },
                None,
            );
            assert_eq!(row.title(), "Synchronization suspended");
            assert_eq!(account_status.label(), "Folder unavailable");
            assert_eq!(status.label(), "The cycle could not finish.");
            assert!(issues.is_visible());
            assert!(start.is_sensitive());
            capture(&window, &format!("root-{name}-{width}"));
            assert!(window.width() <= width + 10);
        }
        ui.begin();
        ui.present_job(
            &crate::automatic::Snapshot {
                revision: 2,
                pausing: false,
                phase: crate::automatic::Phase::Waiting,
                progress: None,
                result: None,
            },
            Some(crate::automatic::Phase::Running),
        );
        assert!(!spinner.is_spinning());
        assert!(folders.is_sensitive());

        assert_eq!(start.label().as_deref(), Some("Synchronize Now"));
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
        let row = adw::ActionRow::new();
        let start = gtk::Button::new();
        let status = gtk::Label::new(None);
        let issues = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let activity = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let skipped = gtk::Label::new(None);
        let preserved = gtk::Label::new(None);
        let review = gtk::Button::new();
        let attention = super::super::attention_summary::AttentionSummary::new(&review);
        let ui = Widgets {
            card: Some(crate::sync_progress_card::SyncProgressCard::new()),
            attention: Some(attention.clone()),
            start: start.downgrade(),
            header: crate::dashboard::account_header::AccountHeader::new("Fixture", "Connected"),
            folders: Default::default(),

            row: row.downgrade(),
            account_row: Default::default(),
            status: status.downgrade(),
            progress: Default::default(),
            issues: issues.downgrade(),
            activity: activity.downgrade(),
            skipped: skipped.downgrade(),
            preserved: preserved.downgrade(),
            spinner: Default::default(),
        };
        for previous in [
            crate::automatic::Phase::Offline,
            crate::automatic::Phase::Blocked,
            crate::automatic::Phase::Running,
        ] {
            ui.present_job(
                &crate::automatic::Snapshot {
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
            if previous != crate::automatic::Phase::Running {
                assert!(issues.is_visible());
                assert_eq!(status.label(), "The cycle could not finish.");
            }
            activity.set_visible(true);
            skipped.set_visible(true);
            preserved.set_visible(true);
            ui.present_job(
                &crate::automatic::Snapshot {
                    revision: 2,
                    pausing: false,
                    phase: crate::automatic::Phase::Waiting,
                    progress: None,
                    result: None,
                },
                Some(previous),
            );
            assert_eq!(row.title(), "Waiting for automatic synchronization");
            assert!(status.label().is_empty());
            assert!(!issues.is_visible());
            assert!(issues.first_child().is_none());
            assert!(!activity.is_visible());
            assert!(!skipped.is_visible());
            assert!(!preserved.is_visible());
        }
        ui.present_job(
            &crate::automatic::Snapshot {
                revision: 3,
                pausing: false,
                phase: crate::automatic::Phase::UpToDate,
                progress: None,
                result: None,
            },
            Some(crate::automatic::Phase::Waiting),
        );
        assert_eq!(row.title(), "Needs attention");
        assert!(status.label().contains("could not be confirmed"));
        let mut snapshot = crate::automatic::Snapshot {
            revision: 3,
            pausing: false,
            phase: crate::automatic::Phase::Attention,
            progress: None,
            result: Some(Ok(DownloadReport {
                issues: vec![drivesync_engine::DownloadIssue {
                    relative_path: Some("pending.txt".into()),
                    error: EngineError::DownloadConflict,
                }],
                ..Default::default()
            })),
        };
        ui.present_job(&snapshot, Some(crate::automatic::Phase::Waiting));
        assert!(attention.widget().is_visible());
        assert!(issues.is_visible());
        snapshot.phase = crate::automatic::Phase::Running;
        snapshot.result = None;
        ui.present_job(&snapshot, Some(crate::automatic::Phase::Attention));
        assert!(
            attention.widget().is_visible(),
            "running independent transfers must not hide observed conflicts"
        );
        assert!(issues.is_visible());
        snapshot.phase = crate::automatic::Phase::Waiting;
        ui.present_job(&snapshot, Some(crate::automatic::Phase::Running));
        assert!(
            !attention.widget().is_visible(),
            "a new configuration discards previous-scope observations"
        );
    }

    #[test]
    fn activity_is_not_an_error_or_an_action_in_the_completion_summary() {
        let report = DownloadReport {
            unchanged: 8,
            previous_paths: vec![drivesync_engine::PreviousPath {
                previous_path: Some("old.txt".into()),
                current_path: None,
            }],
            ..Default::default()
        };
        assert_eq!(summary(&report), "All files are up to date.");
    }
    #[test]
    fn updated_count_and_preserved_paths_remain_visible_in_summary() {
        let report = DownloadReport {
            updated: 2,
            preserved_copies: vec!["Folder/DriveSync backup example.bak".into()],
            ..Default::default()
        };
        assert!(summary(&report).contains("2 updated"));
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
    #[test]
    fn cycle_count_and_current_file_bytes_have_separate_meanings() {
        let mut progress = DownloadProgress {
            preparation: None,
            completed_items: 3,
            total_items: 10,
            current: Some(drivesync_engine::DownloadItemProgress {
                relative_path: "photo.jpg".into(),
                stage: DownloadStage::Downloading,
                received: 25,
                total: Some(100),
            }),
        };
        assert_eq!(cycle_fraction(&progress), 0.3);
        assert!(progress_text(&progress).starts_with("Current file:"));
        assert!(progress_text(&progress).contains("25%"));
        progress.current.as_mut().unwrap().received = 100;
        assert_eq!(cycle_fraction(&progress), 0.3);
        progress.current.as_mut().unwrap().stage = DownloadStage::Verifying;
        assert!(progress_text(&progress).contains("Verifying"));
        assert_eq!(cycle_fraction(&progress), 0.3);
        progress.current.as_mut().unwrap().stage = DownloadStage::Installing;
        assert!(progress_text(&progress).contains("Saving"));
    }
    #[test]
    fn completion_summary_does_not_hide_pending_items() {
        let report = DownloadReport {
            downloaded: 2,
            skipped: 1,
            issues: vec![drivesync_engine::DownloadIssue {
                relative_path: None,
                error: EngineError::DownloadConflict,
            }],
            ..Default::default()
        };
        assert_eq!(
            summary(&report),
            "2 downloaded · 1 skipped · 1 need attention"
        );
    }
}
