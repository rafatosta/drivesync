//! Periodic remote observation; scheduling is independent of external event delivery.
mod models;
pub use models::*;
pub(crate) mod google;
mod runtime;
mod timestamp;
pub use runtime::RemoteDetector;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
#[derive(Clone, Default)]
pub(crate) struct Cancellation {
    stopped: Arc<AtomicBool>,
    notify: Arc<tokio::sync::Notify>,
}
impl Cancellation {
    pub fn cancel(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.notify.notify_one();
    }
    pub fn check(&self) -> RemoteResult<()> {
        if self.stopped.load(Ordering::SeqCst) {
            Err(RemoteFailure::new(RemoteError::Cancelled))
        } else {
            Ok(())
        }
    }
    pub async fn cancelled(&self) {
        loop {
            let notified = self.notify.notified();
            if self.stopped.load(Ordering::SeqCst) {
                return;
            }
            notified.await;
        }
    }
}
pub(crate) fn now() -> crate::Timestamp {
    crate::Timestamp {
        unix_millis: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64,
    }
}
