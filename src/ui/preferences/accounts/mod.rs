//! Account administration reachable only from Preferences.
mod account;
mod components;
mod controller;

use crate::application::Context;
use adw::{gtk, prelude::*};
use drivesync_engine::{Account, AccountId};
use std::rc::Rc;
/// Builds one details page per account and selects the newly connected identity.
pub(crate) fn build(
    context: &Context,
    accounts: Vec<Account>,
    nav: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    selected: Option<&AccountId>,
    edit: bool,
) -> adw::NavigationPage {
    let split = adw::NavigationSplitView::builder()
        .min_sidebar_width(250.0)
        .max_sidebar_width(340.0)
        .build();
    split.set_widget_name("dashboard-split");
    let sidebar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("DriveSync", "")));
    header.pack_start(&crate::ui::components::main_menu::add_account_button());
    header.pack_end(&crate::ui::components::main_menu::button());
    sidebar.add_top_bar(&header);
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .activate_on_single_click(true)
        .css_classes(["navigation-sidebar"])
        .build();
    list.set_widget_name("account-list");
    let side = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let title = gtk::Label::builder()
        .label(gettextrs::gettext("Accounts"))
        .xalign(0.0)
        .margin_start(18)
        .margin_top(12)
        .css_classes(["heading"])
        .build();
    side.append(&title);
    let scroller = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&list)
        .build();
    side.append(&scroller);
    let preferences = gtk::Button::builder()
        .label(gettextrs::gettext("Preferences"))
        .action_name("app.preferences")
        .margin_start(12)
        .margin_end(12)
        .margin_bottom(12)
        .build();
    side.append(&preferences);
    sidebar.set_content(Some(&side));
    split.set_sidebar(Some(&adw::NavigationPage::new(
        &sidebar,
        &gettextrs::gettext("Accounts"),
    )));
    let stack = gtk::Stack::new();
    let mut initial = 0;
    for (index, account) in accounts.iter().enumerate() {
        let name = account
            .display_name
            .as_deref()
            .or(account.email.as_deref())
            .unwrap_or("Google Account");
        let row = adw::ActionRow::builder()
            .title(name)
            .subtitle(account.email.as_deref().unwrap_or(""))
            .use_markup(false)
            .activatable(true)
            .build();
        row.set_title_lines(2);
        list.append(&row);
        let chosen = selected == Some(&account.id);
        if chosen {
            initial = index;
        }
        let weak_row = row.downgrade();
        let updated = Rc::new(move |account: Account| {
            if let Some(row) = weak_row.upgrade() {
                row.set_title(
                    account
                        .display_name
                        .as_deref()
                        .or(account.email.as_deref())
                        .unwrap_or("Google Account"),
                );
            }
        });
        stack.add_named(
            &account::build(account, context, updated, nav, toasts, chosen && edit),
            Some(&index.to_string()),
        );
    }
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(
        &gettextrs::gettext("Account"),
        "",
    )));
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&stack));
    split.set_content(Some(&adw::NavigationPage::new(
        &toolbar,
        &gettextrs::gettext("Account"),
    )));
    let weak_stack = stack.downgrade();
    let weak_split = split.downgrade();
    list.connect_row_selected(move |_, row| {
        if let (Some(row), Some(stack), Some(split)) =
            (row, weak_stack.upgrade(), weak_split.upgrade())
        {
            stack.set_visible_child_name(&row.index().to_string());
            split.set_show_content(true);
        }
    });
    list.select_row(list.row_at_index(initial as i32).as_ref());
    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        660.0,
        adw::LengthUnit::Sp,
    ));
    breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
    let bin = adw::BreakpointBin::new();
    bin.set_width_request(320);
    bin.set_height_request(320);
    bin.set_child(Some(&split));
    bin.add_breakpoint(breakpoint);
    let page = adw::NavigationPage::new(&bin, "DriveSync");
    page.set_widget_name("accounts-dashboard");
    page
}
