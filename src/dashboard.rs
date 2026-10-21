use adw::{gtk, prelude::*};
use drivesync_engine::{DriveSyncEngine, RestoredAccount};
use std::sync::Arc;
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
    for saved in accounts {
        let name = saved
            .account
            .email
            .as_deref()
            .unwrap_or(&saved.account.provider_subject);
        let state = match saved.error {
            None => "Connected",
            Some(
                drivesync_engine::AuthError::ReauthorizationRequired
                | drivesync_engine::AuthError::MissingPermissions,
            ) => "Reconnect required",
            Some(_) => "Connection unavailable",
        };
        let user = crate::flow::row(name, state, "avatar-default-symbolic");
        user.set_activatable(true);
        user.set_selectable(true);
        list.append(&user);
        let group = adw::PreferencesGroup::builder()
            .title(saved.account.email.as_deref().unwrap_or("Google Account"))
            .build();
        group.add(&crate::flow::row(
            "Account",
            &saved.account.provider_subject,
            "avatar-default-symbolic",
        ));
        if let Some(folder) = &saved.folder {
            group.add(&crate::flow::row(
                "Google Drive folder",
                &folder.remote.name,
                "folder-remote-symbolic",
            ));
            group.add(&crate::flow::row(
                "Local folder",
                &folder.local_display.to_string_lossy(),
                "folder-symbolic",
            ));
            group.add(&crate::flow::row(
                "Sync mode",
                "Download only · Not started",
                "folder-download-symbolic",
            ));
        }
        let state = saved
            .error
            .map(|e| e.to_string())
            .unwrap_or_else(|| "Connected".into());
        group.add(&crate::flow::row(
            "Connection",
            &state,
            "network-server-symbolic",
        ));
        if saved.error.is_none() {
            let button = gtk::Button::with_label(if saved.folder.is_some() {
                "Change Folders"
            } else {
                "Choose Folders"
            });
            let engine = engine.clone();
            let account = saved.account.clone();
            let weak_nav = nav.downgrade();
            let weak_toasts = toasts.downgrade();
            button.connect_clicked(move |_| {
                if let (Some(nav), Some(toasts)) = (weak_nav.upgrade(), weak_toasts.upgrade()) {
                    nav.push(&crate::setup::build(
                        engine.clone(),
                        account.clone(),
                        &nav,
                        &toasts,
                    ));
                }
            });
            group.add(&button);
        } else {
            let button = gtk::Button::with_label("Check or Reconnect Account");
            let weak_nav = nav.downgrade();
            button.connect_clicked(move |_| {
                if let Some(nav) = weak_nav.upgrade() {
                    nav.pop_to_tag("welcome");
                }
            });
            group.add(&button);
        }
        group.add(&crate::session::sign_out_button(
            engine.clone(),
            saved.account.id.clone(),
            nav,
            toasts,
        ));
        details.add_named(&group, Some(&saved.account.id.0));
    }
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
