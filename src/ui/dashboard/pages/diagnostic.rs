//! Account-scoped explanation of synchronization failures and blockers.
use super::conflicts;
use crate::{
    application::Context,
    ui::dashboard::sync::{controller::DashboardController, models::AttentionAction},
};
use adw::{gtk, prelude::*};
use drivesync_engine::{Account, Engine};
use gettextrs::gettext;
use std::{cell::RefCell, rc::Rc};

pub(super) fn show(
    navigation: &adw::NavigationView,
    context: &Context,
    engine: &Engine,
    account: &Account,
    controller: &DashboardController,
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
    if let Some(item) = controller.status.item_detail() {
        let item_details = adw::ActionRow::builder()
            .title(gettext("Item"))
            .subtitle(item)
            .use_markup(false)
            .build();
        summary.add(&item_details);
    }
    {
        let configure = adw::ActionRow::builder()
            .title(gettext("Configure folders"))
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

    let issues = adw::PreferencesGroup::new();
    let mut has_details = false;
    for issue in controller.attention.items() {
        if issue.action != AttentionAction::OpenConflictList
            && issue.id != "recovery-blocker"
            && (description == issue.summary
                || description.starts_with(&format!("{}\n", issue.summary)))
        {
            continue;
        }
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
                dialogs
                    .borrow_mut()
                    .push(conflicts::present(row, engine.clone(), account.clone()));
            });
        }
        issues.add(&row);
        has_details = true;
    }
    if has_details {
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
