//! Final read-only review reports verified facts, without simulated sync success.
use super::super::components::Summary;
use adw::{gtk, prelude::*};
use gettextrs::gettext;
pub(in crate::ui::setup) struct ReviewPage {
    root: gtk::Box,
    summary: Summary,
    finish: gtk::Button,
}
impl ReviewPage {
    pub fn new() -> Self {
        let root = super::content();
        root.set_widget_name("setup-review-page");
        let bytes = gtk::glib::Bytes::from_static(include_bytes!(
            "../../../../data/illustrations/cloud-sync-success.svg"
        ));
        let icon = gtk::gio::BytesIcon::new(&bytes);
        let paintable = gtk::IconTheme::for_display(&gtk::gdk::Display::default().unwrap())
            .lookup_by_gicon(
                &icon,
                340,
                1,
                gtk::TextDirection::None,
                gtk::IconLookupFlags::empty(),
            );
        let illustration = gtk::Picture::for_paintable(&paintable);
        illustration.set_size_request(340, 183);
        illustration.set_can_shrink(true);
        // Crop the square icon canvas to the artwork's native wide aspect ratio.
        illustration.set_content_fit(gtk::ContentFit::Cover);
        illustration.set_halign(gtk::Align::Center);
        let header = super::PageHeader::new(
            "review",
            &illustration,
            &gettext("Review configuration"),
            &gettext("Review your connected account and selected folder."),
        );
        root.append(header.widget());
        let summary = Summary::new();
        root.append(summary.widget());
        root.append(&gtk::Label::builder().label(gettext("Your configuration will be checked and saved. You can change it later in Preferences.")).wrap(true).build());
        let start = gtk::Button::builder()
            .label(gettext("Finish setup"))
            .sensitive(false)
            .css_classes(["suggested-action"])
            .build();
        start.set_widget_name("setup-start");
        root.append(&start);
        Self {
            root,
            summary,
            finish: start,
        }
    }
    pub fn on_finish(&self, callback: impl Fn() + 'static) {
        self.finish.connect_clicked(move |_| callback());
    }
    pub fn widget(&self) -> &impl IsA<gtk::Widget> {
        &self.root
    }
    pub fn update(&self, provider: &str, account: &str, local: &str, remote: &str, enabled: bool) {
        self.summary.set_details(provider, account, local, remote);
        self.finish.set_sensitive(enabled);
    }
}
