//! UI-001: restore saved identities on a worker, without opening a browser.
use adw::{gtk, prelude::*};
use drivesync_engine::DriveSyncEngine;
use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

pub fn build(
    engine: Arc<DriveSyncEngine>,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    connect: &gtk::Button,
) -> gtk::Box {
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let status = gtk::Label::builder()
        .wrap(true)
        .label("Checking saved accounts…")
        .build();
    let spinner = gtk::Spinner::new();
    let rows = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let retry = gtk::Button::with_label("Check Saved Accounts Again");
    panel.append(&status);
    panel.append(&spinner);
    panel.append(&rows);
    panel.append(&retry);
    let retry_engine = engine.clone();
    let retry_status = status.clone();
    let retry_spinner = spinner.clone();
    let retry_rows = rows.clone();
    let retry_connect = connect.clone();
    let weak_navigation = navigation.downgrade();
    let weak_toasts = toasts.downgrade();
    retry.connect_clicked(move |button| {
        if !retry_connect.is_sensitive() {
            return;
        }
        if let (Some(navigation), Some(toasts)) = (weak_navigation.upgrade(), weak_toasts.upgrade())
        {
            start(
                retry_engine.clone(),
                &navigation,
                &toasts,
                &retry_connect,
                &retry_status,
                &retry_spinner,
                &retry_rows,
                button,
                false,
            );
        }
    });
    start(
        engine, navigation, toasts, connect, &status, &spinner, &rows, &retry, true,
    );
    panel
}

#[allow(clippy::too_many_arguments)]
fn start(
    engine: Arc<DriveSyncEngine>,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    connect: &gtk::Button,
    status: &gtk::Label,
    spinner: &gtk::Spinner,
    rows: &gtk::Box,
    retry: &gtk::Button,
    auto_open: bool,
) {
    connect.set_sensitive(false);
    retry.set_sensitive(false);
    status.set_label("Checking saved accounts…");
    spinner.set_visible(true);
    spinner.start();
    while let Some(child) = rows.first_child() {
        rows.remove(&child);
    }
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(engine.restore_accounts());
    });
    let navigation = navigation.downgrade();
    let toasts = toasts.downgrade();
    let connect = connect.clone();
    let status = status.clone();
    let spinner = spinner.clone();
    let rows = rows.clone();
    let retry = retry.clone();
    gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return gtk::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => {
                status.set_label("Could not finish checking saved accounts. Please try again.");
                spinner.stop();
                spinner.set_visible(false);
                connect.set_sensitive(true);
                retry.set_sensitive(true);
                return gtk::glib::ControlFlow::Break;
            }
        };
        spinner.stop();
        spinner.set_visible(false);
        connect.set_sensitive(true);
        retry.set_sensitive(true);
        match result {
            Err(error) => status.set_label(&error.to_string()),
            Ok(accounts) => {
                status.set_label(if accounts.is_empty() {
                    ""
                } else {
                    "Saved accounts"
                });
                let single = accounts.len() == 1;
                for saved in accounts {
                    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
                    let name = saved
                        .account
                        .email
                        .as_deref()
                        .unwrap_or(&saved.account.provider_subject);
                    row.append(
                        &gtk::Label::builder()
                            .label(name)
                            .selectable(true)
                            .wrap(true)
                            .build(),
                    );
                    if let Some(error) = saved.error {
                        row.append(
                            &gtk::Label::builder()
                                .label(error.to_string())
                                .wrap(true)
                                .build(),
                        );
                        let reconnect = gtk::Button::with_label("Reconnect Google Account");
                        let connect = connect.downgrade();
                        reconnect.connect_clicked(move |_| {
                            if let Some(connect) = connect.upgrade() {
                                connect.emit_clicked();
                            }
                        });
                        row.append(&reconnect);
                    } else {
                        let open = gtk::Button::with_label("Open Connected Account");
                        let account = saved.account;
                        if auto_open && single {
                            if let (Some(navigation), Some(toasts)) =
                                (navigation.upgrade(), toasts.upgrade())
                            {
                                navigation.push(&crate::account::build(&account, &toasts));
                            }
                        }
                        let navigation = navigation.clone();
                        let toasts = toasts.clone();
                        open.connect_clicked(move |_| {
                            if let (Some(navigation), Some(toasts)) =
                                (navigation.upgrade(), toasts.upgrade())
                            {
                                navigation.push(&crate::account::build(&account, &toasts));
                            }
                        });
                        row.append(&open);
                    }
                    rows.append(&row);
                }
            }
        }
        gtk::glib::ControlFlow::Break
    });
}
