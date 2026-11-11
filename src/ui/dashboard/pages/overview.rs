//! Empty completed destination; configuration remains in Preferences.
use adw::{gtk, prelude::*};
pub(crate) fn build() -> adw::NavigationPage {
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_show_back_button(false);
    header.set_title_widget(Some(&adw::WindowTitle::new("DriveSync", "")));
    header.pack_end(&crate::ui::components::main_menu::button());
    toolbar.add_top_bar(&header);
    let empty = gtk::Box::new(gtk::Orientation::Vertical, 0);
    empty.set_widget_name("dashboard-content");
    toolbar.set_content(Some(&empty));
    let page = adw::NavigationPage::new(&toolbar, "DriveSync");
    page.set_widget_name("drivesync-dashboard");
    page
}
