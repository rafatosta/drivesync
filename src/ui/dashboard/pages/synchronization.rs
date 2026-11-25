//! Existing sidebar selection feeds a replaceable presentation controller.
use crate::{
    application::Context,
    ui::dashboard::sync::{backend::EngineAdapter, controller::DashboardController},
};
use adw::{gtk, prelude::*};
use drivesync_engine::Account;
use std::{cell::RefCell, rc::Rc};
#[derive(Clone)]
pub(super) struct SynchronizationView {
    pub widget: gtk::Box,
    controller: Rc<DashboardController>,
    dialogs: Rc<RefCell<Vec<gtk::Window>>>,
}
impl SynchronizationView {
    pub fn new() -> Self {
        let controller = Rc::new(DashboardController::new());
        controller.widget.set_visible(false);
        let dialogs: Rc<RefCell<Vec<gtk::Window>>> = Rc::default();
        controller.widget.connect_unrealize({
            let dialogs = dialogs.clone();
            move |_| {
                for dialog in dialogs.borrow_mut().drain(..) {
                    dialog.close();
                }
            }
        });
        Self {
            widget: controller.widget.clone(),
            controller,
            dialogs,
        }
    }
    pub fn select(
        &self,
        context: &Context,
        navigation: &adw::NavigationView,
        account: Option<&Account>,
    ) {
        for dialog in self.dialogs.borrow_mut().drain(..) {
            dialog.close();
        }
        self.controller.status.connect_details(|| {});
        let engine = context.engine.borrow().clone();
        self.widget
            .set_visible(account.is_some() && engine.is_some());
        let (Some(account), Some(engine)) = (account, engine) else {
            self.controller.reset();
            return;
        };
        let controller = Rc::downgrade(&self.controller);
        let account = account.clone();
        self.controller.status.connect_details({
            let engine = engine.clone();
            let account = account.clone();
            let navigation = navigation.downgrade();
            let context = context.clone();
            let dialogs = self.dialogs.clone();
            let controller = controller.clone();
            move || {
                let (Some(navigation), Some(controller)) =
                    (navigation.upgrade(), controller.upgrade())
                else {
                    return;
                };
                super::diagnostic::show(
                    &navigation,
                    &context,
                    &engine,
                    &account,
                    &controller,
                    &dialogs,
                );
            }
        });
        self.controller.select(
            account.id.0.clone(),
            Box::new(EngineAdapter::new(engine.clone(), account.id.clone())),
        );
        self.controller.watch_history(engine, account.id);
    }
}
