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
    let add = gtk::Button::with_label("Accounts");
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
            .label("Your folder settings are saved. File transfers are not available yet.")
            .xalign(0.0)
            .wrap(true)
            .build(),
    );
    for saved in accounts {
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
        content.append(&group);
    }
    toolbar.set_content(Some(&crate::flow::scroll(&content, 920)));
    adw::NavigationPage::builder()
        .title("Dashboard")
        .child(&toolbar)
        .build()
}
