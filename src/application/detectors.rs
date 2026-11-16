//! Application-owned capture/preparation sessions; Developer pages only observe these same handles.
use super::Context;
use adw::gtk::{gio, glib};
use drivesync_engine::{
    detectors::*, models::SyncId, orchestrator::OrchestratorCreateRequest, Account, AccountId,
    ListAccountsRequest, Orchestrator,
};
use std::{path::PathBuf, sync::Arc, time::Duration};

pub(crate) struct Session {
    pub root: PathBuf,
    pub orchestrator: Arc<Orchestrator>,
    pub remote_root_id: String,
    pub remote_error: Option<String>,
    pub preparation: Option<glib::JoinHandle<()>>,
}
struct RefreshGuard(std::rc::Rc<std::cell::Cell<bool>>);
impl Drop for RefreshGuard {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
/// Requests preparation independently of any window being open or visible.
pub(crate) fn schedule_refresh(context: &Context) {
    if context.shutting_down.get() || context.engine.borrow().is_none() {
        return;
    }
    let context = context.clone();
    glib::spawn_future_local(async move {
        let _ = refresh(&context).await;
    });
}
fn prepare(session: &mut Session) {
    if let Some(task) = session.preparation.take() {
        task.abort();
    }
    let owner = session.orchestrator.clone();
    session.preparation = Some(glib::spawn_future_local(async move {
        let _ = owner.prepare().await;
    }));
}
/// Runs another local preparation while preserving paused capture and the durable event queue.
pub(crate) fn rescan(context: &Context, id: &str) -> Result<(), String> {
    if context.shutting_down.get() {
        return Err("Application is shutting down".into());
    }
    let mut sessions = context.detectors.borrow_mut();
    let session = sessions
        .get_mut(id)
        .ok_or("Account preparation is unavailable. Refresh to retry.")?;
    use drivesync_engine::orchestrator::PreparationState;
    if matches!(
        session.orchestrator.preparation_status().state,
        PreparationState::Starting
            | PreparationState::Scanning
            | PreparationState::Analyzing
            | PreparationState::ProcessingEvents
    ) {
        return Err("A scan is already running".into());
    }
    prepare(session);
    Ok(())
}
/// Cancels preparation on the main context, then stops and drops capture off the GTK thread.
pub(crate) async fn remove(context: &Context, id: &AccountId) {
    let session = context.detectors.borrow_mut().remove(&id.0);
    if let Some(mut session) = session {
        if let Some(task) = session.preparation.take() {
            task.abort();
        }
        let owner = session.orchestrator;
        let _ = gio::spawn_blocking(move || {
            let _ = owner.stop_detector(DetectorStopMode::StopNow);
        })
        .await;
    }
    context.preparation_errors.borrow_mut().remove(&id.0);
}
/// Serializes configuration discovery and replaces sessions only when the owning scope changes.
/// A page refresh reuses current sessions; it never starts duplicate scans or a diagnostic consumer.
pub(crate) async fn refresh(context: &Context) -> Result<Vec<(Account, Option<String>)>, String> {
    while context.preparation_refreshing.get() {
        if context.shutting_down.get() {
            return Err("Application is shutting down".into());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    if context.shutting_down.get() {
        return Err("Application is shutting down".into());
    }
    context.preparation_refreshing.set(true);
    let _guard = RefreshGuard(context.preparation_refreshing.clone());
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
    for account in accounts {
        if let Some(config) = crate::services::configuration::load(&account.id)
            .await
            .map_err(|_| "Saved account configuration could not be read")?
        {
            let root = crate::services::local_folder::watch_root(&config.local_uri).await;
            configured.push((account, root, config.remote_id));
        }
    }
    if context.shutting_down.get() || context.detector_generation.get() != generation {
        return Err("Account configuration changed; refresh again".into());
    }
    let removed = context
        .detectors
        .borrow()
        .keys()
        .filter(|id| !configured.iter().any(|(a, _, _)| &a.id.0 == *id))
        .cloned()
        .collect::<Vec<_>>();
    for id in removed {
        remove(context, &AccountId(id)).await;
    }
    let mut result = Vec::new();
    for (account, root, remote_root_id) in configured {
        let root = match root {
            Ok(root) => root,
            Err(error) => {
                remove(context, &account.id).await;
                context
                    .preparation_errors
                    .borrow_mut()
                    .insert(account.id.0.clone(), error.clone());
                result.push((account, Some(error)));
                continue;
            }
        };
        let existing = context
            .detectors
            .borrow()
            .get(&account.id.0)
            .is_some_and(|s| s.root == root && s.remote_root_id == remote_root_id);
        let error = if existing {
            None
        } else {
            remove(context, &account.id).await;
            let scope = DetectorScope {
                account_id: account.id.clone(),
                sync_id: SyncId(format!("developer-unified:{}", account.id.0)),
                local_root: root.clone(),
                remote_root_id: remote_root_id.clone(),
            };
            let connector = engine.clone();
            let created = gio::spawn_blocking(move || {
                Orchestrator::new(OrchestratorCreateRequest {
                    execute_creations: true,
                    detector: DetectorCreateRequest {
                        connector,
                        scope,
                        local_settings: LocalDetectorSettings::default(),
                        remote_settings: RemoteDetectorSettings::default(),
                        event_available_sink: None,
                    },
                })
                .map(Arc::new)
            })
            .await;
            if context.shutting_down.get() || context.detector_generation.get() != generation {
                let _ = gio::spawn_blocking(move || drop(created)).await;
                return Err("Account configuration changed; refresh again".into());
            }
            match created {
                Ok(Ok(orchestrator)) => {
                    let mut session = Session {
                        root,
                        orchestrator,
                        remote_root_id,
                        remote_error: None,
                        preparation: None,
                    };
                    prepare(&mut session);
                    context
                        .detectors
                        .borrow_mut()
                        .insert(account.id.0.clone(), session);
                    None
                }
                Ok(Err(e)) => Some(e.to_string()),
                Err(_) => Some("Could not create synchronization preparation".into()),
            }
        };
        if let Some(error) = &error {
            context
                .preparation_errors
                .borrow_mut()
                .insert(account.id.0.clone(), error.clone());
        } else {
            context
                .preparation_errors
                .borrow_mut()
                .remove(&account.id.0);
        }
        result.push((account, error));
    }
    Ok(result)
}
/// Cancels every foreground future before stopping shared capture; no GTK object crosses the worker boundary.
pub(crate) fn shutdown(context: &Context) {
    context.shutting_down.set(true);
    context
        .detector_generation
        .set(context.detector_generation.get().wrapping_add(1));
    let sessions = std::mem::take(&mut *context.detectors.borrow_mut());
    let mut owners = Vec::new();
    for (_, mut session) in sessions {
        if let Some(task) = session.preparation.take() {
            task.abort();
        }
        owners.push(session.orchestrator);
    }
    std::thread::spawn(move || {
        for owner in owners {
            let _ = owner.stop_detector(DetectorStopMode::StopNow);
        }
    });
}
