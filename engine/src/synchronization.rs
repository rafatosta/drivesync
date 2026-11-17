//! Read-only, account-scoped synchronization observations, without storage access.
use crate::AccountId;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

/// Current work, not a claim about queued work or global completion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SynchronizationActivity {
    #[default]
    Idle,
    Starting,
    Scanning,
    Analyzing,
    Reconciling,
    Uploading,
    Downloading,
}
/// Latest observation. Idle never means that all files are synchronized.
/// The current pipeline cannot certify overall completion and never emits it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SynchronizationStatus {
    pub activity: SynchronizationActivity,
    /// True only with an authoritative whole-account completion proof.
    /// The current pipeline always leaves this false.
    pub synchronized: bool,
    pub transferred_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
}
/// Coalescing subscription: slow readers receive the latest state, without a backlog.
/// Dropping it releases the subscription and does not stop synchronization.
pub struct SynchronizationSubscription(watch::Receiver<SynchronizationStatus>);
impl SynchronizationSubscription {
    /// Cached initial/latest value; performs no I/O.
    pub fn snapshot(&self) -> SynchronizationStatus {
        self.0.borrow().clone()
    }
    /// Waits for a change, including changes published before this call.
    pub async fn changed(&mut self) -> bool {
        self.0.changed().await.is_ok()
    }
}
#[derive(Default)]
pub(crate) struct StatusRegistry(pub Mutex<std::collections::HashMap<AccountId, Arc<Publisher>>>);
#[derive(Default)]
struct State {
    base: SynchronizationActivity,
    active: Option<SynchronizationActivity>,
    bytes: Option<u64>,
    total: Option<u64>,
}
pub(crate) struct Publisher {
    state: Mutex<State>,
    sender: watch::Sender<SynchronizationStatus>,
}
impl Publisher {
    pub(crate) fn new() -> Self {
        let (sender, _) = watch::channel(SynchronizationStatus::default());
        Self {
            state: Mutex::new(State::default()),
            sender,
        }
    }
    pub(crate) fn subscribe(&self) -> SynchronizationSubscription {
        SynchronizationSubscription(self.sender.subscribe())
    }
    fn update(&self, change: impl FnOnce(&mut State)) {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        change(&mut s);
        self.sender.send_if_modified(|value| {
            let next = SynchronizationStatus {
                activity: s.active.unwrap_or(s.base),
                synchronized: false,
                transferred_bytes: s.bytes,
                total_bytes: s.total,
            };
            if *value == next {
                false
            } else {
                *value = next;
                true
            }
        });
    }
    pub(crate) fn phase(&self, activity: SynchronizationActivity) {
        self.update(|s| {
            s.base = activity;
            s.active = None;
            s.bytes = None;
            s.total = None;
        });
    }
    pub(crate) fn activity(&self, activity: Option<SynchronizationActivity>) {
        self.update(|s| {
            s.active = activity;
            s.bytes = None;
            s.total = None;
        });
    }
    pub(crate) fn progress(&self, bytes: Option<u64>, total: Option<u64>) {
        self.update(|s| {
            if s.active.is_some() {
                s.bytes = bytes;
                s.total = total;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn progress_is_current_and_terminal_events_restore_phase_without_success() {
        let p = Publisher::new();
        let mut subscription = SynchronizationSubscription(p.sender.subscribe());
        p.phase(SynchronizationActivity::Analyzing);
        assert!(subscription.changed().await);
        assert_eq!(
            subscription.snapshot().activity,
            SynchronizationActivity::Analyzing
        );
        p.activity(Some(SynchronizationActivity::Uploading));
        p.progress(Some(12), Some(24));
        assert!(subscription.changed().await);
        assert_eq!(subscription.snapshot().transferred_bytes, Some(12));
        p.activity(None);
        assert!(subscription.changed().await);
        assert_eq!(
            subscription.snapshot().activity,
            SynchronizationActivity::Analyzing
        );
        assert_eq!(subscription.snapshot().transferred_bytes, None);
        p.phase(SynchronizationActivity::Idle);
        assert!(subscription.changed().await);
        assert!(!subscription.snapshot().synchronized);
        p.progress(Some(24), Some(24));
        assert!(!subscription.0.has_changed().unwrap());
        assert_eq!(subscription.snapshot().transferred_bytes, None);
    }
    #[tokio::test]
    async fn subscriptions_coalesce_and_accounts_are_isolated() {
        let registry = StatusRegistry::default();
        let a = Arc::new(Publisher::new());
        let b = Arc::new(Publisher::new());
        registry
            .0
            .lock()
            .unwrap()
            .insert(AccountId("a".into()), a.clone());
        registry
            .0
            .lock()
            .unwrap()
            .insert(AccountId("b".into()), b.clone());
        let mut reader = SynchronizationSubscription(a.sender.subscribe());
        let other = SynchronizationSubscription(b.sender.subscribe());
        a.phase(SynchronizationActivity::Scanning);
        a.phase(SynchronizationActivity::Analyzing);
        assert!(reader.changed().await);
        assert_eq!(
            reader.snapshot().activity,
            SynchronizationActivity::Analyzing
        );
        assert!(!other.0.has_changed().unwrap());
        a.phase(SynchronizationActivity::Idle);
        assert_eq!(
            a.sender.subscribe().borrow().activity,
            SynchronizationActivity::Idle
        );
        assert!(!reader.snapshot().synchronized);
    }
}
