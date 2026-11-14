//! Provider-independent contracts used by remote dispatch, scheduling and persistence.
use super::{
    RemoteDetectorScope, RemoteError, RemoteFailure, RemotePollRequest, RemotePollResult,
    RemoteResult, RemoteScan, RemoteScanUpdate,
};
use crate::{models::File, AccountId, Engine, ProviderId};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Persisted account identity selected by Accounts, without credentials or UI input.
#[derive(Clone)]
pub(crate) struct RemoteSessionContext {
    pub account_id: AccountId,
    pub provider: ProviderId,
}

/// Minimal provider implementation behind the shared RemoteDetector lifecycle.
/// The facade owns start/stop/status and scheduling; the specialist owns protocol,
/// traversal, cursor interpretation and one bounded, cancellable observation round.
pub(super) trait RemoteObserver: Send + Sync {
    /// Validates its own stored checkpoint without HTTP or credential access.
    fn validate_progress(&self) -> RemoteResult<()>;
    /// Returns only after each reported occurrence has durable acceptance.
    /// The facade supplies a validated finite page budget and a fresh cancellation token.
    fn poll_once(
        &self,
        request: RemotePollRequest,
        cancellation: &Cancellation,
        progress: &dyn Fn(u64, u64),
    ) -> RemoteResult<RemotePollResult>;
}

/// Opaque durable ports; provider implementations never access SQLite or delivery policy.
pub(super) trait RemoteBackend: Send + Sync {
    fn load(&self) -> RemoteResult<Option<Vec<u8>>>;
    /// Compare-and-swap progress after durable acceptance; stale writers must fail.
    fn save(
        &self,
        previous: Option<&[u8]>,
        value: &[u8],
        traversal: &RemoteScanUpdate,
    ) -> RemoteResult<()>;
    /// Reads only the indexed head directory; never materializes the traversal queue.
    fn next_scan(&self) -> RemoteResult<Option<RemoteScan>>;
    /// Accepts one complete occurrence; false denotes an identical durable replay.
    fn emit(&self, file: &File) -> RemoteResult<bool>;
}

pub(super) type ObserverFactory = fn(
    Engine,
    RemoteSessionContext,
    RemoteDetectorScope,
    Arc<dyn RemoteBackend>,
) -> RemoteResult<Box<dyn RemoteObserver>>;

/// Shared cancellation capability; no provider-specific session data is retained.
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
