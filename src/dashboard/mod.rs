//! Read-only account dashboard for the DriveSync MVP.
//!
//! Builds account selection and hosts per-account details. Account cards and
//! their action controls are delegated to [`account_details`]; sync transfers
//! are not implemented by this screen.
mod account_details;

use adw::{gtk, prelude::*};
use drivesync_engine::{DriveSyncEngine, RestoredAccount};
use std::sync::Arc;

/// Builds the account overview page and its selectable detail stack.
pub fn build(
    engine: Arc<DriveSyncEngine>,
    accounts: Vec<RestoredAccount>,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) -> adw::NavigationPage {
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let add = gtk::Button::with_label("Add Account");
    let weak_nav = nav.downgrade();
    add.connect_clicked(move |_| {
        if let Some(nav) = weak_nav.upgrade() {
            nav.pop_to_tag("welcome");
        }
    });
    header.pack_end(&add);
    toolbar.add_top_bar(&header);
    let content = crate::flow::content();
    let title = gtk::Label::builder()
        .label("Your Google Drive folders")
        .xalign(0.0)
        .css_classes(["title-1"])
        .wrap(true)
        .build();
    content.append(&title);
    content.append(
        &gtk::Label::builder()
            .label("Select an account to view its folders and connection. File transfers are not available yet.")
            .xalign(0.0)
            .wrap(true)
            .build(),
    );
    let users = adw::PreferencesGroup::builder()
        .title("Accounts")
        .description("Current accounts in DriveSync")
        .build();
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .css_classes(["boxed-list"])
        .build();
    list.set_widget_name("account-list");
    users.add(&list);
    content.append(&users);
    let details = gtk::Stack::new();
    details.set_widget_name("account-details");
    let ids: Vec<_> = accounts.iter().map(|a| a.account.id.0.clone()).collect();
    account_details::populate(engine, accounts, nav, toasts, &list, &details);
    let weak_details = details.downgrade();
    list.connect_row_selected(move |_, row| {
        if let (Some(details), Some(row)) = (weak_details.upgrade(), row) {
            if let Some(id) = ids.get(row.index() as usize) {
                details.set_visible_child_name(id);
            }
        }
    });
    list.select_row(list.row_at_index(0).as_ref());
    content.append(&details);
    toolbar.set_content(Some(&crate::flow::scroll(&content, 920)));
    adw::NavigationPage::builder()
        .title("Dashboard")
        .child(&toolbar)
        .build()
}
