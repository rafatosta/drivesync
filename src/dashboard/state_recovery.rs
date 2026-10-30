//! Explicit recovery action; the engine owns locking and preservation policy.
use adw::{gtk, prelude::*};
use drivesync_engine::{Cancellation, DriveSyncEngine, RestoredAccount};
use std::sync::Arc;

pub(super) fn button(
    engine: Arc<DriveSyncEngine>,
    saved: &RestoredAccount,
    status: &gtk::Label,
) -> gtk::Button {
    let button = gtk::Button::with_label("Recover synchronization state…");
    button.set_widget_name("recover-state");
    let job = crate::automatic::Controller::attach(engine.clone(), saved);
    let id = saved.account.id.clone();
    let status = status.downgrade();
    button.connect_clicked(move |button| {
        job.pause();
        if job.running() {
            if let Some(status) = status.upgrade() {
                status.set_label("Pausing safely. Wait for the current cycle to finish, then choose recovery again.");
            }
            return;
        }
        let parent = button.root().and_then(|root| root.downcast::<gtk::Window>().ok());
        let dialog = adw::MessageDialog::new(parent.as_ref(), Some("Recover synchronization state?"), Some("Existing files and original databases will be preserved. Valid state stays in use; invalid or incompatible state is replaced with empty observations. Files without confirmed installation evidence remain pending. If upload history is lost, uploads stay blocked until their previous results can be reconciled. Recovery works offline; start synchronization afterward to refresh observations."));
        dialog.add_responses(&[("cancel", "Cancel"), ("recover", "Recover state")]);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let engine = engine.clone();
        let id = id.clone();
        let status = status.clone();
        let weak_button = button.downgrade();
        dialog.connect_response(Some("recover"), move |_, _| {
            if let Some(button) = weak_button.upgrade() { button.set_sensitive(false); }
            let engine = engine.clone();
            let id = id.clone();
            let status = status.clone();
            let button = weak_button.clone();
            crate::flow::work(move || engine.recover_state(&id, &Cancellation::new()), move |result| {
                if let Some(button) = button.upgrade() { button.set_sensitive(true); }
                if let Some(status) = status.upgrade() {
                    status.set_label(&match result {
                        Ok(outcome) if outcome.uploads_blocked => "Local state recovered. Uploads remain blocked because prior upload results require reconciliation. Start synchronization to refresh download observations.".into(),
                        Ok(_) => "Local state verified or recovered. Start synchronization to refresh observations. Existing files were preserved.".into(),
                        Err(error) => error.to_string(),
                    });
                }
            });
        });
        dialog.present();
    });
    button
}
