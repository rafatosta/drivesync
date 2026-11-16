//! Shared account action coordination, independent of component internals.
use super::Context;
use adw::gtk;
use drivesync_engine::Account;
use std::rc::Rc;
/// Confirmation and execution outcome, safe for direct presentation.
pub(crate) enum DisconnectEvent {
    Cancelled,
    Disconnected { draft_removed: bool },
    Failed(String),
}
/// Requests confirmation and then invokes the nonvisual disconnection service.
/// The caller owns its busy state and decides how to refresh its presentation.
pub(crate) fn disconnect(
    context: &Context,
    parent: &gtk::Window,
    account: &Account,
    completed: Rc<dyn Fn(DisconnectEvent)>,
) {
    let context = context.clone();
    let id = account.id.clone();
    let identity = account
        .email
        .as_deref()
        .or(account.display_name.as_deref())
        .unwrap_or(&account.provider_user_id);
    crate::ui::components::confirmation::disconnect(parent, identity, move |confirmed| {
        if !confirmed {
            completed(DisconnectEvent::Cancelled);
            return;
        }
        let Some(engine) = context.engine.borrow().clone() else {
            completed(DisconnectEvent::Failed(gettextrs::gettext(
                "The account service is unavailable. Try again.",
            )));
            return;
        };
        let id = id.clone();
        let completed = completed.clone();
        let context = context.clone();
        gtk::glib::spawn_future_local(async move {
            context
                .detector_generation
                .set(context.detector_generation.get().wrapping_add(1));
            super::detectors::remove(&context, &id).await;
            let result = crate::services::accounts::disconnect(&engine, id).await;
            completed(match result {
                Ok(draft_removed) => DisconnectEvent::Disconnected { draft_removed },
                Err(error) => DisconnectEvent::Failed(crate::core::messages::engine_error(&error)),
            });
        });
    });
}
