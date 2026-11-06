//! Day groups and individual observed activity rows.
use adw::{gtk, prelude::*};
use drivesync_engine::{AccountId, ActivityRecord, LocalActivity, LocalItemKind, RemoteActivity};
use gettextrs::gettext;
use std::path::Path;
use std::rc::Rc;

pub(super) fn render_days<'a>(
    list: &gtk::Box,
    records: Vec<ActivityRecord>,
    account: impl Fn(&AccountId) -> Option<&'a str>,
    selected: Option<i64>,
    activate: impl Fn(&ActivityRecord, &gtk::ListBoxRow) + 'static,
) -> Option<adw::ActionRow> {
    let today = gtk::glib::DateTime::now_local().ok();
    let yesterday = today.as_ref().and_then(|date| date.add_days(-1).ok());
    let mut group: Option<(String, gtk::ListBox)> = None;
    let activate = Rc::new(activate);
    let mut selected_row = None;
    for record in records {
        let Ok(date) = gtk::glib::DateTime::from_unix_local(record.observed_at) else {
            continue;
        };
        let key = formatted(&date, "%F");
        if group.as_ref().is_none_or(|(day, _)| day != &key) {
            let title = if today
                .as_ref()
                .is_some_and(|day| formatted(day, "%F") == key)
            {
                gettext("Today")
            } else if yesterday
                .as_ref()
                .is_some_and(|day| formatted(day, "%F") == key)
            {
                gettext("Yesterday")
            } else {
                formatted(&date, "%x")
            };
            let (section, rows) = day_group(&title);
            list.append(&section);
            group = Some((key, rows));
        }
        if let Some((_, rows)) = &group {
            let row = activity_row(
                &record,
                account(&record.account),
                &formatted(&date, "%H:%M"),
            );
            rows.append(&row);
            if selected == Some(record.id) {
                rows.select_row(Some(&row));
                selected_row = Some(row.clone());
            }
            let activate = activate.clone();
            row.connect_activated(move |row| activate(&record, row.upcast_ref()));
        }
    }
    selected_row
}

fn day_group(title: &str) -> (gtk::Box, gtk::ListBox) {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let heading = gtk::Label::new(Some(title));
    heading.set_xalign(0.0);
    heading.add_css_class("heading");
    section.append(&heading);
    let rows = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .css_classes(["boxed-list"])
        .build();
    section.append(&rows);
    (section, rows)
}

fn activity_row(record: &ActivityRecord, account: Option<&str>, time: &str) -> adw::ActionRow {
    let notice = &record.notice;
    let mut detail = match &notice.activity {
        LocalActivity::Moved { from } => {
            format!("{} → {}", path_text(from), path_text(&notice.path))
        }
        _ => path_text(&notice.path),
    };
    if let Some(account) = account {
        detail.push_str(" · ");
        detail.push_str(account);
    }
    if record.remote.is_some() {
        detail.push_str(" · ");
        detail.push_str(&origin(record));
    }
    let row = adw::ActionRow::builder()
        .title(activity_title(record))
        .subtitle(&detail)
        .use_markup(false)
        .activatable(true)
        .selectable(true)
        .build();
    row.set_subtitle_lines(2);
    row.set_tooltip_text(Some(&detail));
    row.add_prefix(&gtk::Image::from_icon_name(activity_icon(record)));
    let time = gtk::Label::new(Some(time));
    time.add_css_class("dim-label");
    row.add_suffix(&time);
    row
}

fn activity_icon(record: &ActivityRecord) -> &'static str {
    let notice = &record.notice;
    let create = if notice.object_type == LocalItemKind::Directory {
        "folder-new-symbolic"
    } else {
        "document-new-symbolic"
    };
    if let Some(action) = record.remote {
        return match action {
            RemoteActivity::Added if notice.object_type == LocalItemKind::Directory => create,
            RemoteActivity::Added => "folder-download-symbolic",
            RemoteActivity::IntegrityFailed => "dialog-error-symbolic",
            RemoteActivity::Removed => "user-trash-symbolic",
            RemoteActivity::Renamed => "insert-text-symbolic",
            RemoteActivity::Moved => "go-jump-symbolic",
            RemoteActivity::Updated
            | RemoteActivity::RenamedAndUpdated
            | RemoteActivity::MovedAndUpdated => "document-edit-symbolic",
        };
    }
    match &notice.activity {
        LocalActivity::Created => create,
        LocalActivity::Removed => "user-trash-symbolic",
        LocalActivity::MovedIn => "go-next-symbolic",
        LocalActivity::MovedOut => "go-previous-symbolic",
        LocalActivity::Moved { from } if from.parent() == notice.path.parent() => {
            "insert-text-symbolic"
        }
        LocalActivity::Moved { .. } => "go-jump-symbolic",
        LocalActivity::Written => "document-edit-symbolic",
        LocalActivity::ClosedAfterWrite => "document-save-symbolic",
        LocalActivity::MetadataChanged => "document-properties-symbolic",
        LocalActivity::Recheck(_) => "dialog-warning-symbolic",
    }
}

pub(super) fn origin(record: &ActivityRecord) -> String {
    if record.remote.is_some() {
        gettext("Google Drive → local folder")
    } else {
        gettext("Local folder")
    }
}

pub(super) fn activity_title(record: &ActivityRecord) -> String {
    let notice = &record.notice;
    let kind = match notice.object_type {
        LocalItemKind::File => gettext("File"),
        LocalItemKind::Directory => gettext("Folder"),
        LocalItemKind::Symlink => gettext("Link"),
        _ => gettext("Item"),
    };
    if let Some(remote) = record.remote {
        let action = match remote {
            RemoteActivity::Added if notice.object_type == LocalItemKind::Directory => {
                gettext("Created")
            }
            RemoteActivity::Added => gettext("Downloaded"),
            RemoteActivity::IntegrityFailed => gettext("Download integrity check failed"),
            RemoteActivity::Renamed => gettext("Renamed"),
            RemoteActivity::Moved => gettext("Moved"),
            RemoteActivity::MovedAndUpdated => gettext("Moved and updated"),
            RemoteActivity::Updated => gettext("Updated"),
            RemoteActivity::RenamedAndUpdated => gettext("Renamed and updated"),
            RemoteActivity::Removed => gettext("Removed from the local folder"),
        };
        return format!("{kind} · {action}");
    }
    let action = super::super::local_history::activity_label(notice).unwrap_or_default();
    if notice.activity == LocalActivity::Created {
        format!("{kind} · {}", gettext("Created locally"))
    } else if notice.activity == LocalActivity::Removed {
        format!("{kind} · {}", gettext("Removed from the local folder"))
    } else {
        format!("{kind} · {action}")
    }
}

pub(super) fn formatted(date: &gtk::glib::DateTime, pattern: &str) -> String {
    date.format(pattern)
        .map(|value| value.to_string())
        .unwrap_or_default()
}

pub(super) fn path_text(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| if c.is_control() { '\u{fffd}' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use drivesync_engine::LocalNotice;

    #[test]
    #[ignore = "requires a desktop display"]
    fn activity_rows_group_dates_and_display_both_move_paths() {
        adw::init().unwrap();
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let today = gtk::glib::DateTime::now_local().unwrap();
        let records = [today.clone(), today.add_days(-1).unwrap()].map(|date| ActivityRecord {
            id: date.to_unix(),
            account: AccountId("test".into()),
            observed_at: date.to_unix(),
            remote: Some(RemoteActivity::Renamed),
            integrity: None,
            notice: LocalNotice {
                path: "new/report.pdf".into(),
                object_type: LocalItemKind::File,
                activity: LocalActivity::Moved {
                    from: "old/report.pdf".into(),
                },
            },
        });
        let split = adw::NavigationSplitView::new();
        split.set_collapsed(true);
        let (details, panel) = super::super::details::Details::new(&split);
        split.set_sidebar(Some(&adw::NavigationPage::new(&list, "Activities")));
        split.set_content(Some(&adw::NavigationPage::new(&details, "Details")));
        let weak_split = split.downgrade();
        render_days(
            &list,
            records.into(),
            |_| Some("Test account"),
            None,
            move |record, _| {
                panel.show(record, "Test account");
                weak_split.upgrade().unwrap().set_show_content(true);
            },
        );
        let first = list.first_child().unwrap();
        let second = first.next_sibling().unwrap();
        assert!(second.next_sibling().is_none());
        let heading = first
            .first_child()
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();
        assert_eq!(heading.text(), gettext("Today"));
        let group = first
            .last_child()
            .unwrap()
            .downcast::<gtk::ListBox>()
            .unwrap();
        let row = group
            .row_at_index(0)
            .unwrap()
            .downcast::<adw::ActionRow>()
            .unwrap();
        assert_eq!(
            row.subtitle().as_deref(),
            Some(
                format!(
                    "old/report.pdf → new/report.pdf · Test account · {}",
                    gettext("Google Drive → local folder")
                )
                .as_str()
            )
        );
        adw::prelude::ActionRowExt::activate(&row);
        assert!(split.property::<bool>("show-content"));
        let fields = details.last_child().unwrap();
        let new_path = fields
            .last_child()
            .unwrap()
            .last_child()
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();
        assert_eq!(new_path.text(), "new/report.pdf");
        let old_path = fields
            .last_child()
            .unwrap()
            .prev_sibling()
            .unwrap()
            .last_child()
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();
        assert_eq!(old_path.text(), "old/report.pdf");
        details
            .first_child()
            .unwrap()
            .downcast::<gtk::Button>()
            .unwrap()
            .emit_clicked();
        assert!(!split.property::<bool>("show-content"));
    }
}
