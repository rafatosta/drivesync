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
        let avatar = adw::Avatar::new(32, Some(name), true);
        user.add_prefix(&avatar);
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
        let title = gtk::Label::builder()
            .label(name)
            .use_markup(false)
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .css_classes(["title-1"])
            .build();
        identity.append(&title);
        let account_status = gtk::Label::builder()
            .label(&state)
            .use_markup(false)
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        identity.append(&account_status);
        heading.append(&identity);
        let menu = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text("Account actions")
            .valign(gtk::Align::Start)
            .build();
        let popover = gtk::Popover::new();
        let actions = gtk::Box::new(gtk::Orientation::Vertical, 6);
        actions.append(
            &gtk::Label::builder()
                .label(format!("Status: {state}"))
                .xalign(0.0)
                .margin_start(12)
                .margin_end(12)
                .margin_top(8)
                .margin_bottom(8)
                .build(),
        );
        let sync_menu_status = gtk::Label::builder()
            .label("Synchronization: Not started")
            .xalign(0.0)
            .margin_start(12)
            .margin_end(12)
            .margin_top(8)
            .margin_bottom(8)
            .build();
        actions.append(&sync_menu_status);
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

        let description = adw::PreferencesGroup::new();
        let entry = adw::EntryRow::builder()
            .title("Account description")
            .build();
        entry.set_widget_name("account-display-name");

        description.add(&entry);
        let email = adw::ActionRow::builder()
            .title("Email")
            .subtitle(name)
            .use_markup(false)
            .subtitle_selectable(true)
            .build();
        email.set_widget_name("account-email");
        email.set_subtitle_lines(2);
        email.set_tooltip_text(Some(name));
        description.add(&email);
        content.append(&description);
        super::description::bind(
            engine.clone(),
            &saved,
            &entry,
            &description,
            &title,
            &user,
            &avatar,
        );

        let sync_state = crate::flow::row(
            "Not started",
            "Choose folders before synchronization",
            "folder-download-symbolic",
        );
        let status = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let start = gtk::Button::with_label("Synchronize Now");
        start.set_halign(gtk::Align::End);
        start.add_css_class("suggested-action");
        start.set_sensitive(false);
        start.set_tooltip_text(Some(
            "Download new files and update intact managed copies; preserve local edits",
        ));
        start.set_widget_name("start-synchronization");
        let folders = super::folders::build(engine.clone(), &saved, &sync_state, &status, &start);
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
        sync.add(&super::upload_mode::build(
            engine.clone(),
            &saved,
            &folders,
            &sync,
        ));
        sync.add(&sync_state);
        content.append(&sync);
        content.append(&super::synchronization::build(
            engine.clone(),
            &saved,
            &folders,
            &start,
            &sync_state,
            &status,
            &menu,
            &user,
            &account_status,
            &state,
            &sync_menu_status,
        ));
        details.add_named(&content, Some(&saved.account.id.0));
    }
}
