//! Separate observed conflict and problem cards; existing review flows own decisions.
use super::super::presentation::Attention;
use adw::{gtk, prelude::*};
use drivesync_engine::DownloadReport;
use gettextrs::gettext;

thread_local! {
    static CARD_STYLES: std::cell::OnceCell<()> = const { std::cell::OnceCell::new() };
}

#[derive(Clone)]
pub(in crate::features::dashboard) struct AttentionSummary {
    root: gtk::Box,
    pending: std::rc::Rc<std::cell::Cell<bool>>,
    conflicts: gtk::Button,
    conflict_count: gtk::Label,
    problems: gtk::Button,
    problem_count: gtk::Label,
}
impl AttentionSummary {
    /// Embeds account-scoped review buttons without replacing their signal handlers.
    pub fn new(conflicts: &gtk::Button, problems: &gtk::Button) -> Self {
        CARD_STYLES.with(|styles| { styles.get_or_init(|| {
            let provider = gtk::CssProvider::new();
            provider.load_from_data(".attention-conflicts { background-color: alpha(@error_color, 0.06); } .attention-problems { background-color: alpha(@warning_color, 0.06); }");
            gtk::style_context_add_provider_for_display(&conflicts.display(), &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }); });
        conflicts.add_css_class("attention-conflicts");
        problems.add_css_class("attention-problems");
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.set_widget_name("attention-summary");
        root.set_visible(false);
        let cards = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(true)
            .min_children_per_line(1)
            .max_children_per_line(2)
            .column_spacing(12)
            .row_spacing(12)
            .build();
        let conflict_count = card(
            conflicts,
            "Pending conflicts",
            "Review observed versions before deciding",
            "document-properties-symbolic",
            "error",
        );
        let problem_count = card(
            problems,
            "Problems found",
            "Review details and available recovery actions",
            "dialog-warning-symbolic",
            "warning",
        );
        cards.insert(conflicts, -1);
        cards.insert(problems, -1);
        root.append(&cards);
        Self {
            root,
            pending: Default::default(),
            conflicts: conflicts.clone(),
            conflict_count,
            problems: problems.clone(),
            problem_count,
        }
    }
    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }
    pub fn clear(&self) {
        self.pending.set(false);
        self.root.set_visible(false);
    }
    /// Temporarily hides review cards while a cycle is running, preserving counts.
    pub fn set_processing(&self, processing: bool) {
        self.root.set_visible(self.pending.get() && !processing);
    }
    pub fn present(&self, report: &DownloadReport) {
        let counts = Attention::from_report(report);
        self.pending.set(counts.any());
        self.root.set_visible(counts.any());
        self.conflict_count.set_label(&counts.conflicts.to_string());
        self.problem_count.set_label(&counts.problems.to_string());
        // Parent sensitivity preserves category availability when the review button
        // independently updates its account-folder availability.
        if let Some(parent) = self.conflicts.parent() {
            parent.set_sensitive(counts.conflicts > 0);
        }
        if let Some(parent) = self.problems.parent() {
            parent.set_sensitive(counts.problems > 0);
        }
        self.conflicts.set_sensitive(counts.conflicts > 0);
        self.problems.set_sensitive(counts.problems > 0);
    }
}

/// Uses the supplied real review action as the card, including keyboard activation.
fn card(
    button: &gtk::Button,
    title: &str,
    description: &str,
    icon: &str,
    class: &str,
) -> gtk::Label {
    button.add_css_class("card");
    button.set_width_request(240);
    button.set_hexpand(true);
    button.set_tooltip_text(Some(&gettext(description)));
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.set_margin_top(12);
    row.set_margin_bottom(12);
    row.set_margin_start(12);
    row.set_margin_end(12);
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(28);
    image.add_css_class(class);
    row.append(&image);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
    text.set_hexpand(true);
    text.append(
        &gtk::Label::builder()
            .label(gettext(title))
            .wrap(true)
            .xalign(0.0)
            .css_classes(["heading"])
            .build(),
    );
    let count = gtk::Label::builder()
        .label("0")
        .xalign(0.0)
        .css_classes(["title-1"])
        .build();
    text.append(&count);
    row.append(&text);
    row.append(&gtk::Image::from_icon_name("go-next-symbolic"));
    button.set_child(Some(&row));
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an isolated GTK display"]
    fn attention_cards_keep_independent_actions_and_counts() {
        adw::init().unwrap();
        let conflicts = gtk::Button::new();
        let problems = gtk::Button::new();
        let hits = std::rc::Rc::new(std::cell::Cell::new(0));
        let count = hits.clone();
        conflicts.connect_clicked(move |_| count.set(count.get() + 1));
        let count = hits.clone();
        problems.connect_clicked(move |_| count.set(count.get() + 10));
        let summary = AttentionSummary::new(&conflicts, &problems);
        summary.present(&DownloadReport {
            issues: vec![
                drivesync_engine::DownloadIssue {
                    relative_path: Some("conflict.txt".into()),
                    error: drivesync_engine::EngineError::DownloadConflict,
                },
                drivesync_engine::DownloadIssue {
                    relative_path: Some("problem.txt".into()),
                    error: drivesync_engine::EngineError::InvalidFolder,
                },
            ],
            ..Default::default()
        });
        assert!(summary.root.is_visible());
        assert_eq!(summary.conflict_count.text(), "1");
        assert_eq!(summary.problem_count.text(), "1");
        conflicts.emit_clicked();
        assert_eq!(hits.get(), 1);
        problems.emit_clicked();
        assert_eq!(hits.get(), 11);
        summary.set_processing(true);
        assert!(!summary.root.is_visible());
        assert_eq!(summary.conflict_count.text(), "1");
        summary.set_processing(false);
        assert!(
            summary.root.is_visible(),
            "processing must not discard pending observations"
        );
        summary.present(&DownloadReport::default());
        assert!(!summary.root.is_visible());
        assert!(!conflicts.is_sensitive());
        assert!(!problems.is_sensitive());
    }
}
