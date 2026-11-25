//! Existing sidebar selection feeds a replaceable presentation controller.
use crate::{
    application::Context,
    ui::dashboard::sync::{
        backend::EngineAdapter,
        controller::{DashboardController, SubscriptionTask},
        models::StatusView,
    },
};
use adw::{gtk, prelude::*};
use drivesync_engine::Account;
use std::{cell::RefCell, rc::Rc};
#[derive(Clone)]
pub(super) struct SynchronizationView {
    pub widget: gtk::Box,
    controller: Rc<DashboardController>,
    dialogs: Rc<RefCell<Vec<gtk::Window>>>,
    task: Rc<SubscriptionTask>,
}
impl SynchronizationView {
    pub fn new() -> Self {
        let controller = Rc::new(DashboardController::new());
        controller.widget.set_visible(false);
        let task: Rc<SubscriptionTask> = Rc::default();
        let dialogs: Rc<RefCell<Vec<gtk::Window>>> = Rc::default();
        controller.widget.connect_unrealize({
            let dialogs = dialogs.clone();
            let task = task.clone();
            move |_| {
                for dialog in dialogs.borrow_mut().drain(..) {
                    dialog.close();
                }
                task.abort();
            }
        });
        Self {
            widget: controller.widget.clone(),
            controller,
            dialogs,
            task,
        }
    }
    pub fn select(
        &self,
        context: &Context,
        navigation: &adw::NavigationView,
        account: Option<&Account>,
    ) {
        self.task.abort();
        for dialog in self.dialogs.borrow_mut().drain(..) {
            dialog.close();
        }
        self.controller.status.connect_details(|| {});
        self.controller.reset();
        let engine = context.engine.borrow().clone();
        self.widget
            .set_visible(account.is_some() && engine.is_some());
        let (Some(account), Some(engine)) = (account, engine) else {
            return;
        };
        let controller = Rc::downgrade(&self.controller);
        let account = account.clone();
        let local_folder = Rc::new(RefCell::new(None));
        self.controller.status.connect_details({
            let engine = engine.clone();
            let account = account.clone();
            let navigation = navigation.downgrade();
            let context = context.clone();
            let dialogs = self.dialogs.clone();
            let controller = controller.clone();
            let local_folder = local_folder.clone();
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
                    local_folder.borrow().as_deref(),
                    &dialogs,
                );
            }
        });
        self.task.set(gtk::glib::spawn_future_local(async move {
            let result =
                crate::services::synchronization::ensure_started(&engine, &account.id).await;
            let Some(controller) = controller.upgrade() else {
                return;
            };
            match result {
                Ok(true) => {
                    controller.select(
                        account.id.0.clone(),
                        Box::new(EngineAdapter::new(engine.clone(), account.id.clone())),
                    );
                    controller.watch_history(engine, account.id);
                }
                Ok(false) => controller.status.set_state(
                    StatusView::Stopped,
                    &gettextrs::gettext("Configure folders to prepare synchronization."),
                ),
                Err(error) => {
                    *local_folder.borrow_mut() = error.local_folder;
                    controller
                        .status
                        .set_state(StatusView::Failed, &error.message);
                }
            }
        }));
    }
}
