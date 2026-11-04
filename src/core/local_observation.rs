//! Bounded worker transport for local observation; no GTK objects cross threads.
use drivesync_engine::{LocalBatch, LocalScope, LocalWatchOptions, LocalWatchState, LocalWatcher};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TrySendError},
    Arc, Mutex, Weak,
};
use std::thread::JoinHandle;
use std::time::Duration;

static SESSIONS: Mutex<Vec<Weak<AtomicBool>>> = Mutex::new(Vec::new());

/// Account-local updates. Missing configuration differs from an observation failure.
pub(crate) enum Update {
    Batch(LocalBatch),
    Unconfigured,
    Failed,
}

/// Owns cancellation and a bounded receiver. Drop cancels without blocking GTK.
pub(crate) struct Observation {
    cancel: Arc<AtomicBool>,
    pub receiver: Receiver<Update>,
    worker: Option<JoinHandle<()>>,
}
impl Observation {
    /// Resolves local configuration and observes on a worker, without network I/O.
    pub fn start(
        resolve: impl FnOnce() -> Result<Option<LocalScope>, ()> + Send + 'static,
    ) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let (tx, receiver) = mpsc::sync_channel(4);
        if let Ok(mut sessions) = SESSIONS.lock() {
            sessions.retain(|s| s.strong_count() > 0);
            sessions.push(Arc::downgrade(&cancel));
        }
        let worker = std::thread::spawn(move || {
            let scope = match resolve() {
                Ok(Some(scope)) => scope,
                Ok(None) => {
                    deliver(&tx, &flag, Update::Unconfigured);
                    return;
                }
                Err(()) => {
                    deliver(&tx, &flag, Update::Failed);
                    return;
                }
            };
            if flag.load(Ordering::Acquire) {
                return;
            }
            let mut watcher = match LocalWatcher::start(scope, LocalWatchOptions::default()) {
                Ok(watcher) => watcher,
                Err(_) => {
                    deliver(&tx, &flag, Update::Failed);
                    return;
                }
            };
            let mut previous = None;
            while !flag.load(Ordering::Acquire) {
                let batch = watcher.poll();
                let state = batch.state;
                if (!batch.notices.is_empty() || previous != Some(state))
                    && !deliver(&tx, &flag, Update::Batch(batch))
                {
                    break;
                }
                previous = Some(state);
                if state == LocalWatchState::Stopped {
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        });
        Self {
            cancel,
            receiver,
            worker: Some(worker),
        }
    }
}
impl Drop for Observation {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            if worker.is_finished() {
                let _ = worker.join();
            }
            // An active worker observes cancellation; never block the GUI on I/O.
        }
    }
}

/// Application shutdown cancels every live session, including hidden account views.
pub(crate) fn stop_all() {
    if let Ok(mut sessions) = SESSIONS.lock() {
        for session in sessions.drain(..).filter_map(|s| s.upgrade()) {
            session.store(true, Ordering::Release);
        }
    }
}

/// Backpressure preserves the pending batch and lets inotify report any kernel loss.
fn deliver(tx: &SyncSender<Update>, cancel: &AtomicBool, mut value: Update) -> bool {
    loop {
        if cancel.load(Ordering::Acquire) {
            return false;
        }
        match tx.try_send(value) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(pending)) => {
                value = pending;
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drivesync_engine::LocalActivity;
    #[test]
    fn worker_delivers_real_activity_and_cancels_while_backpressured() {
        let path = std::env::temp_dir().join(format!("drivesync-worker-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        let scope = LocalScope::new("test", "account", &path).unwrap();
        let session = Observation::start(move || Ok(Some(scope)));
        assert!(matches!(
            session
                .receiver
                .recv_timeout(Duration::from_secs(3))
                .unwrap(),
            Update::Batch(_)
        ));
        std::fs::write(path.join("file"), b"unchanged").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            let update = session
                .receiver
                .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                .unwrap();
            if let Update::Batch(batch) = update {
                if batch
                    .notices
                    .iter()
                    .any(|n| n.activity == LocalActivity::Written)
                {
                    break;
                }
            }
        }
        // Fill the transport without consuming, then cancel it.
        for n in 0..8 {
            std::fs::write(path.join(format!("file-{n}")), b"x").unwrap();
            std::thread::sleep(Duration::from_millis(35));
        }
        session.cancel.store(true, Ordering::Release);
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !session.worker.as_ref().unwrap().is_finished() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(std::fs::read(path.join("file")).unwrap(), b"unchanged");
        drop(session);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn missing_configuration_and_failed_resolution_are_distinct() {
        let missing = Observation::start(|| Ok(None));
        assert!(matches!(
            missing
                .receiver
                .recv_timeout(Duration::from_secs(2))
                .unwrap(),
            Update::Unconfigured
        ));
        let failed = Observation::start(|| Err(()));
        assert!(matches!(
            failed
                .receiver
                .recv_timeout(Duration::from_secs(2))
                .unwrap(),
            Update::Failed
        ));
    }
}
