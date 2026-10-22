//! Browsing state, Drive pagination and dialog signal handlers.
//!
//! All widget changes run on the GTK main context; blocking folder listing
//! is delegated through `crate::flow::work`.
use super::view;
use adw::{gtk, prelude::*};
use drivesync_engine::{AccountId, DriveFolder, DriveSyncEngine};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Callback invoked once with the folder selected by the user.
type FolderSelected = Box<dyn FnOnce(DriveFolder)>;
struct Picker {
    engine: Arc<DriveSyncEngine>,
    account: AccountId,
    window: gtk::glib::WeakRef<adw::Window>,
    stack: RefCell<Vec<DriveFolder>>,
    next: RefCell<Option<String>>,
    list: gtk::Box,
    status: gtk::Label,
    choose: gtk::Button,
    up: gtk::Button,
    more: gtk::Button,
    retry: gtk::Button,
    done: RefCell<Option<FolderSelected>>,
}


/// Presents the folder picker for one Google account.
///
/// The callback is invoked only when the user explicitly selects a folder.
/// Closing the dialog without choosing does not invoke it.
pub fn show(
    engine: Arc<DriveSyncEngine>,
    account: AccountId,
    parent: Option<&gtk::Window>,
    done: impl FnOnce(DriveFolder) + 'static,
) {
    let view = view::build(parent);
    let window = view.window.clone();
    let picker = Rc::new(Picker {
        engine,
        account,
        window: window.downgrade(),
        stack: RefCell::new(vec![DriveFolder {
            id: "root".into(),
            name: "My Drive".into(),
        }]),
        next: RefCell::new(None),
        list: view.list,
        status: view.status,
        choose: view.choose,
        up: view.up,
        more: view.more,
        retry: view.retry,
        done: RefCell::new(Some(Box::new(done))),
    });
    let weak = Rc::downgrade(&picker);
    picker.up.connect_clicked(move |_| {
        if let Some(p) = weak.upgrade() {
            if p.stack.borrow().len() > 1 {
                p.stack.borrow_mut().pop();
            }
            load(&p, false);
        }
    });
    let weak = Rc::downgrade(&picker);
    picker.retry.connect_clicked(move |_| {
        if let Some(p) = weak.upgrade() {
            load(&p, false);
        }
    });
    let weak = Rc::downgrade(&picker);
    picker.more.connect_clicked(move |_| {
        if let Some(p) = weak.upgrade() {
            load(&p, true);
        }
    });
    let weak = Rc::downgrade(&picker);
    picker.choose.connect_clicked(move |_| {
        if let Some(p) = weak.upgrade() {
            let selected = p.stack.borrow().last().cloned();
            if let (Some(done), Some(folder)) = (p.done.borrow_mut().take(), selected) {
                done(folder);
            }
            if let Some(window) = p.window.upgrade() {
                window.close();
            }
        }
    });
    load(&picker, false);
    let lifetime = RefCell::new(Some(picker));
    window.connect_close_request(move |_| {
        lifetime.borrow_mut().take();
        gtk::glib::Propagation::Proceed
    });
    window.present();
}

/// Requests a page of child folders for the current browsing location.
///
/// The next-page token is used only for explicit load-more requests;
/// errors are shown in the dialog without exposing provider credentials.
fn load(picker: &Rc<Picker>, more: bool) {
    let current = picker.stack.borrow().last().unwrap().clone();
    let page = if more {
        picker.next.borrow().clone()
    } else {
        None
    };
    if !more {
        while let Some(child) = picker.list.first_child() {
            picker.list.remove(&child);
        }
    }
    picker.choose.set_sensitive(false);
    picker.up.set_sensitive(false);
    picker.more.set_sensitive(false);
    picker.retry.set_sensitive(false);
    picker.list.set_sensitive(false);
    picker
        .status
        .set_label("Loading folders from Google Drive…");
    let engine = picker.engine.clone();
    let account = picker.account.clone();
    let weak = Rc::downgrade(picker);
    crate::flow::work(
        move || engine.drive_folders(&account, &current.id, page.as_deref()),
        move |result| {
            let Some(picker) = weak.upgrade() else {
                return;
            };
            picker.up.set_sensitive(picker.stack.borrow().len() > 1);
            picker.retry.set_sensitive(true);
            picker.list.set_sensitive(true);
            match result {
                Err(error) => {
                    picker.status.set_label(&error.to_string());
                    picker.more.set_visible(false);
                }
                Ok(page) => {
                    let path = picker
                        .stack
                        .borrow()
                        .iter()
                        .map(|f| f.name.as_str())
                        .collect::<Vec<_>>()
                        .join(" / ");
                    picker.status.set_label(&path);
                    picker.choose.set_sensitive(true);
                    for folder in page.folders {
                        let button = gtk::Button::builder()
                            .label(&folder.name)
                            .halign(gtk::Align::Fill)
                            .build();
                        let weak = Rc::downgrade(&picker);
                        button.connect_clicked(move |_| {
                            if let Some(p) = weak.upgrade() {
                                p.stack.borrow_mut().push(folder.clone());
                                load(&p, false);
                            }
                        });
                        picker.list.append(&button);
                    }
                    if picker.list.first_child().is_none() {
                        picker.list.append(&gtk::Label::new(Some(
                            "This folder has no subfolders. You can select it here.",
                        )));
                    }
                    picker.more.set_visible(page.next_page.is_some());
                    picker.more.set_sensitive(true);
                    *picker.next.borrow_mut() = page.next_page;
                }
            }
        },
    );
}
