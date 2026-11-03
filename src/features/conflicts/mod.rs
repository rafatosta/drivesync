//! Account-scoped GNOME problem center. Loading never contacts Google Drive.
mod actions;
mod comparison_page;
mod models;
mod text_merge_page;
use adw::{gtk, prelude::*};
use drivesync_engine::{DriveSyncEngine, RestoredAccount};
use std::sync::Arc;

/// Selects an existing review category without changing its resolution actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReviewKind {
    Conflicts,
    Problems,
}
impl ReviewKind {
    fn includes(self, category: models::Category) -> bool {
        (category == models::Category::Conflicts) == (self == Self::Conflicts)
    }
    fn title(self) -> &'static str {
        match self {
            Self::Conflicts => "Pending conflicts",
            Self::Problems => "Problems found",
        }
    }
}

/// Builds a category-specific entry into the existing account review window.
pub(crate) fn button_for(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    job: crate::features::synchronization::automatic::Controller,
    kind: ReviewKind,
) -> gtk::Button {
    review_button(engine, saved, job, Some(kind))
}

/// Loads durable pending observations off the GTK thread each time the user opens
/// the center. Closing the window has no engine effect and never acknowledges a
/// conflict. A failed/busy read stays an error rather than an empty success list.
pub(crate) fn button(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    job: crate::features::synchronization::automatic::Controller,
) -> gtk::Button {
    review_button(engine, saved, job, None)
}

fn review_button(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    job: crate::features::synchronization::automatic::Controller,
    kind: Option<ReviewKind>,
) -> gtk::Button {
    let button = gtk::Button::with_label(&gettextrs::gettext(
        kind.map_or("Conflicts and problems…", ReviewKind::title),
    ));

    button.set_widget_name(match kind {
        None => "problem-center",
        Some(ReviewKind::Conflicts) => "conflicts-center",
        Some(ReviewKind::Problems) => "errors-center",
    });
    button.set_sensitive(saved.folder.is_some());
    let account = saved.account.clone();
    let clicked_job = job.clone();
    button.connect_clicked(move |button| {
        actions::open(button, engine.clone(), &account, clicked_job.clone(), kind);
    });
    let weak = button.downgrade();
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        let Some(button) = weak.upgrade() else {
            return gtk::glib::ControlFlow::Break;
        };
        button.set_sensitive(job.folder().is_some());
        gtk::glib::ControlFlow::Continue
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
        if count == 0 {
            continue;
        }
        let group = adw::PreferencesGroup::builder()
            .title(format!("{} · {count}", category.title()))
            .build();
        for item in items.iter().filter(|item| item.category == category) {
            let row = adw::ActionRow::builder()
                .title(&item.path)
                .subtitle(item.details.lines().next().unwrap_or(category.title()))
                .use_markup(false)
                .activatable(center.is_some())
                .build();
            row.set_title_lines(0);
            row.set_subtitle_lines(2);
            row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
            if let Some(center) = center {
                let weak = std::rc::Rc::downgrade(center);
                let item = item.clone();
                row.connect_activated(move |_| {
                    if let Some(center) = weak.upgrade() {
                        center.detail(&item);
                    }
                });
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
    fn review_categories_partition_existing_items() {
        for category in [
            models::Category::Conflicts,
            models::Category::Incompatibilities,
            models::Category::Errors,
        ] {
            assert_ne!(
                ReviewKind::Conflicts.includes(category),
                ReviewKind::Problems.includes(category)
            );
            assert_eq!(
                ReviewKind::Conflicts.includes(category),
                category == models::Category::Conflicts
            );
        }
    }

    #[test]
    #[ignore = "requires a desktop display"]
    fn problem_rows_preserve_literal_paths_without_truncating_the_list() {
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
            if let Some(row) = widget.downcast_ref::<adw::ActionRow>() {
                *found += 1;
                assert!(!row.uses_markup());
                assert!(row.title().starts_with("<report>&"));
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
