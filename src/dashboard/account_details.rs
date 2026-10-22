//! Per-account information cards and related navigation actions.
//!
//! Renders account identity, stored folder settings, sanitized connection
//! state and explicit controls to configure folders or sign out.
use adw::{gtk, prelude::*};
use drivesync_engine::{DriveSyncEngine, RestoredAccount};
use std::sync::Arc;

/// Populates the dashboard with cards for the restored account profiles.
///
/// Saved metadata and authentication state remain distinct: accounts that
/// require reauthorization link back to welcome rather than opening setup.
pub(super) fn populate(
    engine: Arc<DriveSyncEngine>,
    accounts: Vec<RestoredAccount>,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    list: &gtk::ListBox,
    details: &gtk::Stack,
) {
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
}
