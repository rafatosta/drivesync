//! Existing sidebar selection feeds a replaceable presentation controller.
use crate::{
    application::Context,
    ui::dashboard::sync::{
        backend::EngineAdapter,
        components::SyncIssueDialog,
        controller::DashboardController,
        models::{AttentionAction, IssueView, StatusView},
    },
};
use adw::{gtk, prelude::*};
use drivesync_engine::Account;
use std::{cell::RefCell, rc::Rc};
#[derive(Clone)]
pub(super) struct SynchronizationView {
    pub widget: gtk::Box,
    controller: DashboardController,
    task: Rc<RefCell<Option<gtk::glib::JoinHandle<()>>>>,
}
impl SynchronizationView {
    pub fn new() -> Self {
        let controller = DashboardController::new();
        let task: Rc<RefCell<Option<gtk::glib::JoinHandle<()>>>> = Rc::default();
        controller.widget.connect_unrealize({
            let task = task.clone();
            move |_| {
                if let Some(task) = task.borrow_mut().take() {
                    task.abort();
                }
            }
        });
        Self {
            widget: controller.widget.clone(),
            controller,
            task,
        }
    }
    pub fn select(&self, context: &Context, account: Option<&Account>) {
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        self.controller.reset();
        let (Some(account), Some(engine)) = (account, context.engine.borrow().clone()) else {
            return;
        };
        let controller = self.controller.clone();
        let account = account.clone();
        self.controller.attention.connect_activated({
            let engine = engine.clone();
            let account = account.clone();
            let parent = self.widget.downgrade();
            move |action| {
                let Some(parent) = parent.upgrade() else {
                    return;
                };
                match action {
                    AttentionAction::OpenConflictList => {
                        // Preserve the existing revision-bound real conflict flow until event 10.
                        super::conflicts::present(&parent, engine.clone(), account.clone());
                    }
                    AttentionAction::OpenIssue(_) => {
                        let dialog = SyncIssueDialog::new();
                        dialog.window.set_transient_for(
                            parent.root().and_downcast::<gtk::Window>().as_ref(),
                        );
                        dialog.set_issue(IssueView {
                            title: gettextrs::gettext("Synchronization needs attention"),
                            description: controller_description(&parent),
                            guidance: None,
                        });
                        dialog.present();
                    }
                }
            }
        });
        *self.task.borrow_mut() = Some(gtk::glib::spawn_future_local(async move {
            match crate::services::synchronization::ensure_started(&engine, &account.id).await {
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
fn controller_description(parent: &gtk::Box) -> String {
    parent
        .first_child()
        .and_then(|status| status.last_child())
        .and_downcast::<gtk::Label>()
        .map(|l| l.text().to_string())
        .unwrap_or_default()
}
