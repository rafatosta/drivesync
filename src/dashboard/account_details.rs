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
            .hexpand(true)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .css_classes(["title-1"])
            .build();
        let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        title_row.append(&title);
        let sync_toggle = gtk::Button::from_icon_name("media-playback-start-symbolic");
        sync_toggle.set_tooltip_text(Some("Start or resume synchronization"));
        sync_toggle.set_valign(gtk::Align::Center);
        sync_toggle.set_sensitive(false);
        sync_toggle.set_widget_name("account-sync-toggle");
        title_row.append(&sync_toggle);
        identity.append(&title_row);
        let account_status = gtk::Label::builder()
            .label(&state)
            .use_markup(false)
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        identity.append(&account_status);
        heading.append(&identity);
        let sign_out =
            crate::session::sign_out_button(engine.clone(), saved.account.id.clone(), nav, toasts);
        sign_out.set_visible(false);
        content.append(&sign_out);
        let sync_menu_status = gtk::Label::new(None);
        sync_menu_status.set_visible(false);
        content.append(&sync_menu_status);
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
        let configuration = adw::ExpanderRow::builder()
            .title(gettextrs::gettext("Account configuration"))
            .subtitle(gettextrs::gettext(
                "Account identity and synchronization folders",
            ))
            .expanded(saved.folder.is_none())
            .build();
        configuration.set_widget_name("account-configuration");
        let configuration_group = adw::PreferencesGroup::new();
        configuration_group.add(&configuration);
        configuration.add_row(&description);
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
        configuration.add_row(folders.widget());
        content.append(&configuration_group);
        let job = crate::automatic::Controller::attach(engine.clone(), &saved);
        let actions = super::account_actions::AccountActions::new(
            job.clone(),
            folders.actions(),
            &configuration,
            &user,
            list,
            &sign_out,
        );
        let preferences = gtk::gio::SimpleAction::new("preferences", None);
        let preferences_engine = engine.clone();
        let preferences_saved = saved.clone();
        let preferences_job = job.clone();
        let weak_content = content.downgrade();
        preferences.connect_activate(move |_, _| {
            if let Some(parent) = weak_content
                .upgrade()
                .and_then(|w| w.root())
                .and_downcast::<adw::ApplicationWindow>()
            {
                let mut saved = preferences_saved.clone();
                saved.folder = preferences_job.folder();
                crate::preferences::present_account(&parent, preferences_engine.clone(), &saved);
            }
        });
        actions.group().add_action(&preferences);
        actions.install(&user);
        actions.install(&content);
        let sidebar_menu = actions.menu();
        sidebar_menu.set_widget_name("sidebar-account-menu");
        user.add_suffix(&sidebar_menu);
        let menu = actions.menu();
        menu.set_widget_name("dashboard-account-menu");
        heading.append(&menu);
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
            .build();
        sync.add(&sync_state);
        content.append(&sync);
        content.append(&super::synchronization::build_for_job(
            engine.clone(),
            &saved,
            super::synchronization::Controls {
                folders: folders.widget(),
                start: &start,
                row: &sync_state,
                status: &status,
                menu: &menu,
                account_toggle: &sync_toggle,
                account_row: &user,
                account_status: &account_status,
                account_status_fallback: &state,
                menu_status: &sync_menu_status,
            },
            job,
        ));
        details.add_named(&content, Some(&saved.account.id.0));
    }
}
