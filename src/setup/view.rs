//! GTK widgets and adaptive layout for remote/local folder configuration.
//!
//! Constructs a screen with an account section, folder selectors, a summary,
//! feedback text and action buttons. No I/O or signal connections occur here.
use adw::{gtk, prelude::*};
use drivesync_engine::{AccountSummary, DriveSyncEngine};
use std::sync::Arc;

/// Widgets shared with the action handlers, held only for this setup page.
pub(super) struct SetupView {
    pub(super) page: adw::NavigationPage,
    pub(super) remote_row: adw::ActionRow,
    pub(super) remote_button: gtk::Button,
    pub(super) local_row: adw::ActionRow,
    pub(super) local_button: gtk::Button,
    pub(super) summary_remote: adw::ActionRow,
    pub(super) summary_local: adw::ActionRow,
    pub(super) feedback: gtk::Label,
    pub(super) cancel: gtk::Button,
    pub(super) save: gtk::Button,
}

/// Constructs the setup page, including narrow-window breakpoint behavior.
///
/// Does not connect button signals or read saved configuration; these
/// responsibilities belong to the action controller.
pub(super) fn build(
    engine: Arc<DriveSyncEngine>,
    account: &AccountSummary,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) -> SetupView {
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.pack_end(&crate::session::sign_out_button(
        engine.clone(),
        account.id.clone(),
        nav,
        toasts,
    ));
    toolbar.add_top_bar(&header);
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

    SetupView {
        page,
        remote_row,
        remote_button,
        local_row,
        local_button,
        summary_remote,
        summary_local,
        feedback,
        cancel,
        save,
    }
}
