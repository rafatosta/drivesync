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
    services: (
        &super::components::activities::Activities,
        &super::components::transfers::TransferFeed,
    ),
) -> std::rc::Rc<crate::features::preferences::Accounts> {
    let (activities, transfers) = services;
    let mut settings = vec![];
    let mut menus = vec![];
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
        user.set_title_lines(0);
        user.set_subtitle_lines(2);
        user.set_tooltip_text(Some(name));
        user.set_activatable(true);
        user.set_selectable(true);
        list.append(&user);
        let content = crate::ui::content();
        let heading = super::components::account_header::AccountHeader::new(name, &state);
        let sign_out = crate::features::accounts::session::sign_out_button(
            engine.clone(),
            saved.account.id.clone(),
            nav,
            toasts,
        );
        sign_out.set_visible(false);
        content.append(&sign_out);
        let needs_setup = saved.folder.is_none();
        let dashboard = gtk::Box::new(gtk::Orientation::Vertical, 24);
        dashboard.set_visible(!needs_setup);
        dashboard.append(heading.widget());
        content.append(&dashboard);
        let preferences_editor = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let editor = gtk::Box::new(gtk::Orientation::Vertical, 24);
        editor.set_widget_name("account-settings-editor");
        let description = adw::PreferencesGroup::builder()
            .title(gettextrs::gettext("Identity"))
            .build();
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
        editor.append(&description);
        heading.bind_description(engine.clone(), &saved, &entry, &description, &user, &avatar);

        let folders =
            super::components::folders::build(engine.clone(), &saved, content.upcast_ref());
        let history = super::components::local_history::LocalHistory::new(
            engine.clone(),
            saved.account.id.clone(),
            activities.receiver(),
        );
        let restart = history.restart_callback();
        let sync = gtk::Button::from_icon_name("view-refresh-symbolic");
        sync.set_widget_name("sync-account-now");
        sync.set_valign(gtk::Align::Center);
        sync.set_tooltip_text(Some(&gettextrs::gettext("Sync now")));
        heading.widget().append(&sync);
        let (download, refresh_download) = super::components::initial_download::build(
            engine.clone(),
            &saved,
            transfers.clone(),
            activities.refresh_callback(),
            &sync,
        );
        editor.append(folders.widget());
        if needs_setup {
            content.append(&editor);
        } else {
            preferences_editor.append(&editor);
        }
        let (weak_content, weak_editor, weak_preferences, weak_dashboard, weak_entry) = (
            content.downgrade(),
            editor.downgrade(),
            preferences_editor.downgrade(),
            dashboard.downgrade(),
            entry.downgrade(),
        );
        folders.connect_saved(move || {
            if let (Some(content), Some(editor), Some(preferences), Some(dashboard)) = (
                weak_content.upgrade(),
                weak_editor.upgrade(),
                weak_preferences.upgrade(),
                weak_dashboard.upgrade(),
            ) {
                if !dashboard.is_visible() {
                    if let Some(entry) = weak_entry
                        .upgrade()
                        .filter(|entry| !entry.text().is_empty())
                    {
                        entry.emit_by_name::<()>("apply", &[]);
                    }
                    content.remove(&editor);
                    preferences.append(&editor);
                    dashboard.set_visible(true);
                }
            }
            restart();
            refresh_download();
        });
        dashboard.append(&download);
        dashboard.append(history.widget());
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
        details.add_named(&crate::ui::scroll(&content, 760), Some(&saved.account.id.0));
        menus.push((
            folders.actions().clone(),
            heading,
            user,
            content,
            sign_out,
            saved.account.id.clone(),
        ));
        settings.push(crate::features::preferences::AccountSettings::new(
            saved,
            preferences_editor,
        ));
    }
    let settings = crate::features::preferences::Accounts::new(settings);
    for (folders, heading, user, content, sign_out, id) in menus {
        let weak_content = content.downgrade();
        let actions = super::components::account_actions::AccountActions::new(
            &folders,
            &sign_out,
            move || {
                if let Some(content) = weak_content.upgrade() {
                    let _ = content.activate_action("account.configure", Some(&id.0.to_variant()));
                }
            },
        );
        actions.install(&user);
        actions.install(&content);
        let sidebar_menu = actions.menu();
        sidebar_menu.set_widget_name("sidebar-account-menu");
        sidebar_menu.add_css_class("flat");
        user.add_suffix(&sidebar_menu);
        let menu = actions.menu();
        menu.set_widget_name("dashboard-account-menu");
        heading.set_menu(&menu);
    }
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a desktop display and an isolated XDG_DATA_HOME"]
    fn discarded_dashboard_releases_components_and_observers() {
        adw::init().unwrap();
        assert!(std::env::var("XDG_DATA_HOME")
            .unwrap()
            .starts_with("/tmp/drivesync-ui-"));
        fn named(widget: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
            if widget.widget_name() == name {
                return Some(widget.clone());
            }
            let mut child = widget.first_child();
            while let Some(current) = child {
                if let Some(found) = named(&current, name) {
                    return Some(found);
                }
                child = current.next_sibling();
            }
            None
        }
        let saved = RestoredAccount {
            account: drivesync_engine::AccountSummary {
                id: drivesync_engine::AccountId("google:disposed-ui".into()),
                provider_subject: "disposed-ui".into(),
                email: Some("fixture@example.com".into()),
            },
            error: None,
            folder: None,
        };
        let nav = adw::NavigationView::new();
        let toasts = adw::ToastOverlay::new();
        let page = crate::features::dashboard::build(
            Arc::new(DriveSyncEngine::new()),
            vec![saved],
            &nav,
            &toasts,
        );
        assert!(named(page.upcast_ref(), "start-synchronization").is_none());
        assert!(named(page.upcast_ref(), "account-sync-toggle").is_none());
        let page_ref = page.downgrade();
        drop(page);
        let wait = gtk::glib::MainLoop::new(None, false);
        let finish = wait.clone();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
            finish.quit()
        });
        wait.run();
        assert!(page_ref.upgrade().is_none());
    }
}
