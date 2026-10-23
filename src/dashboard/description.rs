//! Asynchronous local description editing through the engine facade.
use adw::{gtk, prelude::*};
use drivesync_engine::{DriveSyncEngine, RestoredAccount};
use std::sync::Arc;

const HELP: &str = "A local name for this account. Leave blank to use the email address.";

/// Weak widgets let in-flight saves finish without retaining removed pages.
#[derive(Clone)]
struct Presentation {
    entry: gtk::glib::WeakRef<adw::EntryRow>,
    group: gtk::glib::WeakRef<adw::PreferencesGroup>,
    title: gtk::glib::WeakRef<gtk::Label>,
    user: gtk::glib::WeakRef<adw::ActionRow>,
    avatar: gtk::glib::WeakRef<adw::Avatar>,
    email: String,
    state: String,
}
impl Presentation {
    fn display(&self, description: Option<String>) {
        let name = description.as_deref().unwrap_or(&self.email);
        if let Some(entry) = self.entry.upgrade() {
            entry.set_text(description.as_deref().unwrap_or(""));
            entry.set_sensitive(true);
        }
        if let Some(title) = self.title.upgrade() {
            title.set_label(name);
        }
        if let Some(user) = self.user.upgrade() {
            user.set_title(name);
            user.set_subtitle(&self.state);
            user.set_tooltip_text(Some(&self.email));
        }
        if let Some(avatar) = self.avatar.upgrade() {
            avatar.set_text(Some(name));
        }
        if let Some(group) = self.group.upgrade() {
            group.set_description(Some(HELP));
        }
    }
    fn error(&self, error: &drivesync_engine::EngineError) {
        if let Some(group) = self.group.upgrade() {
            group.set_description(Some(&error.to_string()));
        }
    }
}

/// Loads the saved label and commits edits only on the native apply action.
/// Disk work runs off the GTK thread; failed saves keep the draft editable.
pub(super) fn bind(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    entry: &adw::EntryRow,
    group: &adw::PreferencesGroup,
    title: &gtk::Label,
    user: &adw::ActionRow,
    avatar: &adw::Avatar,
) {
    let ui = Presentation {
        entry: entry.downgrade(),
        group: group.downgrade(),
        title: title.downgrade(),
        user: user.downgrade(),
        avatar: avatar.downgrade(),
        email: saved
            .account
            .email
            .clone()
            .unwrap_or_else(|| "Google Account".into()),
        state: saved
            .error
            .map(|e| e.to_string())
            .unwrap_or_else(|| "Connected".into()),
    };
    entry.set_show_apply_button(true);
    entry.set_sensitive(false);
    group.set_description(Some("Loading account description…"));
    let worker = engine.clone();
    let id = saved.account.id.clone();
    let loaded_ui = ui.clone();
    let last_saved = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let loaded_value = last_saved.clone();
    crate::flow::work(
        move || worker.account_description(&id),
        move |result| match result {
            Ok(value) => {
                *loaded_value.borrow_mut() = value.clone().unwrap_or_default();
                loaded_ui.display(value);
            }
            Err(error) => loaded_ui.error(&error),
        },
    );
    let id = saved.account.id.clone();
    entry.connect_apply(move |entry| {
        if !entry.is_sensitive() {
            return;
        }
        let draft = entry.text().to_string();
        entry.set_sensitive(false);
        if let Some(group) = ui.group.upgrade() {
            group.set_description(Some("Saving account description…"));
        }
        let worker = engine.clone();
        let id = id.clone();
        let ui = ui.clone();
        let last_saved = last_saved.clone();
        crate::flow::work(
            move || worker.set_account_description(&id, &draft),
            move |result| match result {
                Ok(value) => {
                    *last_saved.borrow_mut() = value.clone().unwrap_or_default();
                    ui.display(value);
                }
                Err(error) => {
                    if let Some(entry) = ui.entry.upgrade() {
                        let draft = entry.text();
                        entry.set_text(&last_saved.borrow());
                        entry.set_text(&draft);
                        entry.set_sensitive(true);
                    }
                    ui.error(&error);
                }
            },
        );
    });
}
