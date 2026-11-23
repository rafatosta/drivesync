//! Selection generation and cancellation belong to the dashboard, never to components.
use super::{backend::SyncBackend, components::*, models::*};
use adw::{gtk, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[derive(Clone)]
pub struct DashboardController {
    pub widget: gtk::Box,
    pub status: SyncStatus,
    pub activity: SyncActivity,
    pub attention: SyncAttention,
    generation: Rc<Cell<u64>>,
    scope: Rc<RefCell<Option<String>>>,
    task: Rc<RefCell<Option<gtk::glib::JoinHandle<()>>>>,
}
impl DashboardController {
    pub fn new() -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let status = SyncStatus::new();
        let activity = SyncActivity::new();
        let attention = SyncAttention::new();
        widget.append(&status.widget);
        widget.append(&activity.widget);
        widget.append(&attention.widget);
        let value = Self {
            widget,
            status,
            activity,
            attention,
            generation: Rc::default(),
            scope: Rc::default(),
            task: Rc::default(),
        };
        value.widget.connect_unrealize({
            let task = value.task.clone();
            let generation = value.generation.clone();
            move |_| {
                generation.set(generation.get().wrapping_add(1));
                if let Some(task) = task.borrow_mut().take() {
                    task.abort();
                }
            }
        });
        value
    }
    pub fn reset(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        self.scope.borrow_mut().take();
        self.status.reset();
        self.activity.clear();
        self.attention.clear();
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
        let controller = self.clone();
        // Weak ownership across awaits prevents the task retaining a destroyed dashboard.
        let widget = self.widget.downgrade();
        let status = self.status.clone();
        let activity = self.activity.clone();
        let attention = self.attention.clone();
        let scope = self.scope.clone();
        let generations = self.generation.clone();
        drop(controller);
        *self.task.borrow_mut() = Some(gtk::glib::spawn_future_local(async move {
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
