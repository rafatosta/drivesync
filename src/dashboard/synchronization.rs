//! Manual download cycles on workers, using native progress and cancellation UI.
use adw::{gtk, prelude::*};
use drivesync_engine::{
    Cancellation, DownloadProgress, DownloadReport, DownloadStage, DriveSyncEngine, EngineError,
    RestoredAccount,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

/// Weak handles let navigation dispose account pages while cancellation propagates.
#[derive(Clone)]
struct Widgets {
    start: gtk::glib::WeakRef<gtk::Button>,
    folders: gtk::glib::WeakRef<gtk::Box>,
    menu: gtk::glib::WeakRef<gtk::MenuButton>,
    row: gtk::glib::WeakRef<adw::ActionRow>,
    status: gtk::glib::WeakRef<gtk::Label>,
    progress: gtk::glib::WeakRef<gtk::ProgressBar>,
    issues: gtk::glib::WeakRef<gtk::Box>,
    skipped: gtk::glib::WeakRef<gtk::Label>,
    preserved: gtk::glib::WeakRef<gtk::Label>,
    spinner: gtk::glib::WeakRef<gtk::Spinner>,
}
impl Widgets {
    fn progress(&self, event: &DownloadProgress) {
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
    fn finish(&self, result: Result<DownloadReport, EngineError>) {
        if let Some(start) = self.start.upgrade() {
            start.set_label("Start Synchronization");
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
        match result {
            Ok(report) => {
                let attention = !report.issues.is_empty();
                if let Some(row) = self.row.upgrade() {
                    row.set_title(if attention {
                        "Cycle finished with pending items"
                    } else {
                        "Download cycle completed"
                    });
                    row.set_subtitle("Download only · local edits are preserved");
                }
                if let Some(status) = self.status.upgrade() {
                    status.set_label(&summary(&report));
                }
                if let Some(preserved) = self.preserved.upgrade() {
                    preserved.set_label(&preserved_text(&report));
                    preserved.set_visible(!report.preserved_copies.is_empty());
                }
                self.show_issues(&report);
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
                if error == EngineError::InvalidFolder {
                    if let Some(start) = self.start.upgrade() {
                        start.set_sensitive(false);
                    }
                }
                let cancelled = matches!(
                    error,
                    EngineError::Authentication(drivesync_engine::AuthError::Cancelled)
                );
                if let Some(row) = self.row.upgrade() {
                    row.set_title(if cancelled {
                        "Cancelled"
                    } else {
                        "Download needs attention"
                    });
                    row.set_subtitle(
                        "Completed files are preserved; the next cycle rechecks them.",
                    );
                }
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
                            error,
                        }]
                    },
                    ..Default::default()
                };
                self.show_issues(&report);
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
        DownloadStage::Downloading => match item.total {
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
    format!(
        "{} downloaded · {} updated · {} recovered · {} unchanged · {} skipped · {} need attention",
        report.downloaded,
        report.updated,
        report.recovered,
        report.unchanged,
        report.skipped,
        report.issues.len()
    )
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
/// Binds the configured account to a single manual worker. The same button
/// cancels active work; folder/account actions stay unavailable until it stops.
pub(super) fn build(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    folders: &gtk::Box,
    start: &gtk::Button,
    row: &adw::ActionRow,
    status: &gtk::Label,
    menu: &gtk::MenuButton,
) -> gtk::Box {
    let footer = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let note = gtk::Label::builder()
        .label("Manual download · up-to-date files are not downloaded again. Local edits are preserved; updates keep a previous copy.")
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
    footer.append(start);
    let ui = Widgets {
        start: start.downgrade(),
        folders: folders.downgrade(),
        menu: menu.downgrade(),
        row: row.downgrade(),
        status: status.downgrade(),
        progress: progress.downgrade(),
        issues: issues.downgrade(),
        skipped: skipped.downgrade(),
        preserved: preserved.downgrade(),
        spinner: spinner.downgrade(),
    };
    let running: Rc<RefCell<Option<Cancellation>>> = Rc::new(RefCell::new(None));
    let id = saved.account.id.clone();
    start.connect_clicked(move |button| {
        if let Some(cancel) = running.borrow().as_ref() {
            cancel.cancel();
            button.set_sensitive(false);
            button.set_label("Cancelling…");
            if let Some(row) = ui.row.upgrade() {
                row.set_title("Cancelling downloads");
                row.set_subtitle("Waiting for the current operation to stop safely…");
            }
            return;
        }
        crate::background::set_activity(true);
        let cancel = Cancellation::new();
        *running.borrow_mut() = Some(cancel.clone());
        button.set_label("Cancel");
        button.remove_css_class("suggested-action");
        if let Some(folders) = ui.folders.upgrade() {
            folders.set_sensitive(false);
        }
        if let Some(menu) = ui.menu.upgrade() {
            menu.set_sensitive(false);
        }
        if let Some(row) = ui.row.upgrade() {
            row.set_title("Checking for changes");
            row.set_subtitle("Checking Google Drive changes and existing local copies…");
        }
        if let Some(status) = ui.status.upgrade() {
            status.set_label("The first cycle also scans the selected folders.");
        }
        ui.clear_issues();
        if let Some(preserved) = ui.preserved.upgrade() {
            preserved.set_visible(false);
        }
        if let Some(skipped) = ui.skipped.upgrade() {
            skipped.set_visible(false);
        }
        if let Some(spinner) = ui.spinner.upgrade() {
            spinner.set_visible(true);
            spinner.start();
        }
        if let Some(bar) = ui.progress.upgrade() {
            bar.set_fraction(0.0);
            bar.set_visible(false);
        }
        let latest: Arc<Mutex<Option<DownloadProgress>>> = Arc::new(Mutex::new(None));
        let receive = latest.clone();
        let worker = engine.clone();
        let account = id.clone();
        let worker_cancel = cancel.clone();
        let finished = Rc::new(Cell::new(false));
        let progress_finished = finished.clone();
        let updates = ui.clone();
        let active = running.clone();
        let completion = ui.clone();
        let completion_active = running.clone();
        // A single replaceable snapshot keeps the newest file/stage without an event backlog.
        gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
            if progress_finished.get() || active.borrow().is_none() {
                return gtk::glib::ControlFlow::Break;
            }
            if updates.start.upgrade().is_none() {
                cancel.cancel();
                return gtk::glib::ControlFlow::Break;
            }
            if active
                .borrow()
                .as_ref()
                .is_some_and(Cancellation::is_cancelled)
            {
                return gtk::glib::ControlFlow::Continue;
            }
            let event = receive.lock().ok().and_then(|mut slot| slot.take());
            if let Some(event) = event {
                updates.progress(&event);
            }
            gtk::glib::ControlFlow::Continue
        });
        crate::flow::work(
            move || {
                worker.download_once(&account, &worker_cancel, |event| {
                    if let Ok(mut slot) = latest.lock() {
                        *slot = Some(event);
                    }
                })
            },
            move |result| {
                crate::background::set_activity(false);
                finished.set(true);
                *completion_active.borrow_mut() = None;
                completion.finish(result);
            },
        );
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
        folder: None,
    };
    let folders = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let start = gtk::Button::with_label("Start Synchronization");
    let row = adw::ActionRow::new();
    let status = gtk::Label::new(None);
    let menu = gtk::MenuButton::new();
    let footer = build(
        Arc::new(DriveSyncEngine::new()),
        &saved,
        &folders,
        &start,
        &row,
        &status,
        &menu,
    );
    start.emit_clicked();
    assert_eq!(start.label().as_deref(), Some("Cancel"));
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
            .is_some_and(|b| b.label().as_deref() == Some("Start Synchronization"))
            || count.get() > 100
        {
            finish.quit();
            gtk::glib::ControlFlow::Break
        } else {
            gtk::glib::ControlFlow::Continue
        }
    });
    wait.run();
    assert_eq!(row.title(), "Download needs attention");
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
        let start = gtk::Button::with_label("Cancel");
        start.set_halign(gtk::Align::End);
        let folders = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let menu = gtk::MenuButton::new();
        let row = adw::ActionRow::builder()
            .title("Synchronization")
            .use_markup(false)
            .build();
        let status = gtk::Label::builder().wrap(true).xalign(0.0).build();
        let footer = build(
            Arc::new(DriveSyncEngine::new()),
            &saved,
            &folders,
            &start,
            &row,
            &status,
            &menu,
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
            folders: folders.downgrade(),
            menu: menu.downgrade(),
            row: row.downgrade(),
            status: status.downgrade(),
            progress: bar.downgrade(),
            issues: issues.downgrade(),
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
        let mut event = DownloadProgress {
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
        event.current.as_mut().unwrap().stage = DownloadStage::Verifying;
        ui.progress(&event);
        assert_eq!(row.title(), "Verifying file integrity");
        assert_eq!(bar.fraction(), 0.35);
        ui.finish(Ok(DownloadReport {
            skipped: 1,
            ..Default::default()
        }));
        assert_eq!(row.title(), "Download cycle completed");
        assert!(!issues.is_visible());
        assert!(skipped.is_visible());
        assert!(!skipped.has_css_class("error"));
        ui.finish(Ok(DownloadReport {
            unchanged: 5,
            updated: 2,
            preserved_copies: vec!["Reports/DriveSync backup example.bak".into()],
            skipped: 1,
            issues: vec![DownloadIssue {
                relative_path: Some("Reports/Report.pdf".into()),
                error: EngineError::RemoteChanged,
            }],
            ..Default::default()
        }));
        assert!(preserved.is_visible());
        assert!(preserved.text().contains("DriveSync backup example.bak"));
        assert!(issues.is_visible());
        assert!(!bar.is_visible());
        assert!(!spinner.is_spinning());
        capture(&window, &format!("download-result-{width}"));
        assert!(window.width() <= width + 10);
        window.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            "2 downloaded · 0 updated · 0 recovered · 0 unchanged · 1 skipped · 1 need attention"
        );
    }
}
