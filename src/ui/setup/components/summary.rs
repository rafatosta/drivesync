//! Read-only setup summary. No storage or verification is performed here.
use adw::{gtk, prelude::*};
use gettextrs::gettext;
pub(in crate::ui::setup) struct Summary {
    root: adw::PreferencesGroup,
    account: adw::ActionRow,
    provider: adw::ActionRow,
    local: adw::ActionRow,
    remote: adw::ActionRow,
}
impl Summary {
    pub fn new() -> Self {
        let root = adw::PreferencesGroup::new();
        let provider = crate::ui::row(&gettext("Provider"), "", "folder-remote-symbolic");
        let account = crate::ui::row(&gettext("Connected account"), "", "avatar-default-symbolic");
        account.set_widget_name("setup-review-account");
        let local = crate::ui::row(&gettext("Local folder"), "", "folder-symbolic");
        root.add(&provider);
        root.add(&account);
        root.add(&local);
        let remote = crate::ui::row(&gettext("Remote folder"), "", "folder-remote-symbolic");
        remote.set_widget_name("setup-review-remote");
        root.add(&remote);
        Self {
            root,
            account,
            provider,
            local,
            remote,
        }
    }
    pub fn widget(&self) -> &impl IsA<gtk::Widget> {
        &self.root
    }
    pub fn set_details(&self, provider: &str, account: &str, local: &str, remote: &str) {
        self.provider.set_subtitle(provider);
        self.account.set_subtitle(account);
        self.local.set_subtitle(local);
        self.remote.set_subtitle(remote);
    }
}
