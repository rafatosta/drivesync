use adw::{gtk, prelude::*};
use drivesync_engine::{AccountSummary, DriveFolder, DriveSyncEngine};
use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};
#[derive(Default)]
struct Selection {
    remote: Option<DriveFolder>,
    local: Option<PathBuf>,
}
pub fn build(
    engine: Arc<DriveSyncEngine>,
    account: AccountSummary,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) -> adw::NavigationPage {
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let page = adw::NavigationPage::builder()
        .title("New Sync Folder")
        .child(&toolbar)
        .build();
    let content = crate::flow::content();
    let columns = gtk::Box::new(gtk::Orientation::Horizontal, 24);
    let form = gtk::Box::new(gtk::Orientation::Vertical, 24);
    form.set_hexpand(true);
    let who = adw::PreferencesGroup::builder()
        .title("Account")
        .description("Choose folders for this Google account.")
        .build();
    who.add(&crate::flow::row(
        account.email.as_deref().unwrap_or("Google Account"),
        "Connected",
        "avatar-default-symbolic",
    ));
    form.append(&who);
    let remote_group = adw::PreferencesGroup::builder()
        .title("Remote folder")
        .description("Select a folder in My Drive.")
        .build();
    let remote_row = crate::flow::row(
        "Google Drive",
        "No folder selected",
        "folder-remote-symbolic",
    );
    let remote_button = gtk::Button::with_label("Choose Folder");
    remote_button.set_valign(gtk::Align::Center);
    remote_row.add_suffix(&remote_button);
    remote_group.add(&remote_row);
    form.append(&remote_group);
    let local_group = adw::PreferencesGroup::builder()
        .title("Local folder")
        .description("Choose where downloaded files will be stored.")
        .build();
    let local_row = crate::flow::row("On this computer", "No folder selected", "folder-symbolic");
    let local_button = gtk::Button::with_label("Choose Folder");
    local_button.set_valign(gtk::Align::Center);
    local_row.add_suffix(&local_button);
    local_group.add(&local_row);
    form.append(&local_group);
    let rule = gtk::Label::builder()
        .label("The local folder cannot overlap another account’s folder.")
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    form.append(&rule);
    let summary = adw::PreferencesGroup::builder()
        .title("Summary")
        .description("Review your folder settings.")
        .build();
    let summary_remote =
        crate::flow::row("Remote folder", "Not selected", "folder-remote-symbolic");
    let summary_local = crate::flow::row("Local folder", "Not selected", "folder-symbolic");
    summary.add(&crate::flow::row(
        "Account",
        account
            .email
            .as_deref()
            .unwrap_or(&account.provider_subject),
        "avatar-default-symbolic",
    ));
    summary.add(&summary_remote);
    summary.add(&summary_local);
    summary.add(&crate::flow::row(
        "Sync mode",
        "Download only",
        "folder-download-symbolic",
    ));
    columns.append(&form);
    columns.append(&summary);
    content.append(&columns);
    let feedback = gtk::Label::builder().wrap(true).xalign(0.0).build();
    content.append(&feedback);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let cancel = gtk::Button::with_label("Cancel");
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let save = gtk::Button::with_label("Save and Continue");
    save.add_css_class("suggested-action");
    save.set_sensitive(false);
    actions.append(&cancel);
    actions.append(&spacer);
    actions.append(&save);
    content.append(&actions);
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
                                nav.pop_to_tag("welcome");
                                nav.push(&crate::dashboard::build(engine, accounts, &nav, &toasts));
                            }
                        }
                        Err(error) => feedback.set_label(&error.to_string()),
                    }
                },
            );
        });
    }
    let adaptive = adw::BreakpointBin::new();
    adaptive.set_width_request(300);
    adaptive.set_height_request(240);
    adaptive.set_child(Some(&crate::flow::scroll(&content, 1100)));
    let breakpoint =
        adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 760px").unwrap());
    breakpoint.add_setter(
        &columns,
        "orientation",
        Some(&gtk::Orientation::Vertical.to_value()),
    );
    adaptive.add_breakpoint(breakpoint);
    toolbar.set_content(Some(&adaptive));
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
    page
}
