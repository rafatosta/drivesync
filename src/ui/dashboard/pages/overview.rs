//! Account dashboard for browsing saved local and remote folder choices.
use super::sidebar::AccountSidebar;
use crate::application::Context;
use adw::{gtk, prelude::*};
use drivesync_engine::ListAccountsRequest;
use gettextrs::gettext;
use std::{cell::Cell, rc::Rc};

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

    let content = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    split
        .bind_property("collapsed", &header, "show-back-button")
        .sync_create()
        .build();
    let title = adw::WindowTitle::new("DriveSync", "");
    title.set_widget_name("dashboard-account-title");
    header.set_title_widget(Some(&title));
    let toggle = gtk::ToggleButton::builder()
        .icon_name("adw-sidebar-symbolic")
        .tooltip_text(gettext("Toggle sidebar"))
        .css_classes(["flat"])
        .build();
    toggle.set_widget_name("dashboard-sidebar-toggle");
    split
        .bind_property("show-sidebar", &toggle, "active")
        .bidirectional()
        .sync_create()
        .build();
    header.pack_start(&toggle);
    content.add_top_bar(&header);
    let synchronization = super::synchronization::SynchronizationView::new();
    let dashboard = adw::Clamp::builder()
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(16)
        .margin_end(16)
        .valign(gtk::Align::Start)
        .child(&synchronization.widget)
        .build();
    dashboard.set_widget_name("dashboard-content");
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&dashboard)
        .build();
    content.set_content(Some(&scroll));
    split.set_content(Some(&adw::NavigationPage::new(&content, "DriveSync")));
    sidebar.on_selected({
        let context = context.clone();
        let synchronization = synchronization.clone();
        let navigation = navigation.downgrade();
        let title = title.downgrade();
        move |account| {
            if let Some(navigation) = navigation.upgrade() {
                synchronization.select(&context, &navigation, account.as_ref());
            }
            if let Some(title) = title.upgrade() {
                title.set_title(
                    account
                        .as_ref()
                        .and_then(|a| a.display_name.as_deref().or(a.email.as_deref()))
                        .unwrap_or("DriveSync"),
                );
                title.set_subtitle(
                    account
                        .as_ref()
                        .and_then(|a| a.email.as_deref())
                        .unwrap_or(""),
                );
            }
        }
    });
    sidebar.set_accounts(accounts);
    sidebar.list.connect_row_activated({
        let split = split.downgrade();
        move |_, _| {
            if let Some(split) = split.upgrade() {
                if split.is_collapsed() {
                    split.set_show_sidebar(false);
                }
            }
        }
    });
    let generation = Rc::new(Cell::new(0_u64));
    let refresh: Rc<dyn Fn()> = Rc::new({
        let context = context.clone();
        let sidebar = sidebar.downgrade();
        let feedback = feedback.downgrade();
        move || {
            let (Some(sidebar), Some(engine)) =
                (sidebar.upgrade(), context.engine.borrow().clone())
            else {
                return;
            };
            let request = generation.get().wrapping_add(1);
            generation.set(request);
            let generation = generation.clone();
            let feedback = feedback.clone();
            gtk::glib::spawn_future_local(async move {
                let result = engine
                    .list_accounts(ListAccountsRequest { provider: None })
                    .await;
                if generation.get() != request {
                    return;
                }
                match result {
                    Ok(result) => {
                        sidebar.set_accounts(result.accounts);
                        if let Some(label) = feedback.upgrade() {
                            label.set_label("");
                        }
                    }
                    Err(error) => {
                        if let Some(label) = feedback.upgrade() {
                            label.set_label(&crate::core::messages::engine_error(&error));
                        }
                    }
                }
            });
        }
    });
    sidebar.list.connect_map({
        let refresh = refresh.clone();
        move |_| refresh()
    });
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
    adw::NavigationPage::new(&bin, "DriveSync")
}

#[cfg(test)]
pub(crate) fn fixture(
    navigation: &adw::NavigationView,
    account: &drivesync_engine::Account,
) -> adw::NavigationPage {
    super::sidebar::exercise(account);
    let mut work = account.clone();
    work.id = drivesync_engine::AccountId("work-fixture".into());
    work.display_name = Some("Work".into());
    work.email = Some("work@example.invalid".into());
    build_with_accounts(&Context::default(), navigation, vec![account.clone(), work])
}
