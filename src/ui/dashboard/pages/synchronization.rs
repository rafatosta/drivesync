//! Existing sidebar selection feeds a replaceable presentation controller.
use crate::{
    application::Context,
    ui::dashboard::sync::{
        backend::EngineAdapter,
        components::SyncIssueDialog,
        controller::{DashboardController, SubscriptionTask},
        models::{AttentionAction, IssueView, Severity, StatusView},
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
    pub fn select(&self, context: &Context, account: Option<&Account>) {
        self.task.abort();
        for dialog in self.dialogs.borrow_mut().drain(..) {
            dialog.close();
        }
        self.controller.attention.connect_activated(|_| {});
        self.controller.reset();
        let (Some(account), Some(engine)) = (account, context.engine.borrow().clone()) else {
            return;
        };
        let controller = Rc::downgrade(&self.controller);
        let account = account.clone();
        self.controller.attention.connect_activated({
            let engine = engine.clone();
            let account = account.clone();
            let parent = self.widget.downgrade();
            let dialogs = self.dialogs.clone();
            let status = self.controller.status.clone();
            move |action| {
                let Some(parent) = parent.upgrade() else {
                    return;
                };
                match action {
                    AttentionAction::OpenConflictList => {
                        // Preserve the existing revision-bound real conflict flow until event 10.
                        dialogs.borrow_mut().push(super::conflicts::present(
                            &parent,
                            engine.clone(),
                            account.clone(),
                        ));
                    }
                    AttentionAction::OpenIssue(_) => {
                        let dialog = SyncIssueDialog::new();
                        dialog.window.set_transient_for(
                            parent.root().and_downcast::<gtk::Window>().as_ref(),
                        );
                        dialog.set_issue(IssueView {
                            severity: Severity::Warning,
                            title: gettextrs::gettext("Synchronization needs attention"),
                            description: status.description(),
                            guidance: None,
                        });
                        dialog.present();
                        dialogs.borrow_mut().push(dialog.window);
                    }
                }
            }
        });
        self.task.set(gtk::glib::spawn_future_local(async move {
            let result =
                crate::services::synchronization::ensure_started(&engine, &account.id).await;
            let Some(controller) = controller.upgrade() else {
                return;
            };
            match result {
                Ok(true) => controller.select(
                    account.id.0.clone(),
                    Box::new(EngineAdapter::new(engine, account.id)),
                ),
                Ok(false) => controller.status.set_state(
                    StatusView::Stopped,
                    &gettextrs::gettext("Configure folders to prepare synchronization."),
                ),
                Err(error) => controller.status.set_state(StatusView::Failed, &error),
            }
        }));
    }
}
