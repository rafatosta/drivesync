//! Restore current sessions, without showing a connection-history list on Welcome.
use adw::{gtk, prelude::*};
use drivesync_engine::{DriveSyncEngine, RestoredAccount};
use std::{cell::RefCell, rc::Rc, sync::Arc};

pub fn build(
    engine: Arc<DriveSyncEngine>,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    connect: &gtk::Button,
) -> gtk::Box {
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
    panel.set_halign(gtk::Align::Center);
    let status = gtk::Label::builder()
        .label("Checking your connection…")
        .wrap(true)
        .max_width_chars(48)
        .build();
    let spinner = gtk::Spinner::new();
    let retry = gtk::Button::with_label("Try Again");
    retry.set_visible(false);
    let open = gtk::Button::with_label("Open Dashboard");
    open.set_visible(false);
    open.add_css_class("flat");
    panel.append(&spinner);
    panel.append(&status);
    panel.append(&retry);
    panel.append(&open);
    let snapshot = Rc::new(RefCell::new(Vec::<RestoredAccount>::new()));
    let weak_nav = navigation.downgrade();
    let weak_toasts = toasts.downgrade();
    let state = snapshot.clone();
    let page_engine = engine.clone();
    open.connect_clicked(move |_| {
        if let (Some(nav), Some(toasts)) = (weak_nav.upgrade(), weak_toasts.upgrade()) {
            nav.push(&crate::dashboard::build(
                page_engine.clone(),
                state.borrow().clone(),
                &nav,
                &toasts,
            ));
        }
    });
    let controls = (panel.clone(), status, spinner, retry.clone(), open);
    let weak_nav = navigation.downgrade();
    let weak_toasts = toasts.downgrade();
    let button = connect.downgrade();
    let refresh = Rc::new(move |auto_open| {
        let (Some(nav), Some(toasts), Some(connect)) =
            (weak_nav.upgrade(), weak_toasts.upgrade(), button.upgrade())
        else {
            return;
        };
        if !connect.is_sensitive() {
            return;
        }
        connect.set_sensitive(false);
        let (panel, status, spinner, retry, open) = controls.clone();
        panel.set_visible(true);
        status.set_label("Checking your connection…");
        retry.set_visible(false);
        open.set_visible(false);
        spinner.set_visible(true);
        spinner.start();
        let worker = engine.clone();
        let page_engine = engine.clone();
        let weak_nav = nav.downgrade();
        let weak_toasts = toasts.downgrade();
        let snapshot = snapshot.clone();
        crate::flow::work(
            move || worker.restore_accounts(),
            move |result| {
                connect.set_sensitive(true);
                spinner.stop();
                spinner.set_visible(false);
                match result {
                    Err(error) => {
                        status.set_label(&error.to_string());
                        retry.set_visible(true);
                    }
                    Ok(accounts) => {
                        panel.set_visible(!accounts.is_empty());
                        status.set_label("");
                        open.set_visible(!accounts.is_empty());
                        *snapshot.borrow_mut() = accounts.clone();
                        if auto_open {
                            if let (Some(nav), Some(toasts)) =
                                (weak_nav.upgrade(), weak_toasts.upgrade())
                            {
                                match crate::flow::initial_destination(&accounts) {
                                    crate::flow::InitialDestination::Dashboard => {
                                        nav.push(&crate::dashboard::build(
                                            page_engine,
                                            accounts,
                                            &nav,
                                            &toasts,
                                        ))
                                    }
                                    crate::flow::InitialDestination::FolderSetup => {
                                        nav.push(&crate::flow::account_page(
                                            page_engine,
                                            accounts[0].clone(),
                                            &nav,
                                            &toasts,
                                        ))
                                    }
                                    crate::flow::InitialDestination::Accounts => {}
                                }
                            }
                        }
                    }
                }
            },
        );
    });
    let retry_refresh = Rc::downgrade(&refresh);
    retry.connect_clicked(move |_| {
        if let Some(refresh) = retry_refresh.upgrade() {
            refresh(false);
        }
    });
    let actions = gtk::gio::SimpleActionGroup::new();
    let action = gtk::gio::SimpleAction::new("refresh", None);
    let action_refresh = refresh.clone();
    action.connect_activate(move |_, _| action_refresh(false));
    actions.add_action(&action);
    navigation.insert_action_group("accounts", Some(&actions));
    refresh(true);
    panel
}
