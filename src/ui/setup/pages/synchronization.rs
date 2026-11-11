//! Folder choices remain drafts until supported by engine configuration APIs.
use super::super::components::FolderChoice;
use adw::{gtk, prelude::*};
use gettextrs::gettext;
pub(in crate::ui::setup) struct SynchronizationPage {
    root: gtk::Box,
    local: FolderChoice,
    remote: adw::ActionRow,
    choose_remote: gtk::Button,
}
impl SynchronizationPage {
    pub fn new() -> Self {
        let root = super::content(
            &gettextrs::gettext("Configure synchronization"),
            &gettextrs::gettext("Choose where your files will be stored on this computer."),
            "folder-symbolic",
        );
        let remote = adw::PreferencesGroup::builder()
            .title(gettext("Remote folder"))
            .build();
        let remote_location = crate::ui::row(
            &gettext("Remote folder"),
            &gettext("Choose a folder"),
            "folder-remote-symbolic",
        );
        remote_location.set_widget_name("setup-remote-folder");
        let choose_remote = gtk::Button::builder()
            .label(gettext("Choose…"))
            .valign(gtk::Align::Center)
            .build();
        choose_remote.set_widget_name("setup-choose-remote");
        remote_location.add_suffix(&choose_remote);
        remote_location.set_activatable_widget(Some(&choose_remote));
        remote.add(&remote_location);
        remote.set_description(Some(&gettext(
            "Choose a folder from your connected account.",
        )));
        root.append(&remote);
        let local = FolderChoice::new();
        root.append(local.widget());
        root.append(&gtk::Label::builder().label(gettext("Your choices are kept only while setup is open. Closing without completing setup disconnects the account.")).wrap(true).css_classes(["dim-label"]).build());
        Self {
            root,
            local,
            remote: remote_location,
            choose_remote,
        }
    }
    pub fn widget(&self) -> &impl IsA<gtk::Widget> {
        &self.root
    }
    pub fn update(&self, path: Option<&str>, remote: Option<&str>, enabled: bool) {
        self.local.set_location(path, enabled);
        self.remote
            .set_subtitle(remote.unwrap_or(&gettext("Choose a folder")));
        self.choose_remote.set_sensitive(enabled);
    }
    pub fn on_choose_remote(&self, callback: impl Fn() + 'static) {
        self.choose_remote.connect_clicked(move |_| callback());
    }
    pub fn on_choose(&self, callback: impl Fn() + 'static) {
        self.local.on_choose(callback);
    }
}
