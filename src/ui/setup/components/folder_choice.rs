//! Folder presentation; picker behavior is supplied as an event by its owner.
use adw::{gtk, prelude::*};
use gettextrs::gettext;
pub(in crate::ui::setup) struct FolderChoice {
    root: adw::PreferencesGroup,
    location: adw::ActionRow,
    choose: gtk::Button,
}
impl FolderChoice {
    pub fn new() -> Self {
        let root = adw::PreferencesGroup::builder()
            .title(gettext("Local folder"))
            .build();
        let location = adw::ActionRow::builder()
            .title(gettext("On this computer"))
            .subtitle(gettext("Choose a folder"))
            .use_markup(false)
            .subtitle_selectable(true)
            .build();
        location.add_prefix(&gtk::Image::from_icon_name("folder-symbolic"));
        let choose = gtk::Button::builder()
            .label(gettext("Choose…"))
            .valign(gtk::Align::Center)
            .build();
        location.add_suffix(&choose);
        location.set_activatable_widget(Some(&choose));
        root.add(&location);
        Self {
            root,
            location,
            choose,
        }
    }
    pub fn widget(&self) -> &impl IsA<gtk::Widget> {
        &self.root
    }
    pub fn set_location(&self, path: Option<&str>, enabled: bool) {
        self.location
            .set_subtitle(path.unwrap_or(&gettext("Choose a folder")));
        self.choose.set_sensitive(enabled);
    }
    pub fn on_choose(&self, callback: impl Fn() + 'static) {
        self.choose.connect_clicked(move |_| callback());
    }
}
