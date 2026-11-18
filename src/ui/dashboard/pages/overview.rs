//! Adaptive GNOME account navigation; account settings remain in Preferences.
use super::sidebar::AccountSidebar;
use crate::application::Context;
use crate::ui::components::sync_status::{SyncActivity, SyncStatus, SyncStatusData};
use adw::{gtk, prelude::*};
use drivesync_engine::{ListAccountsRequest, SynchronizationActivity, SynchronizationStatus};
use gettextrs::gettext;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub(crate) fn build(context: &Context, navigation: &adw::NavigationView) -> adw::NavigationPage {
    build_with_accounts(context, navigation, Vec::new())
}
fn build_with_accounts(
    context: &Context,
    navigation: &adw::NavigationView,
    accounts: Vec<drivesync_engine::Account>,
) -> adw::NavigationPage {
    let split = adw::OverlaySplitView::builder()
        .min_sidebar_width(240.0)
        .max_sidebar_width(320.0)
        .sidebar_width_fraction(0.3)
        .build();
    split.set_widget_name("dashboard-split");
    let feedback = gtk::Label::builder()
        .wrap(true)
        .margin_start(12)
        .margin_end(12)
        .build();
    feedback.set_widget_name("dashboard-accounts-feedback");
    let sidebar = AccountSidebar::new().with_menu({
        let context = context.clone();
        let navigation = navigation.downgrade();
        let feedback = feedback.downgrade();
        move |account| super::account_menu::build(&context, &navigation, &feedback, account)
    });
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_show_back_button(false);
    header.set_title_widget(Some(&adw::WindowTitle::new("DriveSync", "")));
    header.pack_start(&crate::ui::components::main_menu::add_account_button());
    header.pack_end(&crate::ui::components::main_menu::button());
    let developer_console = gtk::Button::builder()
        .icon_name("utilities-terminal-symbolic")
        .tooltip_text("Developer Console (F12)")
        .action_name("win.developer-console")
        .build();
    header.pack_end(&developer_console);
    toolbar.add_top_bar(&header);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    body.append(&sidebar.heading);
    body.append(&sidebar.list);
    body.append(&feedback);
    toolbar.set_content(Some(
        &gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&body)
            .build(),
    ));
    let version = gtk::Label::builder()
        .label(crate::core::build_info::VERSION)
        .tooltip_text(format!(
            "Source revision: {}",
            crate::core::build_info::REVISION
        ))
        .css_classes(["dim-label", "caption"])
        .selectable(true)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    version.set_widget_name("dashboard-build-version");
    toolbar.add_bottom_bar(&version);
    split.set_sidebar(Some(&adw::NavigationPage::new(&toolbar, "DriveSync")));

    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    split
        .bind_property("collapsed", &header, "show-back-button")
        .sync_create()
        .build();
    let title = adw::WindowTitle::new("DriveSync", "");
    title.set_widget_name("dashboard-account-title");
    header.set_title_widget(Some(&title));
    let sidebar_toggle = gtk::ToggleButton::builder()
        .icon_name("adw-sidebar-symbolic")
        .tooltip_text(gettext("Toggle sidebar"))
        .css_classes(["flat"])
        .build();
    sidebar_toggle.set_widget_name("dashboard-sidebar-toggle");
    split
        .bind_property("show-sidebar", &sidebar_toggle, "active")
        .bidirectional()
        .sync_create()
        .build();
    header.pack_start(&sidebar_toggle);
    toolbar.add_top_bar(&header);
    let empty = gtk::Box::new(gtk::Orientation::Vertical, 0);
    empty.set_widget_name("dashboard-content");
    empty.set_margin_top(24);
    empty.set_margin_bottom(24);
    empty.set_margin_start(24);
    empty.set_margin_end(24);
    let status = SyncStatus::new();
    status.widget().set_visible(false);
    empty.append(status.widget());
    toolbar.set_content(Some(&empty));
    split.set_content(Some(&adw::NavigationPage::new(&toolbar, "DriveSync")));
    let status_subscription = Rc::new(RefCell::new(None::<gtk::glib::JoinHandle<()>>));
    sidebar.on_selected({
        let title = title.downgrade();
        let status = status.clone();
        let status_subscription = status_subscription.clone();
        let context = context.clone();
        move |account| {
            if let Some(title) = title.upgrade() {
                title.set_title(
                    account
                        .as_ref()
                        .and_then(|account| {
                            account.display_name.as_deref().or(account.email.as_deref())
                        })
                        .unwrap_or("DriveSync"),
                );
                title.set_subtitle(
                    account
                        .as_ref()
                        .and_then(|account| account.email.as_deref())
                        .unwrap_or(""),
                );
            }
            if let Some(subscription) = status_subscription.borrow_mut().take() {
                subscription.abort();
            }
            let Some(account) = account else {
                status.widget().set_visible(false);
                return;
            };
            status.update(&SyncStatusData::default());
            let Some(engine) = context.engine.borrow().clone() else {
                return;
            };
            let mut subscription = engine.observe_synchronization(&account.id);
            status.update(&presentation_status(&subscription.snapshot()));
            let status = status.clone();
            *status_subscription.borrow_mut() = Some(gtk::glib::spawn_future_local(async move {
                while subscription.changed().await {
                    status.update(&presentation_status(&subscription.snapshot()));
                }
            }));
        }
    });
    sidebar.set_accounts(accounts);
    let weak_split = split.downgrade();
    sidebar.list.connect_row_activated(move |_, _| {
        if let Some(split) = weak_split.upgrade() {
            if split.is_collapsed() {
                split.set_show_sidebar(false);
            }
        }
    });

    let generation = Rc::new(Cell::new(0_u64));
    let refresh: Rc<dyn Fn()> = Rc::new({
        let context = context.clone();
        let sidebar = sidebar.downgrade();
        let feedback = feedback.downgrade();
        move || {
            let Some(sidebar) = sidebar.upgrade() else {
                return;
            };
            let Some(engine) = context.engine.borrow().clone() else {
                return;
            };
            let request = generation.get().wrapping_add(1);
            generation.set(request);
            let generation = generation.clone();
            let feedback = feedback.clone();
            gtk::glib::spawn_future_local(async move {
                let result = async {
                    let result = engine.list_accounts(ListAccountsRequest { provider: None }).await
                        .map_err(|error| crate::core::messages::engine_error(&error))?;
                    let mut accounts = Vec::new();
                    for account in result.accounts {
                        match crate::services::configuration::load(&account.id).await {
                            Ok(Some(_)) => accounts.push(account),
                            Ok(None) => (),
                            Err(_) => return Err(gettext("Saved configuration could not be read. Your account has been preserved.")),
                        }
                    }
                    Ok(accounts)
                }.await;
                if generation.get() != request {
                    return;
                }
                let Some(feedback) = feedback.upgrade() else {
                    return;
                };
                match result {
                    Ok(accounts) => {
                        sidebar.set_accounts(accounts);
                        feedback.set_label("");
                    }
                    Err(message) => feedback.set_label(&message),
                }
            });
        }
    });
    let on_map = refresh.clone();
    sidebar.list.connect_map(move |_| on_map());
    let group = gtk::gio::SimpleActionGroup::new();
    let action = gtk::gio::SimpleAction::new("refresh", None);
    action.connect_activate(move |_, _| refresh());
    group.add_action(&action);
    navigation.insert_action_group("dashboard", Some(&group));

    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        660.0,
        adw::LengthUnit::Sp,
    ));
    breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
    let bin = adw::BreakpointBin::new();
    bin.set_size_request(320, 320);
    bin.set_child(Some(&split));
    bin.add_breakpoint(breakpoint);
    let page = adw::NavigationPage::new(&bin, "DriveSync");
    page.set_widget_name("drivesync-dashboard");
    page
}

/// Converts the engine's account snapshot into the reusable GTK presentation model.
fn presentation_status(status: &SynchronizationStatus) -> SyncStatusData {
    use SynchronizationActivity as EngineActivity;
    let activity = match status.activity {
        EngineActivity::Idle => SyncActivity::Idle,
        EngineActivity::Starting | EngineActivity::Scanning => SyncActivity::Scanning,
        EngineActivity::Analyzing
        | EngineActivity::ProcessingEvents
        | EngineActivity::Checking
        | EngineActivity::Applying
        | EngineActivity::Verifying => SyncActivity::Reconciling,
        EngineActivity::Uploading => SyncActivity::Upload,
        EngineActivity::Downloading => SyncActivity::Download,
        EngineActivity::Waiting | EngineActivity::Stopped | EngineActivity::Failed => {
            SyncActivity::Idle
        }
    };
    let (transferred_bytes, total_bytes) = match status.progress {
        Some(progress) if progress.unit == drivesync_engine::SynchronizationProgressUnit::Bytes => {
            (Some(progress.completed), progress.total)
        }
        _ => (None, None),
    };
    SyncStatusData {
        activity,
        transferred_bytes,
        total_bytes,
        problems: status.issue.as_ref().map_or(0, |issue| issue.count),
        ..Default::default()
    }
}

#[cfg(test)]
pub(crate) fn fixture(
    navigation: &adw::NavigationView,
    account: &drivesync_engine::Account,
) -> adw::NavigationPage {
    super::sidebar::exercise(account);
    let mut second = account.clone();
    second.id = drivesync_engine::AccountId("sidebar-work".into());
    second.display_name = Some("Work".into());
    second.email = Some("work@example.invalid".into());
    build_with_accounts(
        &Context::default(),
        navigation,
        vec![account.clone(), second],
    )
}
