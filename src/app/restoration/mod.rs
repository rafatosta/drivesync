//! Startup account loading; errors never masquerade as an empty account list.
mod view;
use crate::app::Context;
use adw::{gtk, prelude::*};
use drivesync_engine::{Engine, EngineConfig, ListAccountsRequest};
use std::{cell::Cell, rc::Rc};
/// Opens the engine once, then loads local accounts through the public API.
pub(crate) fn build(
    config: EngineConfig,
    context: &Context,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
) -> adw::NavigationPage {
    let view = view::build();
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.pack_end(&crate::app::main_menu::button());
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&view.panel));
    let page = adw::NavigationPage::new(&toolbar, "DriveSync");
    let weak_nav = navigation.downgrade();
    let weak_toasts = toasts.downgrade();
    let context = context.clone();
    let status = view.status.downgrade();
    let spinner = view.spinner.downgrade();
    let retry = view.retry.downgrade();
    let busy = Rc::new(Cell::new(false));
    let reload = Rc::new(move || {
        let (Some(status), Some(spinner), Some(retry)) =
            (status.upgrade(), spinner.upgrade(), retry.upgrade())
        else {
            return;
        };
        if busy.replace(true) {
            return;
        }
        spinner.start();
        spinner.set_visible(true);
        retry.set_visible(false);
        status.set_label(&gettextrs::gettext("Checking saved accounts…"));
        let config = config.clone();
        let context = context.clone();
        let nav = weak_nav.clone();
        let toasts = weak_toasts.clone();
        let status = status.clone();
        let spinner = spinner.clone();
        let retry = retry.clone();
        let busy = busy.clone();
        gtk::glib::spawn_future_local(async move {
            let current = context.engine.borrow().clone();
            let opened = match current {
                Some(engine) => Ok(engine),
                None => Engine::open(config).await,
            };
            let result = match opened {
                Ok(engine) => {
                    *context.engine.borrow_mut() = Some(engine.clone());
                    engine
                        .list_accounts(ListAccountsRequest { provider: None })
                        .await
                }
                Err(error) => Err(error),
            };
            busy.set(false);
            spinner.stop();
            spinner.set_visible(false);
            let (Some(nav), Some(toasts)) = (nav.upgrade(), toasts.upgrade()) else {
                return;
            };
            match result {
                Ok(result) => {
                    crate::features::accounts::connection::install_action(&context, &nav, &toasts);
                    crate::app::navigation::show_accounts(
                        &context,
                        result.accounts,
                        &nav,
                        &toasts,
                        None,
                        false,
                    );
                }
                Err(error) => {
                    status.set_label(&crate::core::messages::engine_error(&error));
                    retry.set_visible(true);
                }
            }
        });
    });
    let refresh = reload.clone();
    view.retry.connect_clicked(move |_| refresh());
    reload();
    page
}
