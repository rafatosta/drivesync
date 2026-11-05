//! Runs initial materialization when a folder pair is available.
use adw::{gtk, prelude::*};
use drivesync_engine::{AuthError, DriveSyncEngine, EngineError, RestoredAccount};
use gettextrs::gettext;
use std::{
    cell::Cell,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

pub(in crate::features::dashboard) fn build(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    transfers: super::transfers::TransferFeed,
    refresh_activities: impl Fn() + 'static,
) -> (gtk::Box, impl Fn() + 'static) {
    let group = gtk::Box::new(gtk::Orientation::Vertical, 8);
    group.set_visible(saved.folder.is_some());
    let status = gtk::Label::builder().wrap(true).xalign(0.0).build();
    group.append(&status);
    let button = gtk::Button::with_label(&gettext("Retry after resolving the conflict"));
    button.set_halign(gtk::Align::Start);
    button.set_visible(false);
    group.append(&button);

    let busy = Rc::new(Cell::new(false));
    let configuration_changed = Rc::new(Cell::new(false));
    let account = saved.account.id.clone();
    let label = saved
        .account
        .email
        .clone()
        .unwrap_or_else(|| account.0.clone());
    let weak_status = status.downgrade();
    let attempt = Rc::new(Cell::new(0_u64));
    let periodic = Rc::new(Cell::new(false));
    let enabled = Rc::new(Cell::new(saved.folder.is_some() && saved.error.is_none()));
    let last_check = Rc::new(Cell::new(Instant::now()));
    let backoff = Rc::new(Cell::new(1_u64));
    let retry_factor = backoff.clone();
    let (check_remote, checked_at) = (periodic.clone(), last_check.clone());
    let (running, changed) = (busy.clone(), configuration_changed.clone());
    let refresh_activities = Rc::new(refresh_activities);
    button.connect_clicked(move |button| {
        let Some(status) = weak_status.upgrade() else {
            return;
        };
        if running.replace(true) {
            return;
        }
        changed.set(false);
        let is_periodic = check_remote.get();
        checked_at.set(Instant::now());
        let current = attempt.get().wrapping_add(1);
        attempt.set(current);
        let attempt = attempt.clone();
        button.set_visible(false);
        status.remove_css_class("error");
        status.set_label(&gettext(if is_periodic {
            "Checking for remote changes…"
        } else {
            "Checking initial download…"
        }));

        let hold = button.root().and_downcast::<gtk::ApplicationWindow>()
            .and_then(|window| window.application()).map(|app| app.hold());
        let (engine, account) = (engine.clone(), account.clone());
        let (transfers, label) = (transfers.clone(), label.clone());
        let (button, status) = (button.downgrade(), status.downgrade());
        let (running, changed) = (running.clone(), changed.clone());
        let check_remote = check_remote.clone();
        let checked_at = checked_at.clone();
        let retry_factor = retry_factor.clone();
        let refresh_activities = refresh_activities.clone();
        let applying = Arc::new(std::sync::atomic::AtomicBool::new(false));
        if is_periodic {
            let (applying, running, status, attempt) =
                (applying.clone(), running.clone(), status.clone(), attempt.clone());
            gtk::glib::timeout_add_local(Duration::from_millis(150), move || {
                use std::sync::atomic::Ordering;
                if !running.get() || attempt.get() != current {
                    return gtk::glib::ControlFlow::Break;
                }
                if applying.load(Ordering::Relaxed) {
                    if let Some(status) = status.upgrade() {
                        status.set_label(&gettext("Applying changes to the local folder…"));
                    }
                    return gtk::glib::ControlFlow::Break;
                }
                gtk::glib::ControlFlow::Continue
            });
        }
        crate::core::worker::work(
            move || {
                let report = |path: &std::path::Path, progress| transfers.report(&account, &label, path, progress);
                if is_periodic {
                    engine.poll_remote_downloads(&account, report, || {
                        applying.store(true, std::sync::atomic::Ordering::Relaxed);
                    })
                } else {
                    engine.initial_download_with_progress(&account, report).map(|_| false)
                }
            },
            move |result| {
                let _hold = hold;
                refresh_activities();
                running.set(false);
                checked_at.set(Instant::now());
                let (Some(button), Some(status)) = (button.upgrade(), status.upgrade()) else {
                    return;
                };
                if changed.get() {
                    check_remote.set(false);
                    button.emit_clicked();
                    return;
                }
                if is_periodic && result.as_ref().is_err_and(retryable) {
                    retry_factor.set((retry_factor.get() * 2).min(8));
                } else {
                    retry_factor.set(1);
                }
                if !is_periodic && result.is_ok() {
                    check_remote.set(true);
                    button.emit_clicked();
                    return;
                }
                match result {
                    Ok(unresolved) => status.set_label(&gettext(if unresolved {
                        "Remote changes are pending review."
                    } else if is_periodic {
                        "Remote check complete."
                    } else {
                        "Initial download complete."
                    })),
                    Err(error) => {
                        status.add_css_class("error");
                        if matches!(error, EngineError::DownloadConflict | EngineError::DownloadConflictAt(_)) {
                            status.set_label(&format!("{}\n{}", error, gettext("Resolve the conflict while keeping a copy of your files, then retry.")));
                            button.set_visible(true);
                        } else {
                            status.set_label(&error.to_string());
                        }
                        if !is_periodic && retryable(&error) {
                            let button = button.downgrade();
                            gtk::glib::timeout_add_local_once(Duration::from_secs(60), move || {
                                if let Some(button) = button.upgrade().filter(|_| attempt.get() == current) {
                                    button.emit_clicked();
                                }
                            });
                        }
                    }
                }
            },
        );
    });

    if saved.folder.is_some() && saved.error.is_none() {
        let button = button.downgrade();
        gtk::glib::idle_add_local_once(move || {
            if let Some(button) = button.upgrade() {
                button.emit_clicked();
            }
        });
    }
    let timer_button = button.downgrade();
    let (running, ready, mode, last) = (
        busy.clone(),
        enabled.clone(),
        periodic.clone(),
        last_check.clone(),
    );
    gtk::glib::timeout_add_local(Duration::from_secs(5), move || {
        let Some(button) = timer_button.upgrade() else {
            return gtk::glib::ControlFlow::Break;
        };
        if ready.get() && !running.get() && !button.is_visible() {
            let minutes = crate::features::preferences::Preferences::load().remote_check_minutes;
            if last.get().elapsed() >= Duration::from_secs(u64::from(minutes) * 60 * backoff.get())
            {
                mode.set(true);
                button.emit_clicked();
            }
        }
        gtk::glib::ControlFlow::Continue
    });
    let weak_group = group.downgrade();
    let weak_button = button.downgrade();
    let on_saved = move || {
        if let (Some(group), Some(button)) = (weak_group.upgrade(), weak_button.upgrade()) {
            group.set_visible(true);
            enabled.set(true);
            periodic.set(false);
            if busy.get() {
                configuration_changed.set(true);
            } else {
                button.emit_clicked();
            }
        }
    };
    (group, on_saved)
}

fn retryable(error: &EngineError) -> bool {
    matches!(
        error,
        EngineError::DownloadBusy
            | EngineError::RemoteRateLimited
            | EngineError::Authentication(
                AuthError::Network | AuthError::ProviderUnavailable | AuthError::TimedOut
            )
    )
}
