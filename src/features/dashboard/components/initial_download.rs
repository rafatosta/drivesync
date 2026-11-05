//! Runs initial materialization when a folder pair is available.
use adw::{gtk, prelude::*};
use drivesync_engine::{AuthError, DriveSyncEngine, EngineError, RestoredAccount};
use gettextrs::gettext;
use std::{cell::Cell, rc::Rc, sync::Arc, time::Duration};

pub(in crate::features::dashboard) fn build(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    transfers: super::transfers::TransferFeed,
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
    let (running, changed) = (busy.clone(), configuration_changed.clone());
    button.connect_clicked(move |button| {
        let Some(status) = weak_status.upgrade() else {
            return;
        };
        if running.replace(true) {
            return;
        }
        changed.set(false);
        let current = attempt.get().wrapping_add(1);
        attempt.set(current);
        let attempt = attempt.clone();
        button.set_visible(false);
        status.remove_css_class("error");
        status.set_label(&gettext("Checking initial download…"));

        let hold = button.root().and_downcast::<gtk::ApplicationWindow>()
            .and_then(|window| window.application()).map(|app| app.hold());
        let (engine, account) = (engine.clone(), account.clone());
        let (transfers, label) = (transfers.clone(), label.clone());
        let (button, status) = (button.downgrade(), status.downgrade());
        let (running, changed) = (running.clone(), changed.clone());
        crate::core::worker::work(
            move || engine.initial_download_with_progress(&account, |path, progress| {
                transfers.report(&account, &label, path, progress);
            }),
            move |result| {
                let _hold = hold;
                running.set(false);
                let (Some(button), Some(status)) = (button.upgrade(), status.upgrade()) else {
                    return;
                };
                if changed.get() {
                    button.emit_clicked();
                    return;
                }
                match result {
                    Ok(_) => status.set_label(&gettext("Initial download complete.")),
                    Err(error) => {
                        status.add_css_class("error");
                        if matches!(error, EngineError::DownloadConflict | EngineError::DownloadConflictAt(_)) {
                            status.set_label(&format!("{}\n{}", error, gettext("Resolve the conflict while keeping a copy of your files, then retry.")));
                            button.set_visible(true);
                        } else {
                            status.set_label(&error.to_string());
                        }
                        if retryable(&error) {
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
    let weak_group = group.downgrade();
    let weak_button = button.downgrade();
    let on_saved = move || {
        if let (Some(group), Some(button)) = (weak_group.upgrade(), weak_button.upgrade()) {
            group.set_visible(true);
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
