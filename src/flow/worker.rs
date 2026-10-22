//! Execute blocking engine work without blocking the GTK event loop.
//!
//! Completion callbacks are delivered on the GTK main context. A disconnected
//! worker is reported as an internal engine error, never as success.
use adw::gtk;
use drivesync_engine::{AuthError, EngineError};
use std::{sync::mpsc, time::Duration};

/// Runs a fallible blocking job on a worker thread.
///
/// The `job` closure executes off the GTK main thread; `done` executes once
/// on the GTK main context after the channel yields a result. The function
/// returns immediately and does not propagate the job result synchronously.
/// Disk, network and vault work must remain inside `job`.
///
/// Channel disconnection is mapped to an internal engine error.
pub fn work<T: Send + 'static>(
    job: impl FnOnce() -> Result<T, EngineError> + Send + 'static,
    done: impl FnOnce(Result<T, EngineError>) + 'static,
) {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(job());
    });
    let mut done = Some(done);
    gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
        let result = match receive.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return gtk::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => Err(EngineError::Authentication(
                AuthError::Internal,
            )),
        };
        if let Some(done) = done.take() {
            done(result);
        }
        gtk::glib::ControlFlow::Break
    });
}
