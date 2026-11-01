//! Full B/L/R/result review with an explicit, cancel-by-default text-only opt-in.
use adw::{gtk, prelude::*};
use drivesync_engine::{ConflictRecord, TextMergeCandidate};

pub(super) fn dialog(
    parent: &adw::Window,
    account: &str,
    record: &ConflictRecord,
    candidate: &TextMergeCandidate,
) -> adw::MessageDialog {
    review(
        parent,
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
    )
    .0
}

/// The candidate is immutable. Scrolling or closing never dispatches a decision.
fn review(
    parent: &adw::Window,
    account: &str,
    path: &str,
    revision: u64,
    versions: [&[u8]; 4],
) -> (adw::MessageDialog, gtk::CheckButton) {
    let dialog = adw::MessageDialog::new(Some(parent), Some("Review plain-text merge"), None);
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
    let scroll = crate::flow::scroll(&content, 760);
    scroll.set_min_content_height(340);
    scroll.set_max_content_height(480);
    scroll.set_propagate_natural_height(true);
    dialog.set_extra_child(Some(&scroll));
    dialog.add_responses(&[("cancel", "Decide later"), ("apply", "Apply merge")]);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.set_response_enabled("apply", false);
    dialog.set_response_appearance("apply", adw::ResponseAppearance::Destructive);
    let weak = dialog.downgrade();
    reviewed.connect_toggled(move |button| {
        if let Some(dialog) = weak.upgrade() {
            dialog.set_response_enabled("apply", button.is_active());
        }
    });
    (dialog, reviewed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a desktop display"]
    fn review_is_literal_complete_and_requires_explicit_opt_in() {
        adw::init().unwrap();
        let parent = adw::Window::builder()
            .default_width(360)
            .default_height(640)
            .build();
        let (dialog, reviewed) = review(
            &parent,
            "test@example.invalid",
            "<report>&.txt",
            2,
            [b"a\nb\n", b"A\nb\n", b"a\nB\n", b"A\nB\n"],
        );
        assert_eq!(dialog.default_response().as_deref(), Some("cancel"));
        assert_eq!(dialog.close_response(), "cancel");
        assert!(!dialog.is_response_enabled("apply"));
        reviewed.set_active(true);
        assert!(dialog.is_response_enabled("apply"));
        reviewed.set_active(false);
        assert!(!dialog.is_response_enabled("apply"));
        let mut views = Vec::new();
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
        collect(dialog.extra_child().unwrap().upcast_ref(), &mut views);
        assert_eq!(views.len(), 4);
        for (view, expected) in views.iter().zip(["a\nb\n", "A\nb\n", "a\nB\n", "A\nB\n"]) {
            assert!(!view.is_editable());
            assert_eq!(
                view.buffer()
                    .text(&view.buffer().start_iter(), &view.buffer().end_iter(), true),
                expected
            );
        }
        parent.present();
        dialog.set_default_size(360, 640);
        dialog.present();
        let loop_ = gtk::glib::MainLoop::new(None, false);
        let quit = loop_.clone();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
            quit.quit()
        });
        loop_.run();
        assert!(dialog.width() <= 360, "review must fit a narrow window");
        if let Ok(output) = std::env::var("DRIVESYNC_TEXT_MERGE_UI_OUTPUT") {
            std::fs::create_dir_all(&output).unwrap();
            let snapshot = gtk::Snapshot::new();
            gtk::WidgetPaintable::new(Some(&dialog)).snapshot(
                &snapshot,
                dialog.width() as f64,
                dialog.height() as f64,
            );
            dialog
                .renderer()
                .unwrap()
                .render_texture(snapshot.to_node().unwrap(), None)
                .save_to_png(std::path::Path::new(&output).join("text-merge-review-360.png"))
                .unwrap();
        }
        dialog.response("cancel");
        parent.close();
    }
}
