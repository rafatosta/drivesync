//! Final read-only review reports verified facts, without simulated sync success.
use super::super::components::Summary;
use adw::{gtk, prelude::*};
use gettextrs::gettext;
pub(in crate::ui::setup) struct ReviewPage {
    root: gtk::Box,
    summary: Summary,
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
            &gettext("Start synchronization"),
            &gettext("Review your connected account and selected folder."),
        );
        root.append(header.widget());
        let summary = Summary::new();
        root.append(summary.widget());
        root.append(&gtk::Label::builder().label(gettext("Synchronization is not available yet. Folder choices are kept only while setup is open.")).wrap(true).build());
        let start = gtk::Button::builder()
            .label(gettext("Start synchronization"))
            .sensitive(false)
            .tooltip_text(gettext("Synchronization is not available yet."))
            .css_classes(["suggested-action"])
            .build();
        start.set_widget_name("setup-start");
        root.append(&start);
        Self { root, summary }
    }
    pub fn widget(&self) -> &impl IsA<gtk::Widget> {
        &self.root
    }
    pub fn update(&self, provider: &str, account: &str, local: &str, remote: &str) {
        self.summary.set_details(provider, account, local, remote);
    }
}
