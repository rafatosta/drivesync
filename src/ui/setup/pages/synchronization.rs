//! Folder choices remain drafts until supported by engine configuration APIs.
use super::super::components::FolderChoice;
use adw::{gtk, prelude::*};
use gettextrs::gettext;
pub(in crate::ui::setup) struct SynchronizationPage {
    root: gtk::Box,
    local: FolderChoice,
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
        remote.add(&crate::ui::row(
            &gettext("Not configured"),
            &gettext("Remote folder selection is not available yet."),
            "folder-remote-symbolic",
        ));
        root.append(&remote);
        let local = FolderChoice::new();
        root.append(local.widget());
        root.append(&gtk::Label::builder().label(gettext("Your choices are kept only while setup is open. Closing without completing setup disconnects the account.")).wrap(true).css_classes(["dim-label"]).build());
        Self { root, local }
    }
    pub fn widget(&self) -> &impl IsA<gtk::Widget> {
        &self.root
    }
    pub fn update(&self, path: Option<&str>, enabled: bool) {
        self.local.set_location(path, enabled);
    }
    pub fn on_choose(&self, callback: impl Fn() + 'static) {
        self.local.on_choose(callback);
    }
}
