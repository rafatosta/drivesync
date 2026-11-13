//! Account-bound folder shortcuts and confirmed session removal.
use crate::application::{accounts::DisconnectEvent, Context};
use adw::{gtk, prelude::*};
use drivesync_engine::Account;
use gettextrs::gettext;
use gtk::{gio, glib};
use std::rc::Rc;

/// Captures the row's account identity, independently of sidebar selection.
pub(super) fn build(
    context: &Context,
    navigation: &glib::WeakRef<adw::NavigationView>,
    feedback: &glib::WeakRef<gtk::Label>,
    account: &Account,
) -> gtk::MenuButton {
    let menu = gio::Menu::new();
    for (label, action) in [
        (gettext("Open local folder"), "local"),
        (gettext("Open Google Drive folder"), "remote"),
        (gettext("Sign Out"), "sign-out"),
    ] {
        menu.append(Some(&label), Some(&format!("account-menu.{action}")));
    }
    let button = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(gettext("Account actions"))
        .valign(gtk::Align::Center)
        .menu_model(&menu)
        .css_classes(["flat"])
        .build();
    button.set_widget_name("sidebar-account-menu");
    let group = gio::SimpleActionGroup::new();
    for (name, local) in [("local", true), ("remote", false)] {
        let action = gio::SimpleAction::new(name, None);
        if !local {
            action.set_enabled(account.provider.0 == "google");
        }
        let id = account.id.clone();
        let feedback = feedback.clone();
        let button = button.downgrade();
        action.connect_activate(move |_, _| {
            let Some(button) = button.upgrade().filter(|button| button.is_sensitive()) else {
                return;
            };
            let parent = button.root().and_downcast::<gtk::Window>();
            button.set_sensitive(false);
            let button = button.downgrade();
            let id = id.clone();
            let feedback = feedback.clone();
            glib::spawn_future_local(async move {
                let result = async {
                    let config = crate::services::configuration::load(&id).await
                        .map_err(|_| gettext("Saved configuration could not be read. Your account has been preserved."))?
                        .ok_or_else(|| gettext("The configuration is incomplete."))?;
                    if local {
                        // Retain the saved portal grant rather than substituting its display path.
                        gtk::FileLauncher::new(Some(&gio::File::for_uri(&config.local_uri)))
                            .launch_future(parent.as_ref()).await
                    } else {
                        let id = glib::uri_escape_string(&config.remote_id, None::<&str>, false);
                        gtk::UriLauncher::new(&format!("https://drive.google.com/drive/folders/{id}"))
                            .launch_future(parent.as_ref()).await
                    }.map_err(|error| error.to_string())
                }.await;
                if let Some(button) = button.upgrade() {
                    button.set_sensitive(true);
                }
                if let Some(feedback) = feedback.upgrade() {
                    feedback.set_label(&result.err().unwrap_or_default());
                }
            });
        });
        group.add_action(&action);
    }
    let action = gio::SimpleAction::new("sign-out", None);
    let weak_button = button.downgrade();
    let context = context.clone();
    let account = account.clone();
    let navigation = navigation.clone();
    let feedback = feedback.clone();
    action.connect_activate(move |_, _| {
        let Some(button) = weak_button.upgrade().filter(|button| button.is_sensitive()) else {
            return;
        };
        let Some(parent) = button.root().and_downcast::<gtk::Window>() else {
            return;
        };
        button.set_sensitive(false);
        let button = button.downgrade();
        let navigation = navigation.clone();
        let feedback = feedback.clone();
        let owner = context.clone();
        crate::application::accounts::disconnect(&context, &parent, &account, Rc::new(move |event| {
            if let Some(button) = button.upgrade() {
                button.set_sensitive(true);
            }
            match event {
                DisconnectEvent::Cancelled => (),
                DisconnectEvent::Failed(message) => {
                    if let Some(feedback) = feedback.upgrade() {
                        feedback.set_label(&message);
                    }
                }
                DisconnectEvent::Disconnected { draft_removed } => {
                    if let Some(nav) = navigation.upgrade() {
                        if let Some(toasts) = nav.ancestor(adw::ToastOverlay::static_type()).and_downcast::<adw::ToastOverlay>() {
                            if !draft_removed {
                                toasts.add_toast(adw::Toast::new(&gettext("Account disconnected. The saved setup draft could not be removed.")));
                            }
                            crate::application::navigation::reload_accounts(&owner, &nav, &toasts, None);
                        } else {
                            let _ = nav.activate_action("dashboard.refresh", None);
                        }
                    }
                }
            }
        }));
    });
    group.add_action(&action);
    button.insert_action_group("account-menu", Some(&group));
    button
}
