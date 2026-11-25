//! Account-scoped explanation of synchronization failures and blockers.
use super::conflicts;
use crate::{
    application::Context,
    services::synchronization::{self, StartFailure},
    ui::dashboard::sync::{controller::DashboardController, models::AttentionAction},
};
use adw::{gtk, prelude::*};
use drivesync_engine::{Account, Engine, StartSynchronizationRequest};
use gettextrs::gettext;
use std::{cell::RefCell, rc::Rc};

pub(super) fn show(
    navigation: &adw::NavigationView,
    context: &Context,
    engine: &Engine,
    account: &Account,
    controller: &DashboardController,
    failure: Option<&StartFailure>,
    dialogs: &Rc<RefCell<Vec<gtk::Window>>>,
) {
    if navigation
        .visible_page()
        .is_some_and(|page| page.widget_name() == "synchronization-diagnostic")
    {
        return;
    }
    let (title, description) = controller.status.diagnostic();
    let content = adw::PreferencesPage::new();
    let summary = adw::PreferencesGroup::builder().title(title).build();
    let details = adw::ActionRow::builder()
        .title(gettext("Detected problem"))
        .subtitle(if description.is_empty() {
            gettext("DriveSync could not determine the cause of the synchronization failure.")
        } else {
            description.clone()
        })
        .use_markup(false)
        .build();
    summary.add(&details);
    if let Some((request, folder)) = failure.and_then(|failure| {
        failure
            .confirmation
            .as_ref()
            .zip(failure.local_folder.as_deref())
    }) {
        summary.add(&confirmation_row(navigation, engine, request, folder));
    } else if let Some(folder) = failure.and_then(|failure| failure.local_folder.as_deref()) {
        let configure = adw::ActionRow::builder()
            .title(gettext("Configure folders"))
            .subtitle(folder)
            .use_markup(false)
            .activatable(true)
            .build();
        configure.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        let navigation = navigation.downgrade();
        let context = context.clone();
        let account = account.clone();
        configure.connect_activated(move |_| {
            if let Some(navigation) = navigation.upgrade() {
                if let Some(toasts) = navigation
                    .ancestor(adw::ToastOverlay::static_type())
                    .and_downcast::<adw::ToastOverlay>()
                {
                    crate::ui::setup::show(&context, &navigation, &toasts, Some(account.clone()));
                }
            }
        });
        summary.add(&configure);
    }
    content.add(&summary);

    let items = controller.attention.items();
    if !items.is_empty() {
        let issues = adw::PreferencesGroup::new();
        for issue in items {
            let subtitle = if issue.id == "recovery-blocker" {
                format!(
                    "{}\n{}",
                    issue.summary,
                    gettext("Keep the current copies while DriveSync checks the result.")
                )
            } else {
                issue.summary
            };
            let row = adw::ActionRow::builder()
                .title(issue.title)
                .subtitle(if subtitle == description {
                    String::new()
                } else {
                    subtitle
                })
                .use_markup(false)
                .build();
            if issue.action == AttentionAction::OpenConflictList {
                row.set_activatable(true);
                row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
                let engine = engine.clone();
                let account = account.clone();
                let dialogs = dialogs.clone();
                row.connect_activated(move |row| {
                    dialogs.borrow_mut().push(conflicts::present(
                        row,
                        engine.clone(),
                        account.clone(),
                    ));
                });
            }
            issues.add(&row);
        }
        content.add(&issues);
    }
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&content));
    let page = adw::NavigationPage::new(&toolbar, &gettext("Synchronization details"));
    page.set_widget_name("synchronization-diagnostic");
    page.connect_hidden({
        let dialogs = dialogs.clone();
        move |_| {
            for dialog in dialogs.borrow_mut().drain(..) {
                dialog.close();
            }
        }
    });
    navigation.push(&page);
}

fn confirmation_row(
    navigation: &adw::NavigationView,
    engine: &Engine,
    request: &StartSynchronizationRequest,
    folder: &str,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(gettext("Confirm original folder"))
        .subtitle(folder)
        .use_markup(false)
        .activatable(true)
        .build();
    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    let navigation = navigation.downgrade();
    let engine = engine.clone();
    let request = request.clone();
    row.connect_activated(move |row| {
        let Some(window) = row.root().and_downcast::<gtk::Window>() else { return; };
        let dialog = adw::MessageDialog::new(
            Some(&window),
            Some(&gettext("Confirm original folder")),
            Some(&gettext("Confirm only if this is the same folder you previously synchronized. A copy, a restored snapshot, or a newly created folder must be configured separately.")),
        );
        dialog.add_responses(&[("cancel", &gettext("Cancel")), ("confirm", &gettext("Confirm"))]);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let row = row.downgrade();
        let navigation = navigation.clone();
        let request = request.clone();
        let engine = engine.clone();
        dialog.connect_response(Some("confirm"), move |_, _| {
            let Some(row) = row.upgrade() else { return; };
            row.set_sensitive(false);
            let row = row.downgrade();
            let navigation = navigation.clone();
            let engine = engine.clone();
            let request = request.clone();
            gtk::glib::spawn_future_local(async move {
                let result = synchronization::confirm_local_root(&engine, request).await;
                let Some(row) = row.upgrade() else { return; };
                row.set_sensitive(true);
                match result {
                    Ok(()) => {
                        if let Some(navigation) = navigation.upgrade() {
                            navigation.pop();
                            let _ = navigation.activate_action("dashboard.refresh", None);
                        }
                    }
                    Err(error) => row.set_subtitle(&error),
                }
            });
        });
        dialog.present();
    });
    row
}
