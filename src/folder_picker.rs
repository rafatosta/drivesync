use adw::{gtk, prelude::*};
use drivesync_engine::{AccountId, DriveFolder, DriveSyncEngine};
use std::{cell::RefCell, rc::Rc, sync::Arc};
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
pub fn show(
    engine: Arc<DriveSyncEngine>,
    account: AccountId,
    parent: Option<&gtk::Window>,
    done: impl FnOnce(DriveFolder) + 'static,
) {
    let window = adw::Window::builder()
        .title("Choose Google Drive Folder")
        .modal(true)
        .default_width(640)
        .default_height(600)
        .build();
    window.set_transient_for(parent);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let content = crate::flow::content();
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let up = gtk::Button::with_label("Up");
    let retry = gtk::Button::with_label("Reload");
    controls.append(&up);
    controls.append(&retry);
    content.append(&controls);
    let status = gtk::Label::builder().wrap(true).xalign(0.0).build();
    content.append(&status);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.append(&list);
    let more = gtk::Button::with_label("Load More Folders");
    more.set_visible(false);
    content.append(&more);
    let choose = gtk::Button::with_label("Select This Folder");
    choose.add_css_class("suggested-action");
    content.append(&choose);
    toolbar.set_content(Some(&crate::flow::scroll(&content, 620)));
    window.set_content(Some(&toolbar));
    let picker = Rc::new(Picker {
        engine,
        account,
        window: window.downgrade(),
        stack: RefCell::new(vec![DriveFolder {
            id: "root".into(),
            name: "My Drive".into(),
        }]),
        next: RefCell::new(None),
        list,
        status,
        choose,
        up,
        more,
        retry,
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
