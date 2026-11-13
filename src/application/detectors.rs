//! Developer observation sessions keyed by account, independent of GTK widgets.
use super::Context;
use adw::gtk::gio;
use drivesync_engine::{
    detectors::*, models::SyncId, Account, AccountId, ListAccountsRequest, Orchestrator, Timestamp,
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};

pub(crate) struct Session {
    pub root: PathBuf,
    pub orchestrator: Arc<Orchestrator>,
}
/// Stops the account's observation before disconnecting or replacing its scope.
pub(crate) async fn remove(context: &Context, id: &AccountId) {
    let session = context.detectors.borrow_mut().remove(&id.0);
    if let Some(session) = session {
        let _ = gio::spawn_blocking(move || {
            session.orchestrator.stop(LocalStopRequest {
                reason: LocalStopReason::UserRequest,
                requested_at: Timestamp { unix_millis: 0 },
            })
        })
        .await;
    }
}
/// Returns configured accounts and per-account startup errors. No cloud requests.
/// Existing sessions survive console closure; removed scopes are stopped on refresh.
pub(crate) async fn refresh(context: &Context) -> Result<Vec<(Account, Option<String>)>, String> {
    let generation = context.detector_generation.get();
    let engine = context
        .engine
        .borrow()
        .clone()
        .ok_or("Account service unavailable")?;
    let accounts = engine
        .list_accounts(ListAccountsRequest { provider: None })
        .await
        .map_err(|e| crate::core::messages::engine_error(&e))?
        .accounts;
    let mut configured = Vec::new();
    // Read all configurations before altering sessions: read errors preserve observations.
    for account in accounts {
        if let Some(config) = crate::services::configuration::load(&account.id)
            .await
            .map_err(|_| "Saved account configuration could not be read".to_owned())?
        {
            let root = crate::services::local_folder::watch_root(&config.local_uri).await;
            configured.push((account, root));
        }
    }
    if context.detector_generation.get() != generation {
        return Err("Account configuration changed; reload the detector page".into());
    }
    let removed: Vec<_> = context
        .detectors
        .borrow()
        .keys()
        .filter(|id| !configured.iter().any(|(a, _)| &a.id.0 == *id))
        .cloned()
        .collect();
    for id in removed {
        remove(context, &AccountId(id)).await;
    }
    let mut result = Vec::new();
    let queue_directory = adw::gtk::glib::user_data_dir().join("drivesync/detector-queues");
    for (account, root) in configured {
        let root = match root {
            Ok(root) => root,
            Err(error) => {
                remove(context, &account.id).await;
                result.push((account, Some(error)));
                continue;
            }
        };
        let existing = context
            .detectors
            .borrow()
            .get(&account.id.0)
            .map(|s| s.root == root)
            .unwrap_or(false);
        let error = if existing {
            None
        } else {
            remove(context, &account.id).await;
            let connector = engine.clone();
            let queue_directory = queue_directory.clone();
            let scope = LocalDetectorScope {
                account_id: account.id.clone(),
                // A diagnostic session identity, not a persisted synchronization registration.
                sync_id: SyncId(format!("developer-local:{}", account.id.0)),
                local_root: root.clone(),
            };
            let created = gio::spawn_blocking(move || {
                let orchestrator = Arc::new(Orchestrator::observe_persistent(
                    connector,
                    scope,
                    LocalDetectorSettings::default(),
                    queue_directory,
                    DeliveryState::Enabled,
                )?);
                // Preserve Failed status and diagnostics if root capture cannot start.
                let _ = orchestrator.start(LocalStartRequest {
                    mode: LocalStartMode::Initial,
                    requested_at: Timestamp { unix_millis: 0 },
                });
                Ok::<_, QueueFailure>(orchestrator)
            })
            .await
            .map_err(|_| "Could not create observation worker".to_owned())?;
            if context.detector_generation.get() != generation {
                let _ = gio::spawn_blocking(move || drop(created)).await;
                return Err("Account configuration changed; reload the detector page".into());
            }
            match created {
                Ok(orchestrator) => {
                    context
                        .detectors
                        .borrow_mut()
                        .insert(account.id.0.clone(), Session { root, orchestrator });
                    None
                }
                Err(error) => Some(error.to_string()),
            }
        };
        result.push((account, error));
    }
    Ok(result)
}
/// Detaches all sessions for shutdown; blocking destruction stays off the GTK thread.
pub(crate) fn shutdown(context: &Context) {
    context
        .detector_generation
        .set(context.detector_generation.get().wrapping_add(1));
    let sessions: HashMap<_, _> = std::mem::take(&mut *context.detectors.borrow_mut());
    std::thread::spawn(move || drop(sessions));
}
