//! Account-scoped GNOME problem center. Loading never contacts Google Drive.
mod actions;
mod models;
use adw::{gtk, prelude::*};
use drivesync_engine::{DriveSyncEngine, RestoredAccount};
use std::sync::Arc;

/// Loads durable pending observations off the GTK thread each time the user opens
/// the center. Closing the window has no engine effect and never acknowledges a
/// conflict. A failed/busy read stays an error rather than an empty success list.
pub(super) fn button(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    job: crate::automatic::Controller,
) -> gtk::Button {
    let button = gtk::Button::with_label("Conflicts and problems…");
    button.set_widget_name("problem-center");
    button.set_sensitive(saved.folder.is_some());
    let account = saved.account.clone();
    button.connect_clicked(move |button| {
        actions::open(button, engine.clone(), &account, job.clone());
    });
    button
}

/// Renders literal, keyboard-expandable rows without limiting the pending list.
fn present_items(
    content: &gtk::Box,
    items: &[models::Item],
    center: Option<&std::rc::Rc<actions::Center>>,
) {
    for category in [
        models::Category::Conflicts,
        models::Category::Incompatibilities,
        models::Category::Errors,
    ] {
        let count = items
            .iter()
            .filter(|item| item.category == category)
            .count();
        let group = adw::PreferencesGroup::builder()
            .title(format!("{} · {count}", category.title()))
            .build();
        for item in items.iter().filter(|item| item.category == category) {
            let row = adw::ExpanderRow::builder()
                .title(&item.path)
                .subtitle(category.title())
                .use_markup(false)
                .build();
            row.set_title_lines(0);
            let details = adw::ActionRow::builder()
                .title("Observed versions and next steps")
                .subtitle(&item.details)
                .use_markup(false)
                .subtitle_selectable(true)
                .build();
            details.set_subtitle_lines(0);
            row.add_row(&details);
            if let (Some(center), Some(record)) = (center, &item.record) {
                actions::add_choices(&row, center, record);
            }
            group.add(&row);
        }
        content.append(&group);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a desktop display"]
    fn problem_rows_are_literal_selectable_and_keyboard_expandable() {
        adw::init().unwrap();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let items: Vec<_> = (0..12).map(|i| models::Item {
            record: None,
            category: models::Category::Conflicts,
            path: format!("<report>&{i}.txt"),
            details: "Local and Drive versions require comparison. Decide later keeps the pending item.".into(),
        }).collect();
        present_items(&content, &items, None);
        fn rows(widget: &gtk::Widget, found: &mut usize) {
            if let Some(row) = widget.downcast_ref::<adw::ExpanderRow>() {
                *found += 1;
                assert!(!row.uses_markup());
                assert!(row.title().starts_with("<report>&"));
                row.set_expanded(true);
                assert!(row.is_expanded());
            }
            if let Some(row) = widget.downcast_ref::<adw::ActionRow>() {
                if row.title() == "Observed versions and next steps" {
                    assert!(row.is_subtitle_selectable());
                    assert!(!row.uses_markup());
                }
            }
            let mut child = widget.first_child();
            while let Some(current) = child {
                rows(&current, found);
                child = current.next_sibling();
            }
        }
        let mut count = 0;
        rows(content.upcast_ref(), &mut count);
        assert_eq!(count, 12);
    }
}
