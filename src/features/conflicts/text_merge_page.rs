//! Full B/L/R/result review with an explicit, cancel-by-default text-only opt-in.
use adw::{gtk, prelude::*};
use drivesync_engine::{ConflictRecord, TextMergeCandidate};

pub(super) fn page(
    account: &str,
    record: &ConflictRecord,
    candidate: &TextMergeCandidate,
) -> (gtk::Box, gtk::Button) {
    let (content, apply, _) = review(
        account,
        &record
            .relative_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        record.remote_version.unwrap_or_default(),
        [
            candidate.base(),
            candidate.local(),
            candidate.remote(),
            candidate.result(),
        ],
    );
    (content, apply)
}

/// The candidate is immutable. Scrolling or closing never dispatches a decision.
fn review(
    account: &str,
    path: &str,
    revision: u64,
    versions: [&[u8]; 4],
) -> (gtk::Box, gtk::Button, gtk::CheckButton) {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.append(&gtk::Label::builder().label(format!("Account: {account}\nFile: {path}\nObserved Drive revision: {revision}\n\nThis positional candidate may misalign compensating insertions or deletions. Review the whole result and its meaning. Applying replaces both active versions at the existing Drive ID. Base, local and Drive versions and the reviewed result are preserved. Changes since this preview block application; interrupted accepted effects may require recovery.")).use_markup(false).wrap(true).selectable(true).xalign(0.0).build());
    for (title, bytes) in [
        "Confirmed common base (B)",
        "Observed local version (L)",
        "Observed Drive version (R)",
        "Proposed merged result",
    ]
    .into_iter()
    .zip(versions)
    {
        content.append(&gtk::Label::builder().label(title).xalign(0.0).build());
        let view = gtk::TextView::builder()
            .editable(false)
            .monospace(true)
            .wrap_mode(gtk::WrapMode::WordChar)
            .build();
        view.buffer().set_text(&String::from_utf8_lossy(bytes));
        view.update_property(&[gtk::accessible::Property::Label(title)]);
        let scroll = gtk::ScrolledWindow::builder()
            .child(&view)
            .min_content_height(140)
            .max_content_height(240)
            .propagate_natural_height(true)
            .build();
        content.append(&scroll);
    }
    let reviewed = gtk::CheckButton::new();
    let consent = "I reviewed the whole plain-text candidate and accept these consequences";
    reviewed.set_child(Some(
        &gtk::Label::builder()
            .label(consent)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .xalign(0.0)
            .build(),
    ));
    reviewed.update_property(&[gtk::accessible::Property::Label(consent)]);
    content.append(&reviewed);
    let apply = gtk::Button::with_label("Apply merge");
    apply.add_css_class("destructive-action");
    apply.set_sensitive(false);
    let weak = apply.downgrade();
    reviewed.connect_toggled(move |button| {
        if let Some(apply) = weak.upgrade() {
            apply.set_sensitive(button.is_active());
        }
    });
    content.append(&apply);
    (content, apply, reviewed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an isolated GTK display"]
    fn review_preserves_all_versions_and_requires_explicit_opt_in() {
        adw::init().unwrap();
        let count = gtk::Window::list_toplevels().len();
        let (content, apply, reviewed) = review(
            "test@example.invalid",
            "<report>&.txt",
            2,
            [b"a\nb\n", b"A\nb\n", b"a\nB\n", b"A\nB\n"],
        );
        assert_eq!(gtk::Window::list_toplevels().len(), count);
        assert!(!apply.is_sensitive());
        reviewed.set_active(true);
        assert!(apply.is_sensitive());
        reviewed.set_active(false);
        assert!(!apply.is_sensitive());
        fn collect(widget: &gtk::Widget, views: &mut Vec<gtk::TextView>) {
            if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
                views.push(view.clone());
            }
            let mut child = widget.first_child();
            while let Some(current) = child {
                collect(&current, views);
                child = current.next_sibling();
            }
        }
        let mut views = vec![];
        collect(content.upcast_ref(), &mut views);
        assert_eq!(views.len(), 4);
        for (view, expected) in views.iter().zip(["a\nb\n", "A\nb\n", "a\nB\n", "A\nB\n"]) {
            assert!(!view.is_editable());
            assert_eq!(
                view.buffer()
                    .text(&view.buffer().start_iter(), &view.buffer().end_iter(), true),
                expected
            );
        }
    }
}
