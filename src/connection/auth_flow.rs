//! Google OAuth coordination for the connection window.
//!
//! Runs the blocking authorization in a worker thread while using the GTK
//! main context to open the browser through GIO, update progress and
//! navigate only after the engine reports a fully persisted account.
use super::view::ConnectionView;
use adw::{gtk, prelude::*};
use drivesync_engine::{AccountSummary, AuthError, Cancellation, DriveSyncEngine, EngineError};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

/// Messages between the OAuth worker and the GTK main context.
///
/// The browser launch acknowledgement is sent back to the worker so it can
/// fail promptly instead of waiting indefinitely for a callback.
enum AuthEvent {
    OpenBrowser(String, mpsc::Sender<bool>),
    Finished(Result<AccountSummary, EngineError>),
}

/// Connects the connection window's browser authorization and cancellation actions.
///
/// Cancellation is shared with the application lifecycle. The browser is
/// launched from the GTK context using GIO/OpenURI; the worker handles PKCE,
/// token exchange, identity validation and secure persistence through the
/// engine facade. Errors are sanitized by the engine before display.
pub(super) fn bind(
    engine: Arc<DriveSyncEngine>,
    navigation: &adw::NavigationView,
    toasts: &adw::ToastOverlay,
    cancellation: Rc<RefCell<Option<Cancellation>>>,
    view: &ConnectionView,
) {
    let connect = view.connect.clone();
    let window = view.window.downgrade();
    let copy = view.copy.clone();
    let url_slot = Rc::new(RefCell::new(None::<String>));
    let copy_url = url_slot.clone();
    copy.connect_clicked(move |button| {
        if let Some(url) = copy_url.borrow().as_ref() {
            button.clipboard().set_text(url);
        }
    });
    let close_cancel = cancellation.clone();
    let close_url = url_slot.clone();
    view.window.connect_close_request(move |_| {
        if let Some(cancel) = close_cancel.borrow().as_ref() {
            cancel.cancel();
        }
        close_url.borrow_mut().take();
        gtk::glib::Propagation::Proceed
    });
    let cancel_button = view.cancel_button.clone();
    let spinner = view.spinner.clone();
    let feedback = view.feedback.clone();
    let cancel_slot = cancellation.clone();
    cancel_button.connect_clicked(move |button| {
        if let Some(cancel) = cancel_slot.borrow().as_ref() {
            cancel.cancel();
        }
        button.set_sensitive(false);
    });
    let navigation = navigation.downgrade();
    let toasts = toasts.downgrade();
    let feedback_clone = feedback.clone();
    connect.connect_clicked(move |button| {
        if !button.is_sensitive() || cancellation.borrow().is_some() {
            return;
        }
        url_slot.borrow_mut().take();
        copy.set_sensitive(false);
        let cancel = Cancellation::new();
        *cancellation.borrow_mut() = Some(cancel.clone());
        button.set_sensitive(false);
        button.set_label("Waiting for Google…");
        spinner.set_visible(true);
        spinner.start();
        cancel_button.set_sensitive(true);
        cancel_button.set_visible(true);
        feedback_clone.set_label("Complete authorization in your web browser, then return here.");
        feedback_clone.set_visible(true);
        let (sender, receiver) = mpsc::channel();
        let worker_engine = engine.clone();
        std::thread::spawn(move || {
            let browser_sender = sender.clone();
            let browser_cancel = cancel.clone();
            let result = worker_engine.add_account(&cancel, move |url| {
                let (acknowledge, reply) = mpsc::channel();
                browser_sender
                    .send(AuthEvent::OpenBrowser(url.into(), acknowledge))
                    .map_err(|_| AuthError::BrowserUnavailable)?;
                let deadline = Instant::now() + Duration::from_secs(30);
                loop {
                    if browser_cancel.is_cancelled() {
                        return Err(AuthError::Cancelled);
                    }
                    if Instant::now() >= deadline {
                        return Err(AuthError::BrowserUnavailable);
                    }
                    match reply.recv_timeout(Duration::from_millis(100)) {
                        Ok(true) => return Ok(()),
                        Ok(false) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                            return Err(AuthError::BrowserUnavailable)
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    }
                }
            });
            let _ = sender.send(AuthEvent::Finished(result));
        });
        let copy = copy.clone();
        let url_slot = url_slot.clone();
        let window = window.clone();
        let button = button.clone();
        let spinner = spinner.clone();
        let cancel_button = cancel_button.clone();
        let feedback = feedback_clone.clone();
        let navigation = navigation.clone();
        let toasts = toasts.clone();
        let cancellation = cancellation.clone();
        let page_engine = engine.clone();
        gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
            let result = match receiver.try_recv() {
                Ok(AuthEvent::OpenBrowser(url, acknowledge)) => {
                    if cancellation
                        .borrow()
                        .as_ref()
                        .is_some_and(Cancellation::is_cancelled)
                    {
                        let _ = acknowledge.send(false);
                        return gtk::glib::ControlFlow::Continue;
                    }
                    *url_slot.borrow_mut() = Some(url.clone());
                    copy.set_sensitive(true);
                    copy.set_tooltip_text(Some("Copy the current authorization link"));
                    gtk::glib::spawn_future_local(async move {
                        // GIO uses the desktop OpenURI portal inside Flatpak.
                        let launched = gtk::gio::AppInfo::launch_default_for_uri_future(
                            &url,
                            None::<&gtk::gio::AppLaunchContext>,
                        )
                        .await
                        .is_ok();
                        let _ = acknowledge.send(launched);
                    });
                    return gtk::glib::ControlFlow::Continue;
                }
                Ok(AuthEvent::Finished(result)) => result,
                Err(mpsc::TryRecvError::Empty) => return gtk::glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Err(EngineError::Authentication(AuthError::Internal))
                }
            };
            cancellation.borrow_mut().take();
            button.set_sensitive(true);
            button.set_label("Sign In…");
            copy.set_sensitive(false);
            copy.set_tooltip_text(Some("Available after starting sign-in"));
            url_slot.borrow_mut().take();
            spinner.stop();
            spinner.set_visible(false);
            cancel_button.set_visible(false);
            match result {
                Ok(account) => {
                    if let Some(window) = window.upgrade() {
                        window.close();
                    }
                    feedback.set_visible(false);
                    if let (Some(navigation), Some(toasts)) =
                        (navigation.upgrade(), toasts.upgrade())
                    {
                        crate::flow::open_account(
                            page_engine.clone(),
                            account,
                            &navigation,
                            &toasts,
                        );
                    }
                }
                Err(EngineError::Authentication(AuthError::Cancelled)) => {
                    feedback.set_label("");
                    feedback.set_visible(false);
                }
                Err(error) => {
                    feedback.set_label(&error.to_string());
                    if let Some(toasts) = toasts.upgrade() {
                        toasts.add_toast(adw::Toast::new(&error.to_string()));
                    }
                }
            }
            gtk::glib::ControlFlow::Break
        });
    });
}
