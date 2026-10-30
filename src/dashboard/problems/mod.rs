//! Account-scoped GNOME problem center. Loading never contacts Google Drive.
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
        let parent = button.root().and_then(|root| root.downcast::<gtk::Window>().ok());
        let window = adw::Window::builder()
            .title("Conflicts and problems")
            .default_width(640).default_height(560).modal(true).build();
        window.set_transient_for(parent.as_ref());
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let later = gtk::Button::with_label("Decide later");
        let weak = window.downgrade();
        later.connect_clicked(move |_| { if let Some(window) = weak.upgrade() { window.close(); } });
        header.pack_end(&later);
        toolbar.add_top_bar(&header);
        let content = crate::flow::content();
        let identity = gtk::Label::builder().label(format!("Account: {}", account.email.as_deref().unwrap_or(&account.id.0)))
            .wrap(true).selectable(true).xalign(0.0).build();
        content.append(&identity);
        if let Some(result) = job.snapshot().result {
            let errors: Vec<_> = match result {
                Ok(report) => report.issues.iter().map(|issue| (issue.relative_path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Account".into()), issue.error.to_string())).collect(),
                Err(error) => vec![("Account".into(), error.to_string())],
            };
            if !errors.is_empty() {
                let group = adw::PreferencesGroup::builder().title("Errors from the latest cycle").description("Saved item observations below may describe an earlier check.").build();
                for (path, error) in errors {
                    let row = adw::ActionRow::builder().title(&path).subtitle(&error).use_markup(false).subtitle_selectable(true).build();
                    row.set_title_lines(0);
                    row.set_subtitle_lines(0);
                    group.add(&row);
                }
                content.append(&group);
            }
        }
        let status = gtk::Label::builder().label("Loading saved observations…").wrap(true).xalign(0.0).build();
        content.append(&status);
        toolbar.set_content(Some(&crate::flow::scroll(&content, 760)));
        window.set_content(Some(&toolbar));
        let engine = engine.clone();
        let id = account.id.clone();
        let weak_content = content.downgrade();
        let weak_status = status.downgrade();
        crate::flow::work(move || engine.saved_conflicts(&id), move |result| {
            let (Some(content), Some(status)) = (weak_content.upgrade(), weak_status.upgrade()) else { return; };
            match result {
                Err(error) => status.set_label(&format!("Saved problems could not be loaded: {error}. Close and reopen to retry. Existing pending items have not been cleared.")),
                Ok(records) => {
                    let items = models::pending(&records);
                    status.set_label(if items.is_empty() { "No saved pending items. This is not a new synchronization check." } else { "Saved observations — may be outdated. No network check was made. Decide later keeps these items pending." });
                    present_items(&content, &items);
                }
            }
        });
        window.present();
    });
    button
}

/// Renders literal, keyboard-expandable rows without limiting the pending list.
fn present_items(content: &gtk::Box, items: &[models::Item]) {
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
            category: models::Category::Conflicts,
            path: format!("<report>&{i}.txt"),
            details: "Local and Drive versions require comparison. Decide later keeps the pending item.".into(),
        }).collect();
        present_items(&content, &items);
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
