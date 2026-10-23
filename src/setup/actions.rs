//! Folder selection, save and prefill interactions for the setup page.
//!
//! All GTK changes occur on the main thread. I/O and engine work run via
//! `crate::flow::work`, with failures reported in the screen feedback label.
use super::view::SetupView;
use adw::{gtk, prelude::*};
use drivesync_engine::{AccountSummary, DriveFolder, DriveSyncEngine};
use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};

/// Current, not-yet-saved choices for one setup page.
#[derive(Default)]
struct Selection {
    remote: Option<DriveFolder>,
    local: Option<PathBuf>,
}

/// Connects folder chooser, cancellation, save and saved-settings prefill.
///
/// The save button is enabled only when both local and remote selections
/// exist. The existing configuration is fetched asynchronously; errors are
/// shown without overwriting the user's files or performing synchronization.
pub(super) fn bind(
    engine: Arc<DriveSyncEngine>,
    account: AccountSummary,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    view: &SetupView,
) {
    let page = view.page.clone();
    let remote_row = view.remote_row.clone();
    let remote_button = view.remote_button.clone();
    let local_row = view.local_row.clone();
    let local_button = view.local_button.clone();
    let summary_remote = view.summary_remote.clone();
    let summary_local = view.summary_local.clone();
    let feedback = view.feedback.clone();
    let cancel = view.cancel.clone();
    let save = view.save.clone();
    let selection = Rc::new(RefCell::new(Selection::default()));
    {
        let selection = selection.clone();
        let save = save.clone();
        let row = remote_row.clone();
        let summary = summary_remote.clone();
        let worker = engine.clone();
        let account = account.clone();
        remote_button.connect_clicked(move |button| {
            let parent = button.root().and_downcast::<gtk::Window>();
            let selection = selection.clone();
            let save = save.clone();
            let row = row.clone();
            let summary = summary.clone();
            crate::folder_picker::show(
                worker.clone(),
                account.id.clone(),
                parent.as_ref(),
                move |folder| {
                    row.set_subtitle(&folder.name);
                    summary.set_subtitle(&folder.name);
                    selection.borrow_mut().remote = Some(folder);
                    save.set_sensitive(selection.borrow().local.is_some());
                },
            );
        });
    }
    {
        let selection = selection.clone();
        let save = save.clone();
        let row = local_row.clone();
        let summary = summary_local.clone();
        let feedback = feedback.clone();
        local_button.connect_clicked(move |button| {
            let parent = button.root().and_downcast::<gtk::Window>();
            let dialog = gtk::FileDialog::builder()
                .title("Choose Local Folder")
                .modal(true)
                .build();
            if let Some(path) = selection.borrow().local.as_ref() {
                dialog.set_initial_folder(Some(&gtk::gio::File::for_path(path)));
            }
            let selection = selection.clone();
            let save = save.clone();
            let row = row.clone();
            let summary = summary.clone();
            let feedback = feedback.clone();
            gtk::glib::spawn_future_local(async move {
                match dialog.select_folder_future(parent.as_ref()).await {
                    Ok(file) => {
                        if let Some(path) = file.path() {
                            row.set_subtitle(&path.to_string_lossy());
                            summary.set_subtitle(&path.to_string_lossy());
                            selection.borrow_mut().local = Some(path);
                            save.set_sensitive(selection.borrow().remote.is_some());
                            feedback.set_label("");
                        } else {
                            feedback.set_label("Choose a folder available on this computer.");
                        }
                    }
                    Err(error)
                        if error.matches(gtk::DialogError::Dismissed)
                            || error.matches(gtk::DialogError::Cancelled) => {}
                    Err(_) => {
                        feedback.set_label("Could not open the folder selector. Please try again.")
                    }
                }
            });
        });
    }
    let weak_nav = nav.downgrade();
    cancel.connect_clicked(move |_| {
        if let Some(nav) = weak_nav.upgrade() {
            nav.pop();
        }
    });
    {
        let weak_page = page.downgrade();
        let selection = selection.clone();
        let engine = engine.clone();
        let account = account.clone();
        let nav = nav.downgrade();
        let toasts = toasts.downgrade();
        let remote_button = remote_button.clone();
        let local_button = local_button.clone();
        let cancel = cancel.clone();
        let feedback = feedback.clone();
        save.connect_clicked(move |button| {
            let (Some(remote), Some(local)) = (
                selection.borrow().remote.clone(),
                selection.borrow().local.clone(),
            ) else {
                return;
            };
            button.set_sensitive(false);
            button.set_label("Saving…");
            remote_button.set_sensitive(false);
            local_button.set_sensitive(false);
            cancel.set_sensitive(false);
            feedback.set_label("Checking folders and saving your settings…");
            let worker = engine.clone();
            let id = account.id.clone();
            let weak_page = weak_page.clone();
            let engine = engine.clone();
            let nav = nav.clone();
            let toasts = toasts.clone();
            let button = button.clone();
            let remote_button = remote_button.clone();
            let local_button = local_button.clone();
            let cancel = cancel.clone();
            let feedback = feedback.clone();
            crate::flow::work(
                move || {
                    worker.configure_folder(&id, &remote.id, &local)?;
                    worker.restore_accounts()
                },
                move |result| {
                    button.set_sensitive(true);
                    button.set_label("Save and Continue");
                    remote_button.set_sensitive(true);
                    local_button.set_sensitive(true);
                    cancel.set_sensitive(true);
                    match result {
                        Ok(accounts) => {
                            if let (Some(nav), Some(toasts)) = (nav.upgrade(), toasts.upgrade()) {
                                if nav.visible_page() != weak_page.upgrade() {
                                    return;
                                }
                                nav.replace(&[crate::dashboard::build(
                                    engine, accounts, &nav, &toasts,
                                )]);
                            }
                        }
                        Err(error) => feedback.set_label(&error.to_string()),
                    }
                },
            );
        });
    }
    // Prefill edits without doing disk I/O on GTK or overwriting a user's input.
    remote_button.set_sensitive(false);
    local_button.set_sensitive(false);
    let worker = engine.clone();
    let id = account.id.clone();
    crate::flow::work(
        move || worker.saved_folder(&id),
        move |result| {
            remote_button.set_sensitive(true);
            local_button.set_sensitive(true);
            match result {
                Ok(Some(folder)) => {
                    remote_row.set_subtitle(&folder.remote.name);
                    summary_remote.set_subtitle(&folder.remote.name);
                    local_row.set_subtitle(&folder.local_display.to_string_lossy());
                    summary_local.set_subtitle(&folder.local_display.to_string_lossy());
                    *selection.borrow_mut() = Selection {
                        remote: Some(folder.remote),
                        local: Some(folder.local),
                    };
                    save.set_sensitive(true);
                }
                Ok(None) => {}
                Err(error) => feedback.set_label(&error.to_string()),
            }
        },
    );
}
