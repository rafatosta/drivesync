//! Per-account presentation using existing folder and sign-out actions.
use adw::{gtk, prelude::*};
use drivesync_engine::{DriveSyncEngine, RestoredAccount};
use std::sync::Arc;

/// Builds isolated account pages; absent capabilities remain disabled.
pub(super) fn populate(
    engine: Arc<DriveSyncEngine>,
    accounts: Vec<RestoredAccount>,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    list: &gtk::ListBox,
    details: &gtk::Stack,
) {
    for saved in accounts {
        let name = saved.account.email.as_deref().unwrap_or("Google Account");
        let state = saved
            .error
            .map(|e| e.to_string())
            .unwrap_or_else(|| "Connected".into());
        let user = adw::ActionRow::builder()
            .title(name)
            .subtitle(&state)
            .use_markup(false)
            .build();
        user.add_prefix(&adw::Avatar::new(32, Some(name), true));
        user.set_subtitle_selectable(false);
        user.set_title_lines(1);
        user.set_subtitle_lines(2);
        user.set_tooltip_text(Some(name));
        user.set_activatable(true);
        user.set_selectable(true);
        list.append(&user);

        let content = crate::flow::content();
        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let identity = gtk::Box::new(gtk::Orientation::Vertical, 6);
        identity.set_hexpand(true);
        identity.append(
            &gtk::Label::builder()
                .label(name)
                .use_markup(false)
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .css_classes(["title-1"])
                .build(),
        );
        identity.append(
            &gtk::Label::builder()
                .label(&state)
                .use_markup(false)
                .xalign(0.0)
                .wrap(true)
                .css_classes(["dim-label"])
                .build(),
        );
        heading.append(&identity);
        let menu = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text("Account actions")
            .valign(gtk::Align::Start)
            .build();
        let popover = gtk::Popover::new();
        let actions = gtk::Box::new(gtk::Orientation::Vertical, 6);
        actions.append(&crate::session::sign_out_button(
            engine.clone(),
            saved.account.id.clone(),
            nav,
            toasts,
        ));
        popover.set_child(Some(&actions));
        menu.set_popover(Some(&popover));
        heading.append(&menu);
        content.append(&heading);

        let description = adw::PreferencesGroup::builder()
            .description("Local account label. Saving changes is not available yet.")
            .build();
        let entry = adw::EntryRow::builder()
            .title("Account description")
            .build();
        entry.set_widget_name("account-display-name");
        entry.set_text(name);
        description.add(&entry);
        content.append(&description);

        let folders = adw::PreferencesGroup::builder().title("Folders").build();
        let remote = saved
            .folder
            .as_ref()
            .map(|f| f.remote.name.as_str())
            .unwrap_or("Not configured");
        let local = saved
            .folder
            .as_ref()
            .map(|f| f.local_display.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Not configured".into());
        for (title, subtitle, icon) in [
            ("Google Drive", remote, "folder-remote-symbolic"),
            ("Folder on this computer", local.as_str(), "folder-symbolic"),
        ] {
            let row = crate::flow::row(title, subtitle, icon);
            row.set_subtitle_lines(2);
            row.set_tooltip_text(Some(subtitle));
            let button = gtk::Button::with_label("Change…");
            button.set_valign(gtk::Align::Center);
            button.set_tooltip_text(Some("Change Google Drive and local folders"));
            button.set_sensitive(saved.error.is_none());
            let engine = engine.clone();
            let account = saved.account.clone();
            let weak_nav = nav.downgrade();
            let weak_toasts = toasts.downgrade();
            button.connect_clicked(move |_| {
                if let (Some(nav), Some(toasts)) = (weak_nav.upgrade(), weak_toasts.upgrade()) {
                    nav.push(&crate::setup::build(
                        engine.clone(),
                        account.clone(),
                        &nav,
                        &toasts,
                    ));
                }
            });
            row.add_suffix(&button);
            row.set_activatable_widget(Some(&button));
            folders.add(&row);
        }
        content.append(&folders);
        if saved.error.is_some() {
            let reconnect = gtk::Button::with_label("Check or Reconnect Account");
            let weak_nav = nav.downgrade();
            reconnect.connect_clicked(move |_| {
                if let Some(nav) = weak_nav.upgrade() {
                    let _ = nav.activate_action("connection.open", None);
                }
            });
            content.append(&reconnect);
        }
        let sync = adw::PreferencesGroup::builder()
            .title("Synchronization")
            .description("Local files will not be uploaded to Google Drive.")
            .build();
        sync.add(&crate::flow::row(
            "Download only",
            "From Google Drive to this computer",
            "folder-download-symbolic",
        ));
        sync.add(&crate::flow::row(
            "Not started",
            if saved.folder.is_some() {
                "Folders are configured"
            } else {
                "Choose folders before synchronization"
            },
            "media-playback-pause-symbolic",
        ));
        content.append(&sync);
        let banner = adw::Banner::new("File transfers are not available yet.");
        banner.set_revealed(true);
        content.append(&banner);
        let footer = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let status = gtk::Label::builder()
            .label(if saved.folder.is_some() {
                "Configuration saved"
            } else {
                "Configuration required"
            })
            .xalign(0.0)
            .hexpand(true)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        footer.append(&status);
        let start = gtk::Button::with_label("Start Synchronization");
        start.set_halign(gtk::Align::End);
        start.add_css_class("suggested-action");
        start.set_sensitive(false);
        start.set_tooltip_text(Some("Synchronization is not implemented yet"));
        start.set_widget_name("start-synchronization");
        footer.append(&start);
        content.append(&footer);
        details.add_named(&content, Some(&saved.account.id.0));
    }
}
