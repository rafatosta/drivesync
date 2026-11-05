//! Combined session activity across the dashboard's accounts.
use super::local_history::activity_row;
use adw::{gtk, prelude::*};
use drivesync_engine::{LocalBatch, RestoredAccount};
use gettextrs::gettext;

pub(in crate::features::dashboard) const PAGE: &str = "activities";

pub(in crate::features::dashboard) struct Activities {
    pub root: gtk::Box,
    list: gtk::ListBox,
    empty: gtk::Label,
    accounts: Vec<String>,
}

impl Activities {
    pub fn new(accounts: &[RestoredAccount]) -> Self {
        let accounts: Vec<_> = accounts
            .iter()
            .map(|account| {
                account
                    .account
                    .email
                    .as_ref()
                    .unwrap_or(&account.account.id.0)
                    .clone()
            })
            .collect();
        let root = crate::ui::content();
        root.set_widget_name("activities-page");
        let group = adw::PreferencesGroup::new();
        let choices = gtk::StringList::new(&[]);
        choices.append(&gettext("All accounts"));
        for account in &accounts {
            choices.append(account);
        }
        let filter = adw::ComboRow::builder()
            .title(gettext("Account"))
            .model(&choices)
            .build();
        filter.set_widget_name("activities-account-filter");
        group.add(&filter);
        root.append(&group);

        let empty = gtk::Label::new(Some(&gettext("No changes detected in this session.")));
        empty.add_css_class("dim-label");
        empty.set_wrap(true);
        root.append(&empty);
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        list.set_widget_name("activities-list");
        root.append(&list);

        let weak_filter = filter.downgrade();
        list.set_filter_func(move |row| {
            weak_filter.upgrade().is_some_and(|filter| {
                filter.selected() == 0 || row.widget_name() == filter.selected().to_string()
            })
        });
        let (weak_list, weak_empty) = (list.downgrade(), empty.downgrade());
        filter.connect_selected_notify(move |_| {
            if let (Some(list), Some(empty)) = (weak_list.upgrade(), weak_empty.upgrade()) {
                list.invalidate_filter();
                refresh_empty(&list, &empty);
            }
        });
        Self {
            root,
            list,
            empty,
            accounts,
        }
    }

    pub fn receiver(&self, index: usize) -> impl Fn(&LocalBatch) + 'static {
        let (list, empty) = (self.list.downgrade(), self.empty.downgrade());
        let account = self.accounts[index].clone();
        let filter_key = (index + 1).to_string();
        move |batch| {
            let (Some(list), Some(empty)) = (list.upgrade(), empty.upgrade()) else {
                return;
            };
            for notice in &batch.notices {
                if let Some(row) = activity_row(notice, Some(&account)) {
                    row.set_widget_name(&filter_key);
                    list.prepend(&row);
                    if let Some(oldest) = list.row_at_index(200) {
                        list.remove(&oldest);
                    }
                }
            }
            refresh_empty(&list, &empty);
        }
    }
}

fn refresh_empty(list: &gtk::ListBox, empty: &gtk::Label) {
    empty.set_visible(
        !(0..200)
            .map_while(|i| list.row_at_index(i))
            .any(|row| row.is_child_visible()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use drivesync_engine::{
        AccountId, AccountSummary, LocalActivity, LocalItemKind, LocalNotice, LocalScope,
        LocalWatchState,
    };

    #[test]
    #[ignore = "requires a desktop display"]
    fn activities_filters_events_from_two_accounts() {
        adw::init().unwrap();
        let accounts = ["first", "second"].map(|name| RestoredAccount {
            account: AccountSummary {
                id: AccountId(name.into()),
                provider_subject: name.into(),
                email: Some(format!("{name}@example.com")),
            },
            error: None,
            folder: None,
        });
        let page = Activities::new(&accounts);
        for (index, account) in accounts.iter().enumerate() {
            page.receiver(index)(&LocalBatch {
                scope: LocalScope::new(
                    "client",
                    &account.account.id.0,
                    std::path::Path::new("/test"),
                )
                .unwrap(),
                state: LocalWatchState::Observing,
                notices: vec![LocalNotice {
                    path: format!("{}.txt", account.account.id.0).into(),
                    object_type: LocalItemKind::File,
                    activity: LocalActivity::Created,
                }],
            });
        }
        let first = page.list.row_at_index(1).unwrap();
        let second = page.list.row_at_index(0).unwrap();
        assert!(first.is_child_visible() && second.is_child_visible());
        let group = page
            .root
            .first_child()
            .unwrap()
            .downcast::<adw::PreferencesGroup>()
            .unwrap();
        fn find_filter(widget: &gtk::Widget) -> Option<adw::ComboRow> {
            if let Ok(filter) = widget.clone().downcast::<adw::ComboRow>() {
                return Some(filter);
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                if let Some(filter) = find_filter(&widget) {
                    return Some(filter);
                }
                child = widget.next_sibling();
            }
            None
        }
        let filter = find_filter(group.upcast_ref()).unwrap();
        filter.set_selected(1);
        assert!(first.is_child_visible());
        assert!(!second.is_child_visible());
        filter.set_selected(2);
        assert!(!first.is_child_visible());
        assert!(second.is_child_visible());
    }
}
