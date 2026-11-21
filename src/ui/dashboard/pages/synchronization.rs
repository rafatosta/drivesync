//! Live account status with explicit subscription cancellation on selection changes and teardown.
use crate::application::Context;
use adw::{gtk, prelude::*};
use drivesync_engine::{Account, SynchronizationStatus};
use gettextrs::gettext;
use std::{cell::RefCell, rc::Rc};
#[derive(Clone)]
pub(super) struct SynchronizationView {
    pub widget: gtk::Box,
    label: gtk::Label,
    progress: gtk::ProgressBar,
    task: Rc<RefCell<Option<gtk::glib::JoinHandle<()>>>>,
}
impl SynchronizationView {
    pub fn new() -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let label = gtk::Label::builder()
            .wrap(true)
            .justify(gtk::Justification::Center)
            .build();
        label.set_widget_name("dashboard-synchronization-status");
        let progress = gtk::ProgressBar::new();
        progress.set_visible(false);
        progress.set_show_text(true);
        progress.set_widget_name("dashboard-synchronization-progress");
        widget.append(&label);
        widget.append(&progress);
        let task: Rc<RefCell<Option<gtk::glib::JoinHandle<()>>>> = Rc::default();
        label.connect_unrealize({
            let task = task.clone();
            move |_| {
                if let Some(task) = task.borrow_mut().take() {
                    task.abort();
                }
            }
        });
        Self {
            widget,
            label,
            progress,
            task,
        }
    }
    pub fn select(&self, context: &Context, account: Option<&Account>) {
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        self.progress.set_visible(false);
        self.label.set_label("");
        let (Some(account), Some(engine)) = (account, context.engine.borrow().clone()) else {
            return;
        };
        let id = account.id.clone();
        let label = self.label.downgrade();
        let progress = self.progress.downgrade();
        self.label.set_label(&gettext("Preparing synchronization…"));
        *self.task.borrow_mut() = Some(gtk::glib::spawn_future_local(async move {
            match crate::services::synchronization::ensure_started(&engine, &id).await {
                Ok(true) => {}
                Ok(false) => {
                    if let Some(label) = label.upgrade() {
                        label.set_label(&gettext("Configure folders to prepare synchronization."));
                    }
                    return;
                }
                Err(message) => {
                    if let Some(label) = label.upgrade() {
                        label.set_label(&message);
                    }
                    return;
                }
            }
            let mut subscription = match engine.subscribe_synchronization(id).await {
                Ok(subscription) => subscription,
                Err(error) => {
                    if let Some(label) = label.upgrade() {
                        label.set_label(&crate::core::messages::engine_error(&error));
                    }
                    return;
                }
            };
            loop {
                let (Some(label), Some(progress)) = (label.upgrade(), progress.upgrade()) else {
                    break;
                };
                render(&label, &progress, &subscription.current());
                // Keep only weak widget references across an unbounded subscription wait.
                drop(label);
                drop(progress);
                if subscription.changed().await.is_none() {
                    break;
                }
            }
        }));
    }
}
fn render(label: &gtk::Label, progress: &gtk::ProgressBar, status: &SynchronizationStatus) {
    label.set_label(&crate::services::synchronization::description(status));
    let measured = status
        .progress
        .as_ref()
        .filter(|progress| progress.total > 0);
    progress.set_visible(measured.is_some());
    if let Some(measured) = measured {
        progress.set_fraction((measured.completed as f64 / measured.total as f64).clamp(0.0, 1.0));
        progress.set_text(Some(
            &gettext("{completed} of {total} operations")
                .replace("{completed}", &measured.completed.to_string())
                .replace("{total}", &measured.total.to_string()),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drivesync_engine::{
        AccountId, SynchronizationActivity, SynchronizationIssue, SynchronizationIssueKind,
        SynchronizationProgress, SynchronizationState, Timestamp,
    };
    #[test]
    #[ignore = "requires an isolated GTK display and XDG profile"]
    fn status_widget_presents_only_measured_progress_and_releases_selection() {
        let profile = std::env::var("XDG_DATA_HOME").expect("isolated profile required");
        assert!(profile.starts_with("/tmp/drivesync-ui-test-"));
        adw::init().unwrap();
        let view = SynchronizationView::new();
        let mut status = SynchronizationStatus {
            account_id: AccountId("synthetic".into()),
            state: SynchronizationState::Waiting,
            activity: SynchronizationActivity::Observing,
            current_item: None,
            progress: None,
            issue: None,
            updated_at: Timestamp { unix_millis: 0 },
        };
        render(&view.label, &view.progress, &status);
        assert_eq!(view.label.label(), gettext("Waiting for observations."));
        assert!(!view.progress.is_visible());
        status.state = SynchronizationState::Running;
        status.activity = SynchronizationActivity::Uploading;
        status.progress = Some(SynchronizationProgress {
            completed: 1,
            total: 3,
        });
        render(&view.label, &view.progress, &status);
        assert!(view.progress.is_visible());
        assert!((view.progress.fraction() - 1.0 / 3.0).abs() < 0.0001);
        status.state = SynchronizationState::Waiting;
        status.issue = Some(SynchronizationIssue {
            kind: SynchronizationIssueKind::RecoveryRequired,
            retryable: false,
        });
        status.progress = None;
        render(&view.label, &view.progress, &status);
        assert!(!view.progress.is_visible());
        assert_eq!(
            view.label.label(),
            gettext("An operation needs verification before this item can continue.")
        );
        view.select(&Context::default(), None);
        assert!(view.label.label().is_empty());
        assert!(view.task.borrow().is_none());
    }
}
