//! Day groups and individual observed activity rows.
use adw::{gtk, prelude::*};
use drivesync_engine::{AccountId, ActivityRecord, LocalActivity, LocalItemKind};
use gettextrs::gettext;
use std::path::Path;

pub(super) fn render_days<'a>(
    list: &gtk::Box,
    records: Vec<ActivityRecord>,
    account: impl Fn(&AccountId) -> Option<&'a str>,
) {
    let today = gtk::glib::DateTime::now_local().ok();
    let yesterday = today.as_ref().and_then(|date| date.add_days(-1).ok());
    let mut group: Option<(String, gtk::ListBox)> = None;
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
            rows.append(&activity_row(
                &record,
                account(&record.account),
                &formatted(&date, "%H:%M"),
            ));
        }
    }
}

fn day_group(title: &str) -> (gtk::Box, gtk::ListBox) {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let heading = gtk::Label::new(Some(title));
    heading.set_xalign(0.0);
    heading.add_css_class("heading");
    section.append(&heading);
    let rows = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    section.append(&rows);
    (section, rows)
}

fn activity_row(record: &ActivityRecord, account: Option<&str>, time: &str) -> adw::ActionRow {
    let notice = &record.notice;
    let kind = match notice.object_type {
        LocalItemKind::File => gettext("File"),
        LocalItemKind::Directory => gettext("Folder"),
        LocalItemKind::Symlink => gettext("Link"),
        _ => gettext("Item"),
    };
    let action = super::super::local_history::activity_label(notice).unwrap_or_default();
    let title = if notice.activity == LocalActivity::Removed {
        format!("{kind} · {}", gettext("Removed from the local folder"))
    } else {
        format!("{kind} · {action}")
    };
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
    let icon = match notice.activity {
        LocalActivity::Created | LocalActivity::MovedIn => "list-add-symbolic",
        LocalActivity::Removed | LocalActivity::MovedOut => "edit-delete-symbolic",
        LocalActivity::Moved { .. } => "go-jump-symbolic",
        _ => "document-edit-symbolic",
    };
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(&detail)
        .use_markup(false)
        .activatable(false)
        .selectable(false)
        .build();
    row.set_subtitle_lines(2);
    row.set_tooltip_text(Some(&detail));
    row.add_prefix(&gtk::Image::from_icon_name(icon));
    let time = gtk::Label::new(Some(time));
    time.add_css_class("dim-label");
    row.add_suffix(&time);
    row
}

fn formatted(date: &gtk::glib::DateTime, pattern: &str) -> String {
    date.format(pattern)
        .map(|value| value.to_string())
        .unwrap_or_default()
}

fn path_text(path: &Path) -> String {
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
            notice: LocalNotice {
                path: "new/report.pdf".into(),
                object_type: LocalItemKind::File,
                activity: LocalActivity::Moved {
                    from: "old/report.pdf".into(),
                },
            },
        });
        render_days(&list, records.into(), |_| Some("Test account"));
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
            Some("old/report.pdf → new/report.pdf · Test account")
        );
    }
}
