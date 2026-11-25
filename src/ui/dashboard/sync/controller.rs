//! Selection generation and cancellation belong to the dashboard, never to components.
use super::{backend::SyncBackend, components::*, models::*};
use adw::{gtk, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
/// Aborts a detached GLib future when its last UI owner is destroyed, even before realization.
#[derive(Default)]
pub struct SubscriptionTask(RefCell<Option<gtk::glib::JoinHandle<()>>>);
impl SubscriptionTask {
    pub fn abort(&self) {
        if let Some(task) = self.0.borrow_mut().take() {
            task.abort();
        }
    }
    pub fn set(&self, task: gtk::glib::JoinHandle<()>) {
        self.abort();
        *self.0.borrow_mut() = Some(task);
    }
}
impl Drop for SubscriptionTask {
    fn drop(&mut self) {
        if let Some(task) = self.0.get_mut().take() {
            task.abort();
        }
    }
}
#[derive(Clone)]
pub struct DashboardController {
    pub widget: gtk::Box,
    pub status: SyncStatus,
    pub activity: SyncActivity,
    pub attention: SyncAttention,
    dialogs: Rc<RefCell<Vec<gtk::Window>>>,
    generation: Rc<Cell<u64>>,
    scope: Rc<RefCell<Option<String>>>,
    task: Rc<SubscriptionTask>,
}
impl DashboardController {
    pub fn new() -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let status = SyncStatus::new();
        let activity = SyncActivity::new();
        let attention = SyncAttention::new();
        widget.append(&feedback_card(&status, &activity));
        let value = Self {
            widget,
            status,
            activity,
            attention,
            dialogs: Rc::default(),
            generation: Rc::default(),
            scope: Rc::default(),
            task: Rc::default(),
        };
        value.widget.connect_unrealize({
            let task = value.task.clone();
            let dialogs = value.dialogs.clone();
            let generation = value.generation.clone();
            move |_| {
                for dialog in dialogs.borrow_mut().drain(..) {
                    dialog.close();
                }
                generation.set(generation.get().wrapping_add(1));
                task.abort();
            }
        });
        value
    }
    pub fn reset(&self) {
        for dialog in self.dialogs.borrow_mut().drain(..) {
            dialog.close();
        }
        self.generation.set(self.generation.get().wrapping_add(1));
        self.task.abort();
        self.scope.borrow_mut().take();
        self.status.reset();
        self.activity.clear();
        self.attention.clear();
    }
    /// Presents a controlled fixture or reviewed conflict; intentions remain backend-independent.
    pub fn present_conflict(
        &self,
        view: ConflictView,
        callback: impl Fn(ConflictChoiceRequest) + 'static,
    ) -> ConflictResolver {
        let dialog = ConflictResolver::new();
        dialog.set_conflict(view);
        let generation = self.generation.get();
        let generations = self.generation.clone();
        dialog.connect_choice(move |request| {
            if generations.get() == generation {
                callback(request);
            }
        });
        dialog.connect_dismissed(|| {});
        dialog
            .window
            .set_transient_for(self.widget.root().and_downcast::<gtk::Window>().as_ref());
        dialog.window.present();
        self.dialogs.borrow_mut().push(dialog.window.clone());
        dialog
    }
    pub fn present_issue(&self, view: IssueView) -> SyncIssueDialog {
        let dialog = SyncIssueDialog::new();
        dialog.set_issue(view);
        dialog
            .window
            .set_transient_for(self.widget.root().and_downcast::<gtk::Window>().as_ref());
        dialog.present();
        self.dialogs.borrow_mut().push(dialog.window.clone());
        dialog
    }
    pub fn apply(&self, snapshot: Snapshot) -> bool {
        if self.scope.borrow().as_deref() != Some(snapshot.scope.as_str()) {
            return false;
        }
        self.status
            .set_state(snapshot.status, &snapshot.description);
        match snapshot.activity.filter(|a| a.scope == snapshot.scope) {
            Some(activity) => self.activity.set_activity(activity),
            None => self.activity.clear(),
        }
        self.attention.set_items(
            snapshot
                .attention
                .into_iter()
                .filter(|a| a.scope == snapshot.scope)
                .collect(),
        );
        true
    }
    pub fn select(&self, scope: String, mut backend: Box<dyn SyncBackend>) {
        self.reset();
        *self.scope.borrow_mut() = Some(scope);
        let generation = self.generation.get();
        // Weak ownership across awaits prevents the task retaining a destroyed dashboard.
        let widget = self.widget.downgrade();
        let status = self.status.clone();
        let activity = self.activity.clone();
        let attention = self.attention.clone();
        let scope = self.scope.clone();
        let generations = self.generation.clone();
        self.task.set(gtk::glib::spawn_future_local(async move {
            while let Some(snapshot) = backend.next().await {
                if generations.get() != generation || widget.upgrade().is_none() {
                    break;
                }
                if scope.borrow().as_deref() != Some(&snapshot.scope) {
                    continue;
                }
                status.set_state(snapshot.status, &snapshot.description);
                match snapshot.activity.filter(|a| a.scope == snapshot.scope) {
                    Some(value) => activity.set_activity(value),
                    None => activity.clear(),
                }
                attention.set_items(
                    snapshot
                        .attention
                        .into_iter()
                        .filter(|a| a.scope == snapshot.scope)
                        .collect(),
                );
            }
        }));
    }
}
