//! Native account navigation, independent of account loading and persistence.
use adw::{gtk, prelude::*};
use drivesync_engine::{Account, AccountId};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
pub(super) struct AccountSidebar {
    pub list: gtk::ListBox,
    accounts: Rc<RefCell<Vec<Account>>>,
}
#[derive(Clone)]
pub(super) struct SidebarHandle {
    list: gtk::glib::WeakRef<gtk::ListBox>,
    accounts: Rc<RefCell<Vec<Account>>>,
}
impl SidebarHandle {
    pub fn upgrade(&self) -> Option<AccountSidebar> {
        Some(AccountSidebar {
            list: self.list.upgrade()?,
            accounts: self.accounts.clone(),
        })
    }
}
impl AccountSidebar {
    pub fn downgrade(&self) -> SidebarHandle {
        SidebarHandle {
            list: self.list.downgrade(),
            accounts: self.accounts.clone(),
        }
    }
    pub fn new() -> Self {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .activate_on_single_click(true)
            .css_classes(["navigation-sidebar"])
            .build();
        list.set_widget_name("dashboard-accounts");
        Self {
            list,
            accounts: Rc::default(),
        }
    }
    pub fn selected(&self) -> Option<Account> {
        let index = self.list.selected_row()?.index() as usize;
        self.accounts.borrow().get(index).cloned()
    }
    /// Replaces the snapshot while retaining selection by account identity.
    pub fn set_accounts(&self, accounts: Vec<Account>) {
        let selected: Option<AccountId> = self.selected().map(|account| account.id);
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        *self.accounts.borrow_mut() = accounts.clone();
        let mut chosen = 0;
        for (index, account) in accounts.iter().enumerate() {
            let name = account
                .display_name
                .as_deref()
                .or(account.email.as_deref())
                .unwrap_or(&account.provider_user_id);
            let row = adw::ActionRow::builder()
                .title(name)
                .subtitle(account.email.as_deref().unwrap_or(""))
                .use_markup(false)
                .activatable(true)
                .build();
            row.set_title_lines(1);
            row.set_subtitle_lines(1);
            row.add_prefix(&adw::Avatar::new(32, Some(name), true));
            if selected.as_ref() == Some(&account.id) {
                chosen = index;
            }
            self.list.append(&row);
        }
        self.list
            .select_row(self.list.row_at_index(chosen as i32).as_ref());
    }
    pub fn on_selected(&self, callback: impl Fn(Option<Account>) + 'static) {
        let accounts = self.accounts.clone();
        self.list.connect_row_selected(move |_, row| {
            callback(row.and_then(|row| accounts.borrow().get(row.index() as usize).cloned()));
        });
    }
}

#[cfg(test)]
pub(super) fn exercise(account: &Account) {
    let sidebar = AccountSidebar::new();
    let mut second = account.clone();
    second.id = AccountId("sidebar-second".into());
    second.display_name = Some("Work <&>".into());
    sidebar.set_accounts(vec![account.clone(), second.clone()]);
    sidebar
        .list
        .select_row(sidebar.list.row_at_index(1).as_ref());
    assert_eq!(sidebar.selected().unwrap().id, second.id);
    sidebar.set_accounts(vec![second.clone(), account.clone()]);
    assert_eq!(sidebar.selected().unwrap().id, second.id);
    sidebar.set_accounts(vec![account.clone()]);
    assert_eq!(sidebar.selected().unwrap().id, account.id);
    sidebar.set_accounts(Vec::new());
    assert!(sidebar.selected().is_none());
}
