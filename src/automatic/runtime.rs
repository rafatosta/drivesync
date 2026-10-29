//! GTK-main-context coordination with blocking engine work on worker threads.
use super::{policy::Schedule, Phase};
use adw::{
    gtk::{gio, glib},
    prelude::*,
};
use drivesync_engine::{
    AccountId, Cancellation, DownloadProgress, DownloadReport, DriveSyncEngine, EngineError,
    FolderConfiguration, RestoredAccount,
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

type ResultReport = Result<DownloadReport, EngineError>;
type Execute = Arc<
    dyn Fn(&AccountId, &Cancellation, &mut dyn FnMut(DownloadProgress)) -> ResultReport
        + Send
        + Sync,
>;
thread_local! { static SERVICE: RefCell<Option<Rc<Service>>> = const { RefCell::new(None) }; }
fn now() -> u64 {
    glib::monotonic_time().max(0) as u64 / 1_000_000
}

struct Service {
    jobs: RefCell<HashMap<String, Controller>>,
    accounts: RefCell<HashSet<String>>,
    online: RefCell<bool>,
    stopped: RefCell<bool>,
}

/// A revisioned view of one account. Results retain engine issue paths and data
/// preservation evidence; the timer never treats partial completion as success.
#[derive(Clone)]
pub(crate) struct Snapshot {
    pub revision: u64,
    pub pausing: bool,
    pub phase: Phase,
    pub progress: Option<DownloadProgress>,
    pub result: Option<ResultReport>,
}
struct State {
    schedule: Schedule,
    folder: Option<FolderConfiguration>,
    eligible: bool,
    cancel: Option<Cancellation>,
    user_paused: bool,
    generation: u64,
    revision: u64,
    progress: Option<DownloadProgress>,
    result: Option<ResultReport>,
}
struct Inner {
    id: AccountId,
    execute: Execute,
    state: RefCell<State>,
    desktop: bool,
}

/// Shared account job. All mutations happen on the main context; worker threads
/// receive only the executor, account identity, cancellation and progress mailbox.
#[derive(Clone)]
pub(crate) struct Controller(Rc<Inner>);
impl Controller {
    fn new(id: AccountId, execute: Execute, desktop: bool) -> Self {
        Self(Rc::new(Inner {
            id,
            execute,
            desktop,
            state: RefCell::new(State {
                schedule: Schedule::new(false, now()),
                folder: None,
                eligible: false,
                cancel: None,
                user_paused: false,
                generation: 0,
                revision: 0,
                progress: None,
                result: None,
            }),
        }))
    }
    /// Returns the application-owned controller, or an isolated manual controller
    /// for explicit desktop fixtures that do not install the application service.
    pub fn attach(engine: Arc<DriveSyncEngine>, saved: &RestoredAccount) -> Self {
        if let Some(job) = SERVICE.with(|service| {
            service
                .borrow()
                .as_ref()
                .and_then(|s| s.jobs.borrow().get(&saved.account.id.0).cloned())
        }) {
            return job;
        }
        Self::for_engine(engine, saved, false)
    }
    fn for_engine(engine: Arc<DriveSyncEngine>, saved: &RestoredAccount, desktop: bool) -> Self {
        let worker_engine = engine.clone();
        let execute: Execute = Arc::new(move |id, cancel, progress| {
            worker_engine.synchronize_once(id, cancel, progress)
        });
        let job = Self::new(saved.account.id.clone(), execute, desktop);
        job.configure(saved.folder.clone(), super::eligible(saved));
        if saved.folder.is_some() {
            // Do not dispatch automatically before durable suspension is restored.
            job.0.state.borrow_mut().schedule.due = None;
            let restore = job.clone();
            let generation = job.0.state.borrow().generation;
            let id = saved.account.id.clone();
            crate::flow::work(
                move || engine.root_suspension(&id),
                move |result| {
                    let error = match result {
                        Ok(Some(reason)) => Some(EngineError::RootSuspended(reason)),
                        Ok(None) => None,
                        Err(error) => Some(error),
                    };
                    restore.restore_availability(generation, error);
                },
            );
        }
        job
    }
    /// A late disk read cannot replace a manual cycle or a newer configuration.
    fn restore_availability(&self, generation: u64, error: Option<EngineError>) {
        let mut state = self.0.state.borrow_mut();
        if state.generation != generation
            || state.cancel.is_some()
            || state.result.is_some()
            || !state.eligible
        {
            return;
        }
        if let Some(error) = error {
            let result = Err(error);
            state.schedule.finish(&result, now());
            state.result = Some(result);
        } else {
            state.schedule.configure(true, now());
        }
        state.revision += 1;
    }
    fn configure(&self, folder: Option<FolderConfiguration>, eligible: bool) {
        let mut state = self.0.state.borrow_mut();
        if state.folder == folder && state.eligible == eligible {
            return;
        }
        state.folder = folder;
        state.eligible = eligible;
        state.generation += 1;
        state.user_paused = false;
        if let Some(cancel) = &state.cancel {
            cancel.cancel();
        }
        let enabled = eligible && state.folder.is_some();
        state.schedule.configure(enabled, now());
        state.progress = None;
        state.result = None;
        state.revision += 1;
    }
    pub fn folder(&self) -> Option<FolderConfiguration> {
        self.0.state.borrow().folder.clone()
    }
    pub fn revision(&self) -> u64 {
        self.0.state.borrow().revision
    }
    pub fn snapshot(&self) -> Snapshot {
        let state = self.0.state.borrow();
        Snapshot {
            revision: state.revision,
            pausing: state
                .cancel
                .as_ref()
                .is_some_and(Cancellation::is_cancelled),
            phase: if state.cancel.is_some() {
                Phase::Running
            } else {
                state.schedule.phase
            },
            progress: state.progress.clone(),
            result: state.result.clone(),
        }
    }
    pub fn running(&self) -> bool {
        self.0.state.borrow().cancel.is_some()
    }
    /// Requests a safe pause. The worker retains its running guard until the
    /// current engine operation reaches a cancellation checkpoint or safe commit.
    pub fn pause(&self) {
        let mut state = self.0.state.borrow_mut();
        if let Some(cancel) = &state.cancel {
            cancel.cancel();
            state.user_paused = true;
            state.revision += 1;
        }
    }
    pub fn request(&self) {
        let online =
            SERVICE.with(|service| service.borrow().as_ref().is_none_or(|s| *s.online.borrow()));
        self.start(now(), online, true);
    }
    fn network_changed(&self, online: bool) {
        let mut state = self.0.state.borrow_mut();
        state.schedule.network_changed(online, now());
        state.revision += 1;
    }
    fn start(&self, timestamp: u64, online: bool, manual: bool) {
        let (cancel, generation) = {
            let mut state = self.0.state.borrow_mut();
            let previous = state.schedule.phase;
            if !state.schedule.start(timestamp, online, manual) {
                if previous != state.schedule.phase {
                    state.revision += 1;
                }
                return;
            }
            let cancel = Cancellation::new();
            state.cancel = Some(cancel.clone());
            state.generation += 1;
            state.user_paused = false;
            state.progress = None;
            state.result = None;
            state.revision += 1;
            (cancel, state.generation)
        };
        if self.0.desktop {
            crate::background::set_activity(true);
        }
        let latest = Arc::new(Mutex::new(None));
        let receive = latest.clone();
        let updates = self.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let mut state = updates.0.state.borrow_mut();
            if state.cancel.is_none() || state.generation != generation {
                return glib::ControlFlow::Break;
            }
            if let Some(progress) = receive.lock().ok().and_then(|mut slot| slot.take()) {
                state.progress = Some(progress);
                state.revision += 1;
            }
            glib::ControlFlow::Continue
        });
        let worker = self.0.execute.clone();
        let id = self.0.id.clone();
        let completion = self.clone();
        crate::flow::work(
            move || {
                worker(&id, &cancel, &mut |event| {
                    if let Ok(mut slot) = latest.lock() {
                        *slot = Some(event);
                    }
                })
            },
            move |result| {
                if completion.0.desktop {
                    crate::background::set_activity(false);
                }
                let mut state = completion.0.state.borrow_mut();
                state.cancel = None;
                state.progress = None;
                if state.generation == generation {
                    if state.user_paused {
                        state.schedule.finish(
                            &Err(EngineError::Authentication(
                                drivesync_engine::AuthError::Cancelled,
                            )),
                            now(),
                        );
                    } else {
                        state.schedule.finish(&result, now());
                    }
                    state.result = Some(result);
                } else {
                    // The old root/account must reach a safe stop before replacement.
                    state.schedule.running = false;
                }
                state.revision += 1;
            },
        );
    }
}

/// Installs one lifetime timer and one connectivity observer for the application.
/// No credentials, filesystem access or network calls run in the timer callback.
pub(crate) fn install(app: &adw::Application) {
    let monitor = gio::NetworkMonitor::default();
    let service = Rc::new(Service {
        jobs: RefCell::new(HashMap::new()),
        accounts: RefCell::new(HashSet::new()),
        online: RefCell::new(monitor.is_network_available()),
        stopped: RefCell::new(false),
    });
    SERVICE.with(|slot| *slot.borrow_mut() = Some(service.clone()));
    let weak = Rc::downgrade(&service);
    monitor.connect_network_changed(move |_, online| {
        if let Some(service) = weak.upgrade() {
            *service.online.borrow_mut() = online;
            for job in service.jobs.borrow().values() {
                job.network_changed(online);
            }
        }
    });
    let weak = Rc::downgrade(&service);
    glib::timeout_add_local(Duration::from_secs(1), move || {
        let Some(service) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if *service.stopped.borrow() {
            return glib::ControlFlow::Break;
        }
        service
            .jobs
            .borrow_mut()
            .retain(|id, job| service.accounts.borrow().contains(id) || job.running());
        for job in service.jobs.borrow().values() {
            job.start(now(), *service.online.borrow(), false);
        }
        publish_idle_status(&service);
        glib::ControlFlow::Continue
    });
    app.connect_shutdown(move |_| {
        *service.stopped.borrow_mut() = true;
        for job in service.jobs.borrow().values() {
            job.configure(None, false);
        }
        SERVICE.with(|slot| slot.borrow_mut().take());
    });
}

/// Reuses jobs across navigation replacements and cancels removed accounts.
/// Only accounts with saved folder pairs and usable authentication are scheduled.
pub(crate) fn reconcile(engine: Arc<DriveSyncEngine>, accounts: &[RestoredAccount]) {
    SERVICE.with(|slot| {
        if let Some(service) = slot.borrow().as_ref() {
            *service.accounts.borrow_mut() =
                accounts.iter().map(|a| a.account.id.0.clone()).collect();
            let mut jobs = service.jobs.borrow_mut();
            jobs.retain(|id, job| {
                let keep = accounts.iter().any(|a| &a.account.id.0 == id);
                if !keep {
                    job.configure(None, false);
                }
                keep || job.running()
            });
            for saved in accounts {
                let job = jobs
                    .entry(saved.account.id.0.clone())
                    .or_insert_with(|| Controller::for_engine(engine.clone(), saved, true));
                job.configure(saved.folder.clone(), super::eligible(saved));
            }
        }
    });
}

/// A successful folder save rearms automatic work using the newly validated pair.
pub(crate) fn configured(folder: FolderConfiguration) {
    SERVICE.with(|slot| {
        if let Some(job) = slot
            .borrow()
            .as_ref()
            .and_then(|s| s.jobs.borrow().get(&folder.account_id.0).cloned())
        {
            job.configure(Some(folder), true);
            let mut state = job.0.state.borrow_mut();
            if !state.schedule.running {
                state.schedule.configure(true, now());
                state.result = None;
                state.revision += 1;
            }
        }
    });
}

fn publish_idle_status(service: &Service) {
    let phases: Vec<_> = service
        .jobs
        .borrow()
        .values()
        .map(|job| job.0.state.borrow().schedule.phase)
        .collect();
    let message = if phases
        .iter()
        .any(|p| matches!(p, Phase::Blocked | Phase::Attention))
    {
        "Synchronization needs attention"
    } else if phases.contains(&Phase::Offline) {
        "Waiting for a connection"
    } else if phases.contains(&Phase::Retrying) {
        "Waiting to retry synchronization"
    } else if phases
        .iter()
        .any(|p| matches!(p, Phase::Waiting | Phase::UpToDate | Phase::Running))
    {
        "Automatic synchronization enabled"
    } else if phases.contains(&Phase::Paused) {
        "Synchronization paused"
    } else {
        "Choose folders to enable synchronization"
    };
    crate::background::set_idle_status(message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use drivesync_engine::{AuthError, DriveFolder};
    use std::{
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
        time::Instant,
    };

    fn folder(id: &str, name: &str) -> FolderConfiguration {
        FolderConfiguration {
            account_id: AccountId(id.into()),
            remote: DriveFolder {
                id: name.into(),
                name: name.into(),
            },
            local: format!("/tmp/{name}").into(),
            local_display: format!("/tmp/{name}").into(),
        }
    }
    fn wait_until(context: &glib::MainContext, condition: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !condition() {
            assert!(Instant::now() < deadline, "worker completion timed out");
            while context.pending() {
                context.iteration(false);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    /// Verifies the real application timer keeps dispatching without a visible
    /// window. Injected workers never initialize an engine or access user data.
    #[test]
    #[ignore = "requires a desktop sandbox, session bus and background permission"]
    fn hidden_window_runs_automatic_cycles() {
        assert_eq!(
            std::env::var("DRIVESYNC_BACKGROUND_TEST").as_deref(),
            Ok("1")
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let app = adw::Application::builder()
            .application_id(crate::APP_ID)
            .build();
        let calls_check = calls.clone();
        let observed = Rc::new(RefCell::new(None));
        let result = observed.clone();
        app.connect_startup(move |app| {
            install(app);
            SERVICE.with(|slot| *slot.borrow().as_ref().unwrap().online.borrow_mut() = true);
            let calls = calls_check.clone();
            let execute: Execute = Arc::new(move |_, _, _| {
                if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(EngineError::Authentication(AuthError::Network))
                } else {
                    Ok(DownloadReport {
                        unchanged: 1,
                        issues: vec![drivesync_engine::DownloadIssue {
                            relative_path: Some("test/local-edit.txt".into()),
                            error: EngineError::DownloadConflict,
                        }],
                        ..Default::default()
                    })
                }
            });
            let job = Controller::new(AccountId("automatic-desktop-fixture".into()), execute, true);
            job.configure(
                Some(folder(
                    "automatic-desktop-fixture",
                    "automatic-desktop-fixture",
                )),
                true,
            );
            SERVICE.with(|slot| {
                let service = slot.borrow();
                let service = service.as_ref().unwrap();
                service.accounts.borrow_mut().insert(job.0.id.0.clone());
                service
                    .jobs
                    .borrow_mut()
                    .insert(job.0.id.0.clone(), job.clone());
            });
            let window = adw::ApplicationWindow::builder()
                .application(app)
                .title("DriveSync automatic background test")
                .build();
            crate::background::install(app, &window);
            window.present();
            let close = window.downgrade();
            glib::timeout_add_local_once(Duration::from_millis(300), move || {
                if let Some(window) = close.upgrade() {
                    window.close();
                }
            });
            let retry = job.clone();
            glib::timeout_add_local_once(Duration::from_secs(5), move || {
                retry.network_changed(true)
            });
            let app = app.clone();
            let observed = observed.clone();
            glib::timeout_add_local_once(Duration::from_secs(10), move || {
                *observed.borrow_mut() = Some((!window.is_visible(), job.snapshot().phase));
                app.quit();
            });
        });
        app.run_with_args::<&str>(&[]);
        assert_eq!(*result.borrow(), Some((true, Phase::Attention)));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    /// Runs scripted workers with no GTK windows, credentials, files or network.
    /// Exercises the same worker dispatch/completion paths used by real downloads.
    #[test]
    fn automatic_workers_retry_preserve_guards_and_reject_obsolete_results() {
        let context = glib::MainContext::default();
        let _guard = context.acquire().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(AtomicBool::new(false));
        let calls_worker = calls.clone();
        let release_worker = release.clone();
        let execute: Execute = Arc::new(move |_, cancel, progress| {
            let n = calls_worker.fetch_add(1, Ordering::SeqCst);
            progress(DownloadProgress {
                preparation: None,
                completed_items: 0,
                total_items: 1,
                current: None,
            });
            while !release_worker.load(Ordering::SeqCst) && !cancel.is_cancelled() {
                std::thread::sleep(Duration::from_millis(2));
            }
            if cancel.is_cancelled() {
                return Err(EngineError::Authentication(AuthError::Cancelled));
            }
            if n == 0 {
                Err(EngineError::Authentication(AuthError::Network))
            } else {
                Ok(DownloadReport {
                    unchanged: 1,
                    ..Default::default()
                })
            }
        });
        let job = Controller::new(AccountId("a".into()), execute, false);
        job.configure(Some(folder("a", "first")), true);
        let service = Rc::new(Service {
            jobs: RefCell::new(HashMap::from([("a".into(), job.clone())])),
            accounts: RefCell::new(HashSet::from(["a".into()])),
            online: RefCell::new(true),
            stopped: RefCell::new(false),
        });
        SERVICE.with(|slot| *slot.borrow_mut() = Some(service));
        job.start(now() + 3, true, false);
        wait_until(&context, || calls.load(Ordering::SeqCst) == 1);
        for _ in 0..20 {
            job.request();
            job.start(now() + 999, true, false);
        }
        // Navigation attachment of the same configuration must not restart it.
        let saved = RestoredAccount {
            account: drivesync_engine::AccountSummary {
                id: AccountId("a".into()),
                provider_subject: "a".into(),
                email: None,
            },
            error: None,
            folder: Some(folder("a", "first")),
        };
        let engine = Arc::new(DriveSyncEngine::new());
        reconcile(engine.clone(), std::slice::from_ref(&saved));
        let reattached = Controller::attach(engine.clone(), &saved);
        assert!(Rc::ptr_eq(&job.0, &reattached.0));
        assert!(job.running());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        release.store(true, Ordering::SeqCst);
        wait_until(&context, || !job.running());
        assert_eq!(job.snapshot().phase, Phase::Offline);
        let retry = job.0.state.borrow().schedule.due.unwrap();
        job.start(retry - 1, true, false);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        job.start(retry, true, false);
        wait_until(&context, || !job.running());
        assert_eq!(job.snapshot().phase, Phase::UpToDate);
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        release.store(false, Ordering::SeqCst);
        job.request();
        wait_until(&context, || calls.load(Ordering::SeqCst) == 3);
        job.pause();
        job.request();
        assert!(job.snapshot().pausing);
        wait_until(&context, || !job.running());
        assert_eq!(job.snapshot().phase, Phase::Paused);
        job.start(now() + 10000, true, false);
        assert_eq!(calls.load(Ordering::SeqCst), 3);

        job.request();
        wait_until(&context, || calls.load(Ordering::SeqCst) == 4);
        job.configure(Some(folder("a", "replacement")), true);
        job.request();
        assert!(job.running());
        wait_until(&context, || !job.running());
        assert_eq!(job.snapshot().phase, Phase::Waiting);
        assert!(
            job.snapshot().result.is_none(),
            "obsolete cancellation must not pause replacement"
        );
        release.store(true, Ordering::SeqCst);
        job.start(now() + 3, true, false);
        wait_until(&context, || !job.running());
        assert_eq!(calls.load(Ordering::SeqCst), 5);
        assert_eq!(job.snapshot().phase, Phase::UpToDate);

        release.store(false, Ordering::SeqCst);
        job.request();
        wait_until(&context, || calls.load(Ordering::SeqCst) == 6);
        reconcile(engine, &[]); // Account removal uses the same cancellation boundary.
        wait_until(&context, || !job.running());
        job.start(now() + 10000, true, true);
        assert_eq!(calls.load(Ordering::SeqCst), 6);
        assert_eq!(job.snapshot().phase, Phase::Unconfigured);
        // Cancellation after the executor returns but before delivery still pauses.
        let returned = Arc::new(AtomicBool::new(false));
        let ready = returned.clone();
        let fast = Controller::new(
            AccountId("second".into()),
            Arc::new(move |_, _, _| {
                ready.store(true, Ordering::SeqCst);
                Ok(DownloadReport::default())
            }),
            false,
        );
        fast.configure(Some(folder("second", "second")), true);
        fast.request();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !returned.load(Ordering::SeqCst) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        fast.pause();
        wait_until(&context, || !fast.running());
        assert_eq!(fast.snapshot().phase, Phase::Paused);
        assert!(fast.snapshot().result.unwrap().is_ok());
        let restored = Controller::new(
            AccountId("restored".into()),
            Arc::new(|_, _, _| Ok(DownloadReport::default())),
            false,
        );
        restored.configure(Some(folder("restored", "volume")), true);
        let generation = restored.0.state.borrow().generation;
        let unavailable =
            EngineError::RootSuspended(drivesync_engine::RootSuspension::LocalUnavailable);
        restored.restore_availability(generation, Some(unavailable.clone()));
        assert_eq!(restored.snapshot().phase, Phase::Blocked);
        assert!(
            matches!(restored.snapshot().result, Some(Err(ref error)) if error == &unavailable)
        );
        restored.configure(Some(folder("restored", "replacement")), true);
        restored.restore_availability(generation, Some(unavailable));
        assert_eq!(restored.snapshot().phase, Phase::Waiting);
        assert!(restored.snapshot().result.is_none());
        let generation = restored.0.state.borrow().generation;
        restored.0.state.borrow_mut().schedule.due = None;
        assert!(!restored
            .0
            .state
            .borrow_mut()
            .schedule
            .start(now() + 10000, true, false));
        restored.restore_availability(generation, None);
        assert!(restored.0.state.borrow().schedule.due.is_some());
        SERVICE.with(|slot| slot.borrow_mut().take());
        // Drain retired progress sources before releasing the default context.
        let until = Instant::now() + Duration::from_millis(120);
        while Instant::now() < until {
            while context.pending() {
                context.iteration(false);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
