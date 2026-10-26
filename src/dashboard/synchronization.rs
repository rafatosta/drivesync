//! Automatic and explicit downloads observing application-owned account jobs.
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
    start: gtk::glib::WeakRef<gtk::Button>,
    account_toggle: gtk::glib::WeakRef<gtk::Button>,
    folders: gtk::glib::WeakRef<gtk::Box>,
    menu: gtk::glib::WeakRef<gtk::MenuButton>,
    row: gtk::glib::WeakRef<adw::ActionRow>,
    account_row: gtk::glib::WeakRef<adw::ActionRow>,
    account_status: gtk::glib::WeakRef<gtk::Label>,
    account_status_fallback: String,
    status: gtk::glib::WeakRef<gtk::Label>,
    menu_status: gtk::glib::WeakRef<gtk::Label>,
    progress: gtk::glib::WeakRef<gtk::ProgressBar>,
    issues: gtk::glib::WeakRef<gtk::Box>,
    activity: gtk::glib::WeakRef<gtk::Box>,
    skipped: gtk::glib::WeakRef<gtk::Label>,
    preserved: gtk::glib::WeakRef<gtk::Label>,
    spinner: gtk::glib::WeakRef<gtk::Spinner>,
}
impl Widgets {
    /// Mirrors the shared synchronization state in the account-header control.
    fn present_account_toggle(&self, snapshot: &crate::automatic::Snapshot) {
        let Some(button) = self.account_toggle.upgrade() else {
            return;
        };
        match snapshot.phase {
            crate::automatic::Phase::Running if snapshot.pausing => {
                button.set_icon_name("process-working-symbolic");
                button.set_tooltip_text(Some("Pausing safely…"));
                button.set_sensitive(false);
            }
            crate::automatic::Phase::Running => {
                button.set_icon_name("media-playback-pause-symbolic");
                button.set_tooltip_text(Some("Pause synchronization"));
                button.set_sensitive(true);
            }
            crate::automatic::Phase::Paused => {
                button.set_icon_name("media-playback-start-symbolic");
                button.set_tooltip_text(Some("Resume synchronization"));
                button.set_sensitive(true);
            }
            crate::automatic::Phase::Unconfigured => {
                button.set_icon_name("media-playback-start-symbolic");
                button.set_tooltip_text(Some("Choose a folder to enable synchronization"));
                button.set_sensitive(false);
            }
            _ => {
                button.set_icon_name("media-playback-start-symbolic");
                button.set_tooltip_text(Some("Start or resume synchronization"));
                button.set_sensitive(true);
            }
        }
    }
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
        if let Some(container) = self.issues.upgrade() {
            while let Some(child) = container.first_child() {
                container.remove(&child);
            }
            container.set_visible(false);
        }
    }
    fn show_issues(&self, report: &DownloadReport) {
        self.clear_issues();
        if let Some(container) = self.issues.upgrade() {
            let group = adw::PreferencesGroup::builder()
                .title("Needs attention")
                .build();
            for issue in report.issues.iter().take(5) {
                let path = issue.relative_path.as_deref();
                let name = path
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Item".into());
                let row = adw::ActionRow::builder()
                    .title(&name)
                    .subtitle(issue.error.to_string())
                    .use_markup(false)
                    .build();
                row.set_title_lines(0);
                row.set_subtitle_lines(3);
                row.set_tooltip_text(Some(&format!(
                    "{}\n{}",
                    path.map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "Item".into()),
                    issue.error
                )));
                let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
                icon.add_css_class("warning");
                row.add_prefix(&icon);
                group.add(&row);
            }
            if report.issues.len() > 5 {
                group.add(
                    &adw::ActionRow::builder()
                        .title(format!(
                            "{} more items need attention",
                            report.issues.len() - 5
                        ))
                        .build(),
                );
            }
            container.append(&group);
            container.set_visible(!report.issues.is_empty());
        }
    }
    /// Normal remote path changes belong in optional activity details. They
    /// never display warning icons or require a user acknowledgment.
    fn show_activity(&self, report: &DownloadReport) {
        let Some(container) = self.activity.upgrade() else {
            return;
        };
        while let Some(child) = container.first_child() {
            container.remove(&child);
        }
        container.set_visible(!report.previous_paths.is_empty());
        if report.previous_paths.is_empty() {
            return;
        }
        let group = adw::PreferencesGroup::new();
        let details = adw::ExpanderRow::builder()
            .title("View activity")
            .subtitle("Previous local copies are kept. No action is needed.")
            .use_markup(false)
            .build();
        details.set_subtitle_lines(0);
        for activity in report.previous_paths.iter().take(50) {
            let name = activity
                .previous_path
                .as_deref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Previous location".into());
            let subtitle = activity
                .current_path
                .as_deref()
                .map(|path| {
                    format!(
                        "Now at {} on Google Drive. Previous local copies are kept.",
                        path.to_string_lossy()
                    )
                })
                .unwrap_or_else(|| {
                    "No longer listed in the selected Google Drive folder. Local copies are kept."
                        .into()
                });
            let row = adw::ActionRow::builder()
                .title(&name)
                .subtitle(&subtitle)
                .use_markup(false)
                .build();
            row.set_title_lines(0);
            row.set_subtitle_lines(0);
            row.set_tooltip_text(Some(&format!("{name}\n{subtitle}")));
            row.add_prefix(&gtk::Image::from_icon_name("document-open-recent-symbolic"));
            details.add_row(&row);
        }
        if report.previous_paths.len() > 50 {
            details.add_row(
                &adw::ActionRow::builder()
                    .title(format!(
                        "{} additional previous paths recorded",
                        report.previous_paths.len() - 50
                    ))
                    .build(),
            );
        }
        group.add(&details);
        container.append(&group);
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
        if let Some(menu) = self.menu.upgrade() {
            menu.set_sensitive(false);
        }
        if let Some(row) = self.row.upgrade() {
            row.set_title("Checking for changes");
            row.set_subtitle("Checking Google Drive changes and existing local copies…");
        }
        if let Some(status) = self.status.upgrade() {
            status.set_label("Connecting to Google Drive. Analysis progress will appear here…");
        }
        self.clear_issues();
        if let Some(activity) = self.activity.upgrade() {
            activity.set_visible(false);
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
        use crate::automatic::Phase;
        self.present_account_toggle(snapshot);
        let sync_status = match snapshot.phase {
            Phase::Unconfigured => "Not configured",
            Phase::Waiting => "Waiting",
            Phase::Running if snapshot.pausing => "Pausing",
            Phase::Running => "Syncing",
            Phase::UpToDate => "Up to date",
            Phase::Attention | Phase::Blocked => "Needs attention",
            Phase::Offline => "Offline",
            Phase::Retrying => "Retrying",
            Phase::Paused => "Paused",
        };
        if let Some(status) = self.menu_status.upgrade() {
            status.set_label(&format!("Synchronization: {sync_status}"));
        }
        let account_status = if snapshot.phase == Phase::Unconfigured {
            self.account_status_fallback.as_str()
        } else {
            sync_status
        };
        if let Some(row) = self.account_row.upgrade() {
            row.set_subtitle(account_status);
        }
        if let Some(status) = self.account_status.upgrade() {
            status.set_label(account_status);
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
                    row.set_subtitle(
                        "The current operation will finish or stop at a safe point…",
                    );
                }
            } else if let Some(progress) = &snapshot.progress {
                self.progress(progress);
            }
            return;
        }
        if let Some(result) = &snapshot.result {
            self.finish(result);
        } else if previous == Some(Phase::Running) {
            self.idle();
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
                Phase::UpToDate => ("Up to date", "Next automatic check in about a minute"),
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
                Phase::Blocked => (
                    "Needs attention",
                    "Fix the problem, then start synchronization to retry.",
                ),
                Phase::Running => unreachable!(),
            };
            row.set_title(title);
            row.set_subtitle(subtitle);
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
        if let Some(menu) = self.menu.upgrade() {
            menu.set_sensitive(true);
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
                    preserved.set_visible(!report.preserved_copies.is_empty());
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
/// Preparation has no known total: show actual observations without a percentage.
fn preparation_text(stage: &drivesync_engine::PreparationProgress) -> (&'static str, String) {
    use drivesync_engine::PreparationProgress;
    match stage {
        PreparationProgress::ScanningUploads => (
            "Looking for new local files",
            "Checking which new files and folders can be uploaded…".into(),
        ),
        PreparationProgress::PreparingUpload { current_path } => (
            "Preparing a new upload",
            format!("Reading: {}", current_path.to_string_lossy()),
        ),
        PreparationProgress::Connecting => (
            "Connecting to Google Drive",
            "Verifying account access and the selected folder…".into(),
        ),
        PreparationProgress::ReadingChanges => (
            "Checking recent Drive changes",
            "Reading changes reported by Google Drive…".into(),
        ),
        PreparationProgress::Scanning { files, folders, .. } => (
            "Analyzing Google Drive",
            format!("{files} files and {folders} folders found so far…"),
        ),
        PreparationProgress::CheckingLocal { checked } => (
            "Checking files on this computer",
            format!("{checked} local paths checked…"),
        ),
        PreparationProgress::SavingState => (
            "Preparing downloads",
            "Saving the analysis before checking and downloading files…".into(),
        ),
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
    for (count, label) in [
        (report.downloaded, "downloaded"),
        (report.uploaded, "uploaded"),
        (report.created_remote_folders, "folders created in Drive"),
        (report.updated, "updated"),
        (report.recovered, "recovered"),
        (report.skipped, "skipped"),
        (report.issues.len(), "need attention"),
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
    if report.preserved_copies.is_empty() {
        return String::new();
    }
    let mut text = String::from("Previous versions kept beside the updated files:\n");
    text.push_str(
        &report
            .preserved_copies
            .iter()
            .take(3)
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    if report.preserved_copies.len() > 3 {
        text.push_str(&format!(
            "\n{} additional copies kept.",
            report.preserved_copies.len() - 3
        ));
    }
    text
}
/// Observes the shared account job. Page disposal stops observation only; the
/// application-owned worker continues while hidden or while pages are rebuilt.
pub(super) fn build(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    folders: &gtk::Box,
    start: &gtk::Button,
    row: &adw::ActionRow,
    status: &gtk::Label,
    menu: &gtk::MenuButton,
    account_toggle: &gtk::Button,
    account_row: &adw::ActionRow,
    account_status: &gtk::Label,
    account_status_fallback: &str,
    menu_status: &gtk::Label,
) -> gtk::Box {
    let footer = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let note = gtk::Label::builder()
        .label("Automatic checks run every minute. Up-to-date files are not transferred again. Existing local edits and previous copies are preserved.")
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    footer.append(&note);
    let spinner = gtk::Spinner::new();
    spinner.set_widget_name("download-activity");
    spinner.set_halign(gtk::Align::Start);
    spinner.set_visible(false);
    footer.append(&spinner);
    let progress = gtk::ProgressBar::new();
    progress.set_widget_name("download-progress");
    progress.set_show_text(true);
    progress.set_tooltip_text(Some("Cycle progress counts all processed files and folders, including skipped items and conflicts. It does not show downloaded bytes."));
    progress.set_visible(false);
    footer.append(&progress);
    footer.append(status);
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
    footer.append(start);
    let ui = Widgets {
        start: start.downgrade(),
        account_toggle: account_toggle.downgrade(),
        folders: folders.downgrade(),
        menu: menu.downgrade(),
        row: row.downgrade(),
        account_row: account_row.downgrade(),
        account_status: account_status.downgrade(),
        account_status_fallback: account_status_fallback.to_owned(),
        status: status.downgrade(),
        menu_status: menu_status.downgrade(),
        progress: progress.downgrade(),
        issues: issues.downgrade(),
        activity: activity.downgrade(),
        skipped: skipped.downgrade(),
        preserved: preserved.downgrade(),
        spinner: spinner.downgrade(),
    };
    let job = crate::automatic::Controller::attach(engine, saved);
    for button in [start, account_toggle] {
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
    let mut revision = u64::MAX;
    let mut phase = None;
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
    let account_status = gtk::Label::new(Some("Connected"));
    let menu = gtk::MenuButton::new();
    let account_toggle = gtk::Button::from_icon_name("media-playback-start-symbolic");
    let menu_status = gtk::Label::new(Some("Synchronization: Not started"));
    let footer = build(
        Arc::new(DriveSyncEngine::new()),
        &saved,
        &folders,
        &start,
        &row,
        &status,
        &menu,
        &account_toggle,
        &account_row,
        &account_status,
        "Connected",
        &menu_status,
    );
    start.emit_clicked();
    assert_eq!(start.label().as_deref(), Some("Pause"));
    assert!(!folders.is_sensitive());
    assert!(!menu.is_sensitive());
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
    assert!(menu.is_sensitive());
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
        let menu = gtk::MenuButton::new();
        let row = adw::ActionRow::builder()
            .title("Synchronization")
            .use_markup(false)
            .build();
        let account_row = adw::ActionRow::new();
        let account_status = gtk::Label::new(Some("Connected"));
        let status = gtk::Label::builder().wrap(true).xalign(0.0).build();
        let account_toggle = gtk::Button::from_icon_name("media-playback-start-symbolic");
        let menu_status = gtk::Label::new(Some("Synchronization: Not started"));
        let footer = build(
            Arc::new(DriveSyncEngine::new()),
            &saved,
            &folders,
            &start,
            &row,
            &status,
            &menu,
            &account_toggle,
            &account_row,
            &account_status,
            "Connected",
            &menu_status,
        );
        let mut controls = std::collections::HashMap::new();
        let mut child = footer.first_child();
        while let Some(widget) = child {
            controls.insert(widget.widget_name().to_string(), widget.clone());
            child = widget.next_sibling();
        }
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
        let ui = Widgets {
            start: start.downgrade(),
            account_toggle: account_toggle.downgrade(),
            folders: folders.downgrade(),
            menu: menu.downgrade(),
            row: row.downgrade(),
            account_row: account_row.downgrade(),
            account_status: account_status.downgrade(),
            account_status_fallback: "Connected".into(),
            status: status.downgrade(),
            menu_status: menu_status.downgrade(),
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
        let group = adw::PreferencesGroup::builder()
            .title("Synchronization")
            .build();
        group.add(&row);
        content.append(&group);
        content.append(&footer);
        let window = adw::Window::builder()
            .default_width(width)
            .default_height(520)
            .content(&content)
            .build();
        window.present();
        spinner.set_visible(true);
        spinner.start();
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
            ui.progress(&DownloadProgress {
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
        ui.progress(&event);
        assert_eq!(bar.fraction(), 0.35);
        assert!(bar.text().unwrap().contains("7 of 20"));
        assert!(status.text().starts_with("Current file:"));
        capture(&window, &format!("download-progress-{width}"));
        assert!(window.width() <= width + 10);
        event.current.as_mut().unwrap().stage = DownloadStage::Uploading;
        ui.progress(&event);
        assert_eq!(row.title(), "Uploading new file");
        capture(&window, &format!("upload-progress-{width}"));
        event.current.as_mut().unwrap().stage = DownloadStage::Verifying;
        ui.progress(&event);
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
                    result: None,
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
        assert!(menu.is_sensitive());
        assert_eq!(start.label().as_deref(), Some("Synchronize Now"));
        window.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
