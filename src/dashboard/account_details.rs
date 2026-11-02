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
        let heading = super::account_header::AccountHeader::new(name, &state);
        let sign_out =
            crate::session::sign_out_button(engine.clone(), saved.account.id.clone(), nav, toasts);
        sign_out.set_visible(false);
        content.append(&sign_out);
        content.append(heading.widget());

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
        heading.bind_description(engine.clone(), &saved, &entry, &description, &user, &avatar);

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
        let folders = super::folders::build(
            engine.clone(),
            &saved,
            &sync_state,
            &status,
            &start,
            &configuration,
        );
        configuration.add_row(folders.widget());

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
        heading.set_menu(&menu);
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
        content.append(&super::synchronization::build_for_job(
            engine.clone(),
            &saved,
            super::synchronization::Controls {
                folders: folders.widget(),
                start: &start,
                row: &sync_state,
                status: &status,
                header: &heading,
                account_row: &user,
            },
            job,
        ));
        content.append(&configuration_group);
        details.add_named(&content, Some(&saved.account.id.0));
    }
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
        let page =
            crate::dashboard::build(Arc::new(DriveSyncEngine::new()), vec![saved], &nav, &toasts);
        let start = named(page.upcast_ref(), "start-synchronization")
            .unwrap()
            .downgrade();
        let header_toggle = named(page.upcast_ref(), "account-sync-toggle")
            .unwrap()
            .downgrade();
        let attention = named(page.upcast_ref(), "attention-summary")
            .unwrap()
            .downgrade();
        let page_ref = page.downgrade();
        drop(page);
        let wait = gtk::glib::MainLoop::new(None, false);
        let finish = wait.clone();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
            finish.quit()
        });
        wait.run();
        assert!(page_ref.upgrade().is_none());
        assert!(start.upgrade().is_none());
        assert!(header_toggle.upgrade().is_none());
        assert!(attention.upgrade().is_none());
    }
}
