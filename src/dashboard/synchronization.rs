//! Manual download cycles on workers, using native progress and cancellation UI.
use adw::{gtk, prelude::*};
use drivesync_engine::{
    Cancellation, DownloadProgress, DownloadReport, DriveSyncEngine, EngineError, RestoredAccount,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{mpsc, Arc},
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
    issues: gtk::glib::WeakRef<gtk::Label>,
}
impl Widgets {
    fn progress(&self, event: &DownloadProgress) {
        if let Some(bar) = self.progress.upgrade() {
            bar.set_fraction(if event.total == 0 {
                0.0
            } else {
                (event.received as f64 / event.total as f64).clamp(0.0, 1.0)
            });
        }
        if let Some(row) = self.row.upgrade() {
            row.set_title("Downloading");
            row.set_subtitle(&event.relative_path.to_string_lossy());
        }
        if let Some(status) = self.status.upgrade() {
            status.set_label(&format!(
                "{} / {} received · verifying before installation",
                gtk::glib::format_size(event.received),
                gtk::glib::format_size(event.total)
            ));
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
        match result {
            Ok(report) => {
                let attention = !report.issues.is_empty() || report.skipped > 0;
                if let Some(row) = self.row.upgrade() {
                    row.set_title(if attention {
                        "Cycle finished with pending items"
                    } else {
                        "Download cycle completed"
                    });
                    row.set_subtitle("Manual mode · existing local files are preserved");
                }
                if let Some(status) = self.status.upgrade() {
                    status.set_label(&summary(&report));
                }
                if let Some(issues) = self.issues.upgrade() {
                    let mut messages: Vec<_> = report
                        .issues
                        .iter()
                        .take(5)
                        .map(|issue| {
                            format!(
                                "{}: {}",
                                issue
                                    .relative_path
                                    .as_ref()
                                    .map(|p| p.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| "Item".into()),
                                issue.error
                            )
                        })
                        .collect();
                    if report.issues.len() > 5 {
                        messages.push(format!(
                            "{} additional items need attention.",
                            report.issues.len() - 5
                        ));
                    }
                    if report.skipped > 0 {
                        messages.push("Unsupported items, including Google documents and shortcuts, were skipped.".into());
                    }
                    issues.set_label(&messages.join("\n\n"));
                    issues.set_visible(!messages.is_empty());
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
                if let Some(issues) = self.issues.upgrade() {
                    issues.set_label(&error.to_string());
                    issues.set_visible(!cancelled);
                }
            }
        }
    }
}
fn summary(report: &DownloadReport) -> String {
    format!(
        "{} downloaded · {} recovered · {} unchanged · {} skipped · {} need attention",
        report.downloaded,
        report.recovered,
        report.unchanged,
        report.skipped,
        report.issues.len()
    )
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
    let note=gtk::Label::builder().label("Manual download preview. Existing files are preserved. Automatic updates and Google documents are not supported yet.").wrap(true).xalign(0.0).css_classes(["dim-label"]).build();
    footer.append(&note);
    let progress = gtk::ProgressBar::new();
    progress.set_widget_name("download-progress");
    progress.set_visible(false);
    footer.append(&progress);
    footer.append(status);
    let issues = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .visible(false)
        .css_classes(["error"])
        .build();
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
    };
    let running: Rc<RefCell<Option<Cancellation>>> = Rc::new(RefCell::new(None));
    let id = saved.account.id.clone();
    start.connect_clicked(move |button| {
        if let Some(cancel) = running.borrow().as_ref() {
            cancel.cancel();
            button.set_sensitive(false);
            button.set_label("Cancelling…");
            return;
        }
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
            row.set_title("Preparing downloads");
            row.set_subtitle("Scanning Google Drive and checking local files…");
        }
        if let Some(status) = ui.status.upgrade() {
            status.set_label("Preparing a fresh download plan…");
        }
        if let Some(issues) = ui.issues.upgrade() {
            issues.set_visible(false);
        }
        if let Some(bar) = ui.progress.upgrade() {
            bar.set_fraction(0.0);
            bar.set_visible(true);
        }
        let (send, receive) = mpsc::sync_channel(1);
        let worker = engine.clone();
        let account = id.clone();
        let worker_cancel = cancel.clone();
        let finished = Rc::new(Cell::new(false));
        let progress_finished = finished.clone();
        let updates = ui.clone();
        let active = running.clone();
        let completion = ui.clone();
        let completion_active = running.clone();
        // Bounded progress messages never block a disk/network worker.
        gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
            if progress_finished.get() || active.borrow().is_none() {
                return gtk::glib::ControlFlow::Break;
            }
            if updates.start.upgrade().is_none() {
                cancel.cancel();
                return gtk::glib::ControlFlow::Break;
            }
            if let Ok(event) = receive.try_recv() {
                updates.progress(&event);
            }
            gtk::glib::ControlFlow::Continue
        });
        crate::flow::work(
            move || {
                worker.download_once(&account, &worker_cancel, |event| {
                    let _ = send.try_send(event);
                })
            },
            move |result| {
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

#[cfg(test)]
mod tests {
    use super::*;
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
            "2 downloaded · 0 recovered · 0 unchanged · 1 skipped · 1 need attention"
        );
    }
}
