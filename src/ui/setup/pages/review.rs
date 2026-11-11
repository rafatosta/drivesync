//! Final read-only review reports verified facts, without simulated sync success.
use super::super::components::Summary;
use adw::{gtk, prelude::*};
use gettextrs::gettext;
pub(in crate::ui::setup) struct ReviewPage {
    root: gtk::Box,
    summary: Summary,
    dashboard: gtk::Button,
}
impl ReviewPage {
    pub fn new() -> Self {
        let root = super::content(
            &gettextrs::gettext("Start synchronization"),
            &gettextrs::gettext("Review your connected account and saved folder choice."),
            "dialog-information-symbolic",
        );
        let summary = Summary::new();
        root.append(summary.widget());
        root.append(&gtk::Label::builder().label(gettext("Synchronization is not available yet. Remote folder configuration and file checks have not been performed.")).wrap(true).build());
        let start = gtk::Button::builder()
            .label(gettext("Start synchronization"))
            .sensitive(false)
            .tooltip_text(gettext("Synchronization is not available yet."))
            .css_classes(["suggested-action"])
            .build();
        start.set_widget_name("setup-start");
        root.append(&start);
        let dashboard = gtk::Button::with_label(&gettext("View account in dashboard"));
        root.append(&dashboard);
        Self {
            root,
            summary,
            dashboard,
        }
    }
    pub fn widget(&self) -> &impl IsA<gtk::Widget> {
        &self.root
    }
    pub fn update(&self, provider: &str, account: &str, local: &str) {
        self.summary.set_details(provider, account, local);
    }
    pub fn on_dashboard(&self, callback: impl Fn() + 'static) {
        self.dashboard.connect_clicked(move |_| callback());
    }
}
