//! Native opt-in for create-only uploads, including account-specific OAuth consent.
use adw::{gtk, prelude::*};
use drivesync_engine::{AuthError, Cancellation, DriveSyncEngine, EngineError, RestoredAccount};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};
fn present(row: &adw::SwitchRow, group: &adw::PreferencesGroup, active: bool, guard: &Cell<bool>) {
    guard.set(true);
    row.set_active(active);
    guard.set(false);
    group.set_description(Some(if active {"Downloads continue. Only new local files and folders are uploaded. Existing remote files are never replaced; deletions are not sent."} else {"Download only. Local files will not be uploaded to Google Drive."}));
    row.set_subtitle("Requires your permission. Existing files and deletions are not sent.");
}
pub(super) fn build(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    folders: &gtk::Box,
    group: &adw::PreferencesGroup,
) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title("Upload new files and folders")
        .subtitle("Download only until enabled")
        .build();
    row.set_subtitle_lines(0);
    row.set_title_lines(0);
    let guard = Rc::new(Cell::new(false));
    let busy = Rc::new(Cell::new(false));
    let job = crate::automatic::Controller::attach(engine.clone(), saved);
    let poll_job = job.clone();
    let weak = row.downgrade();
    let weak_group = group.downgrade();
    let read_engine = engine.clone();
    let read_id = saved.account.id.clone();
    let read_guard = guard.clone();
    let read_busy = busy.clone();
    let folder_controls = folders.downgrade();
    let mut last = None;
    let mut initial = true;
    gtk::glib::timeout_add_local(Duration::from_millis(250), move || {
        let (Some(row), Some(group)) = (weak.upgrade(), weak_group.upgrade()) else {
            return gtk::glib::ControlFlow::Break;
        };
        row.set_sensitive(
            !read_busy.get()
                && !poll_job.running()
                && poll_job.folder().is_some()
                && folder_controls.upgrade().is_some_and(|w| w.is_sensitive()),
        );
        let folder = poll_job.folder();
        if !read_busy.get() && (initial || folder != last) {
            initial = false;
            last = folder;
            read_busy.set(true);
            row.set_sensitive(false);
            let engine = read_engine.clone();
            let id = read_id.clone();
            let guard = read_guard.clone();
            let busy = read_busy.clone();
            let row = row.downgrade();
            crate::flow::work(
                move || engine.upload_enabled(&id),
                move |result| {
                    busy.set(false);
                    if let Some(row) = row.upgrade() {
                        match result {
                            Ok(active) => present(&row, &group, active, &guard),
                            Err(e) => row.set_subtitle(&e.to_string()),
                        }
                    }
                },
            );
        }
        gtk::glib::ControlFlow::Continue
    });
    let id = saved.account.id.clone();
    let group = group.downgrade();
    row.connect_active_notify(move |row| {
        if guard.get() {
            return;
        }
        let enabled = row.is_active();
        if busy.get() || job.running() {
            guard.set(true);
            row.set_active(!enabled);
            guard.set(false);
            return;
        }
        busy.set(true);
        row.set_sensitive(false);
        let cancel = Cancellation::new();
        let dialog = adw::Window::builder()
            .title("Google Drive permission")
            .default_width(420)
            .default_height(180)
            .modal(true)
            .build();
        if let Some(parent) = row.root().and_then(|r| r.downcast::<gtk::Window>().ok()) {
            dialog.set_transient_for(Some(&parent));
        }
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        let label = gtk::Label::builder()
            .label(if enabled {
                "Authorize upload for this account in your browser. Choose the same Google account."
            } else {
                "Switching to download only…"
            })
            .wrap(true)
            .build();
        content.append(&label);
        let stop = gtk::Button::with_label("Cancel");
        content.append(&stop);
        dialog.set_content(Some(&content));
        let c = cancel.clone();
        stop.connect_clicked(move |b| {
            c.cancel();
            b.set_sensitive(false);
        });
        let c = cancel.clone();
        dialog.connect_close_request(move |_| {
            c.cancel();
            gtk::glib::Propagation::Proceed
        });
        dialog.present();
        let (tx, rx) = mpsc::channel::<(String, mpsc::Sender<bool>)>();
        let worker = engine.clone();
        let id = id.clone();
        let worker_cancel = cancel.clone();
        let row = row.downgrade();
        let guard = guard.clone();
        let busy = busy.clone();
        let group = group.clone();
        crate::flow::work(
            move || {
                let folder = worker
                    .saved_folder(&id)?
                    .ok_or(EngineError::InvalidFolder)?;
                if enabled {
                    match worker.set_upload_enabled(&folder, true) {
                        Ok(()) => return Ok(folder),
                        Err(EngineError::Authentication(AuthError::MissingPermissions)) => {}
                        Err(e) => return Err(e),
                    }
                    worker.authorize_upload(&id, &worker_cancel, |url| {
                        let (ack, reply) = mpsc::channel();
                        tx.send((url.into(), ack))
                            .map_err(|_| AuthError::BrowserUnavailable)?;
                        let until = Instant::now() + Duration::from_secs(30);
                        loop {
                            if worker_cancel.is_cancelled() {
                                return Err(AuthError::Cancelled);
                            }
                            if Instant::now() >= until {
                                return Err(AuthError::BrowserUnavailable);
                            }
                            match reply.recv_timeout(Duration::from_millis(100)) {
                                Ok(true) => return Ok(()),
                                Ok(false) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                                    return Err(AuthError::BrowserUnavailable)
                                }
                                Err(_) => {}
                            }
                        }
                    })?;
                }
                if worker_cancel.is_cancelled() {
                    return Err(EngineError::Authentication(AuthError::Cancelled));
                }
                worker.set_upload_enabled(&folder, enabled)?;
                Ok(folder)
            },
            move |result| {
                busy.set(false);
                if let Ok(folder) = &result {
                    crate::automatic::configured(folder.clone());
                }
                dialog.close();
                if let Some(row) = row.upgrade() {
                    if let Some(group) = group.upgrade() {
                        present(
                            &row,
                            &group,
                            if result.is_ok() { enabled } else { !enabled },
                            &guard,
                        );
                    }
                    if let Err(error) = result {
                        row.set_subtitle(&error.to_string());
                    }
                }
            },
        );
        gtk::glib::timeout_add_local(Duration::from_millis(50), move || match rx.try_recv() {
            Ok((url, ack)) => {
                gtk::glib::spawn_future_local(async move {
                    let success = gtk::gio::AppInfo::launch_default_for_uri_future(
                        &url,
                        None::<&gtk::gio::AppLaunchContext>,
                    )
                    .await
                    .is_ok();
                    let _ = ack.send(success);
                });
                gtk::glib::ControlFlow::Continue
            }
            Err(mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => gtk::glib::ControlFlow::Break,
        });
    });
    row
}

/// Native presentation fixtures never authorize, persist or access an account.
#[cfg(test)]
pub(super) fn verify_presentations(mut capture: impl FnMut(&adw::Window, &str)) {
    for width in [360, 720] {
        let guard = Cell::new(false);
        let group = adw::PreferencesGroup::builder()
            .title("Synchronization")
            .build();
        let row = adw::SwitchRow::builder()
            .title("Upload new files and folders")
            .build();
        row.set_title_lines(0);
        row.set_subtitle_lines(0);
        group.add(&row);
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        content.append(&group);
        let window = adw::Window::builder()
            .default_width(width)
            .default_height(260)
            .content(&content)
            .build();
        window.present();
        for active in [false, true] {
            present(&row, &group, active, &guard);
            assert_eq!(row.is_active(), active);
            assert!(!guard.get());
            assert!(group.description().unwrap().contains(if active {
                "Only new local files"
            } else {
                "Download only"
            }));
            capture(
                &window,
                &format!(
                    "upload-mode-{}-{width}",
                    if active { "enabled" } else { "disabled" }
                ),
            );
            assert!(window.width() <= width + 10);
        }
        window.close();
    }
}
