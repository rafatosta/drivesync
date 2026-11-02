//! Compact observed conflicts and problems; existing review flows own decisions.
use super::presentation::Attention;
use adw::{gtk, prelude::*};
use drivesync_engine::DownloadReport;
use gettextrs::gettext;

#[derive(Clone)]
pub(super) struct AttentionSummary {
    root: gtk::Box,
    conflicts: adw::ActionRow,
    problems: adw::ActionRow,
}
impl AttentionSummary {
    pub fn new(review: &gtk::Button) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_widget_name("attention-summary");
        root.set_visible(false);
        let group = adw::PreferencesGroup::builder()
            .title(gettext("Observed pending items"))
            .build();
        let conflicts = crate::flow::row(&gettext("Conflicts"), "", "document-properties-symbolic");
        let problems = crate::flow::row(&gettext("Problems"), "", "dialog-warning-symbolic");
        for row in [&conflicts, &problems] {
            row.set_activatable(true);
            row.set_subtitle_lines(0);
            row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
            let weak_review = review.downgrade();
            row.connect_activated(move |_| {
                if let Some(button) = weak_review.upgrade() {
                    button.emit_clicked();
                }
            });
            group.add(row);
        }
        root.append(&group);
        Self {
            root,
            conflicts,
            problems,
        }
    }
    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }
    pub fn clear(&self) {
        self.root.set_visible(false);
    }
    pub fn present(&self, report: &DownloadReport) {
        let counts = Attention::from_report(report);
        self.root.set_visible(counts.any());
        self.conflicts.set_visible(counts.conflicts > 0);
        self.conflicts.set_subtitle(&format!(
            "{} · {}",
            counts.conflicts,
            gettext("Review observed versions before deciding")
        ));
        self.problems.set_visible(counts.problems > 0);
        self.problems.set_subtitle(&format!(
            "{} · {}",
            counts.problems,
            gettext("Review details and available recovery actions")
        ));
    }
}
