//! Account-isolated persistence for synchronization baselines and operation state.
//!
//! The Inventory stores facts and transaction revisions only. It does not collect observations,
//! analyze synchronization, or execute filesystem or provider operations.

mod activity;
mod database;
pub(crate) mod models;

pub(crate) use activity::{
    Activity, ActivityInbox, LookupOutcome as ActivityLookupOutcome,
    LookupSummary as ActivityLookupSummary, Origin as ActivityOrigin,
    Persistence as ActivityPersistence, Phase as ActivityPhase, Scope as ActivityScope,
    State as ActivityState,
};

use database::Database;
use models::{
    InventoryError, InventoryLookup, InventoryLookupResult, InventoryWrite, InventoryWriteResult,
    WriteStatus,
};
use std::{
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::{Mutex, MutexGuard, TryLockError},
    time::{Duration, Instant},
};

/// Private storage location and account scope for one Inventory instance.
pub(crate) struct InventoryConfig {
    /// Dedicated private directory in which per-account databases are stored.
    pub directory: PathBuf,
    /// Account whose synchronization records this instance may access.
    pub account_id: crate::models::AccountId,
}

/// Typed persistence facade for one account's synchronization state.
///
/// A database is opened or created beneath the configured directory. Its filename is derived
/// from the opaque account identifier; every SQL operation also filters by account and sync root.
pub(crate) struct Inventory {
    account_id: crate::models::AccountId,
    database: Mutex<Database>,
    activity: Option<(ActivityInbox, ActivityOrigin)>,
}

struct DatabaseAccess<'a> {
    database: Option<MutexGuard<'a, Database>>,
    activity: Option<activity::Guard>,
}

impl Deref for DatabaseAccess<'_> {
    type Target = Database;

    fn deref(&self) -> &Self::Target {
        self.database.as_deref().expect("database access is held")
    }
}

impl DerefMut for DatabaseAccess<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.database
            .as_deref_mut()
            .expect("database access is held")
    }
}

impl Drop for DatabaseAccess<'_> {
    fn drop(&mut self) {
        drop(self.database.take());
        if let Some(activity) = self.activity.take() {
            activity.finish(ActivityState::Complete);
        }
    }
}

impl Inventory {
    /// Opens this account's private database without reading or modifying account credentials.
    pub(crate) fn open(config: InventoryConfig) -> Result<Self, InventoryError> {
        let database = Database::open(&config.directory, &config.account_id)?;
        Ok(Self {
            account_id: config.account_id,
            database: Mutex::new(database),
            activity: None,
        })
    }

    /// Opens the account database and reports its existing preparation work to the scoped inbox.
    pub(crate) fn open_observed(
        config: InventoryConfig,
        inbox: ActivityInbox,
        origin: ActivityOrigin,
    ) -> Result<Self, InventoryError> {
        let mut activity = inbox.open(origin);
        if let Some(activity) = activity.as_mut() {
            activity.phase(ActivityPhase::Prepare, ActivityState::Running);
        }
        let result = Self::open(config);
        if let Some(activity) = activity {
            activity.finish(if result.is_ok() {
                ActivityState::Complete
            } else {
                ActivityState::Unavailable
            });
        }
        result.map(|mut inventory| {
            inventory.activity = Some((inbox, origin));
            inventory
        })
    }

    /// Holds an OS lease on this account database while Operations dispatches or recovers effects.
    /// Closing the descriptor releases ownership after cancellation or process exit.
    pub(crate) fn execution_lease(&self) -> Result<std::fs::File, InventoryError> {
        use std::os::{
            fd::AsRawFd,
            unix::fs::{MetadataExt, OpenOptionsExt},
        };
        let activity = self
            .activity
            .as_ref()
            .and_then(|(inbox, origin)| inbox.lease(*origin));
        let result = (|| {
            let database = self.database().map_err(|_| InventoryError::Unavailable)?;
            let path = database
                .connection
                .path()
                .ok_or(InventoryError::Unavailable)?;
            let file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)
                .map_err(|_| InventoryError::Unavailable)?;
            let meta = file.metadata().map_err(|_| InventoryError::Unavailable)?;
            if !meta.is_file()
                || meta.uid() != unsafe { libc::geteuid() }
                || meta.nlink() != 1
                || unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0
            {
                return Err(InventoryError::Unavailable);
            }
            Ok(file)
        })();
        if let Some(mut activity) = activity {
            if result.is_ok() {
                activity.phase(ActivityPhase::Lease, ActivityState::Complete);
                activity.finish(ActivityState::Complete);
            } else {
                activity.finish(ActivityState::Unavailable);
            }
        }
        result
    }

    /// Returns a conclusive result only when the database and scope were readable.
    pub(crate) fn lookup(&self, request: InventoryLookup) -> InventoryLookupResult {
        let started = self.activity.as_ref().map(|_| Instant::now());
        let result = if request.account_id != self.account_id {
            InventoryLookupResult::unavailable("Requested account is outside this Inventory scope.")
        } else {
            match self.database() {
                Ok(database) => database.lookup(request),
                Err(()) => InventoryLookupResult::unavailable("Inventory is unavailable."),
            }
        };
        if let Some((inbox, _)) = &self.activity {
            let outcome = match result.status {
                models::InventoryLookupStatus::Found => ActivityLookupOutcome::Found,
                models::InventoryLookupStatus::NotFound => ActivityLookupOutcome::NotFound,
                models::InventoryLookupStatus::Unavailable => ActivityLookupOutcome::Unavailable,
                models::InventoryLookupStatus::Ambiguous | models::InventoryLookupStatus::Error => {
                    ActivityLookupOutcome::Rejected
                }
            };
            inbox.record_lookup(outcome);
        }
        if let Some(started) =
            started.filter(|started| started.elapsed() >= Duration::from_millis(200))
        {
            if let Some((inbox, origin)) = &self.activity {
                if let Some(activity) = inbox.slow_lookup(*origin, started) {
                    activity.finish(match result.status {
                        models::InventoryLookupStatus::Found => ActivityState::Complete,
                        models::InventoryLookupStatus::NotFound => ActivityState::NotFound,
                        models::InventoryLookupStatus::Unavailable => ActivityState::Unavailable,
                        models::InventoryLookupStatus::Ambiguous
                        | models::InventoryLookupStatus::Error => ActivityState::Rejected,
                    });
                }
            }
        }
        result
    }

    /// Reads all facts in one account/root, failing rather than returning a partial view.
    pub(crate) fn enumerate(
        &self,
        query: models::InventoryScopeQuery,
    ) -> Result<Vec<models::InventorySnapshot>, InventoryError> {
        if query.account_id != self.account_id {
            return Err(InventoryError::InvalidConfiguration);
        }
        let mut activity = self
            .activity
            .as_ref()
            .and_then(|(inbox, origin)| inbox.enumerate(*origin));
        let result = match self.database() {
            Ok(database) => database.enumerate_observed(query, activity.as_mut()),
            Err(()) => Err(InventoryError::Unavailable),
        };
        if let Some(activity) = activity {
            activity.finish(if result.is_ok() {
                ActivityState::Complete
            } else {
                ActivityState::Unavailable
            });
        }
        result
    }

    /// Atomically records a typed operation effect with optional optimistic revision checking.
    pub(crate) fn write(&self, request: InventoryWrite) -> InventoryWriteResult {
        let _sample = crate::performance::Sample::begin("inventory_write");
        let mut activity = self.activity.as_ref().and_then(|(inbox, origin)| {
            let persistence = match &request.effect {
                models::PersistedEffect::ItemRegistered { .. } => ActivityPersistence::Identity,
                models::PersistedEffect::ConvergenceConfirmed { .. }
                | models::PersistedEffect::BaselineConfirmed { .. } => {
                    ActivityPersistence::Baseline
                }
                models::PersistedEffect::Pending(_) => ActivityPersistence::PendingOperation,
                models::PersistedEffect::RecoveryCheckpoint(_) => {
                    ActivityPersistence::RecoveryCheckpoint
                }
                models::PersistedEffect::RecoveryObserved { .. } => {
                    ActivityPersistence::RecoveryAssessment
                }
                models::PersistedEffect::OperationApplied(_) => {
                    ActivityPersistence::OperationEffect
                }
                models::PersistedEffect::OperationConfirmed => {
                    ActivityPersistence::OperationConfirmation
                }
                models::PersistedEffect::OperationFailed => ActivityPersistence::OperationFailure,
                models::PersistedEffect::ConflictUpdated(_) => ActivityPersistence::Conflict,
            };
            inbox.write(*origin, persistence)
        });
        if request.identity.account_id != self.account_id {
            let result =
                InventoryWriteResult::new(WriteStatus::Rejected, None, request.recorded_at);
            if let Some(activity) = activity {
                activity.finish(ActivityState::Rejected);
            }
            return result;
        }
        let recorded_at = request.recorded_at;
        let mut rolled_back = false;
        let result = match self.database() {
            Ok(mut database) => {
                let (result, transaction_rolled_back) =
                    database.write_observed(request, activity.as_mut());
                rolled_back = transaction_rolled_back;
                result
            }
            Err(()) => InventoryWriteResult::new(WriteStatus::Unavailable, None, recorded_at),
        };
        if let Some(mut activity) = activity {
            if rolled_back {
                activity.phase(ActivityPhase::Rollback, ActivityState::RolledBack);
            }
            activity.finish(match result.status {
                WriteStatus::Written => ActivityState::Complete,
                WriteStatus::RevisionConflict | WriteStatus::Rejected => ActivityState::Rejected,
                WriteStatus::Unavailable if rolled_back => ActivityState::RolledBack,
                WriteStatus::Unavailable => ActivityState::Unavailable,
            });
        }
        result
    }

    fn database(&self) -> Result<DatabaseAccess<'_>, ()> {
        if self.activity.is_none() {
            return self
                .database
                .lock()
                .map(|database| DatabaseAccess {
                    database: Some(database),
                    activity: None,
                })
                .map_err(|_| ());
        }
        let mut activity = None;
        let database = match self.database.try_lock() {
            Ok(database) => database,
            Err(TryLockError::Poisoned(_)) => {
                if let Some((inbox, origin)) = &self.activity {
                    if let Some(activity) = inbox.waiting_for_mutex(*origin) {
                        activity.finish(ActivityState::Failed);
                    }
                }
                return Err(());
            }
            Err(TryLockError::WouldBlock) => {
                let Some((inbox, origin)) = &self.activity else {
                    return self
                        .database
                        .lock()
                        .map(|database| DatabaseAccess {
                            database: Some(database),
                            activity: None,
                        })
                        .map_err(|_| ());
                };
                let mut waiting = inbox.waiting_for_mutex(*origin);
                let database = match self.database.lock() {
                    Ok(database) => database,
                    Err(_) => {
                        if let Some(waiting) = waiting.take() {
                            waiting.finish(ActivityState::Failed);
                        }
                        return Err(());
                    }
                };
                if let Some(waiting) = waiting.as_mut() {
                    waiting.phase(ActivityPhase::Access, ActivityState::Running);
                }
                activity = waiting;
                database
            }
        };
        Ok(DatabaseAccess {
            database: Some(database),
            activity,
        })
    }
}

#[cfg(test)]
mod access_tests {
    use super::*;
    use crate::models::SyncRootId;
    use std::sync::Arc;

    fn observed_inventory() -> (Arc<Inventory>, ActivityInbox, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "drivesync-inventory-access-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let account_id = crate::models::AccountId("access-test-account".into());
        let inbox = ActivityInbox::new(ActivityScope {
            account: account_id.clone(),
            root: Some(SyncRootId("access-test-root".into())),
        });
        let inventory = Inventory::open_observed(
            InventoryConfig {
                directory: path.clone(),
                account_id,
            },
            inbox.clone(),
            ActivityOrigin::Detectors,
        )
        .unwrap();
        (Arc::new(inventory), inbox, path)
    }

    #[test]
    fn mutex_wait_is_published_before_access_and_finished_after_release() {
        let (inventory, inbox, path) = observed_inventory();
        let held = inventory.database.lock().unwrap();
        let worker_inventory = inventory.clone();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            drop(worker_inventory.database().unwrap());
        });
        started_rx.recv().unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        let waiting = loop {
            if let Some(activity) = inbox.current().into_iter().find(|activity| {
                activity.kind == activity::Kind::Access
                    && activity.state == ActivityState::WaitingForMutex
            }) {
                break activity;
            }
            assert!(Instant::now() < deadline, "mutex wait was not published");
            std::thread::yield_now();
        };
        assert_eq!(waiting.origin, ActivityOrigin::Detectors);

        drop(held);
        worker.join().unwrap();
        assert!(inbox.current().is_empty());
        assert!(inbox.recent().iter().any(|activity| {
            activity.id == waiting.id
                && activity.phase == ActivityPhase::Access
                && activity.state == ActivityState::Complete
        }));
        drop(inventory);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn uncontended_access_does_not_create_an_activity() {
        let (inventory, inbox, path) = observed_inventory();
        drop(inventory.database().unwrap());
        assert!(inbox.current().is_empty());
        assert!(!inbox
            .recent()
            .iter()
            .any(|activity| activity.kind == activity::Kind::Access));
        drop(inventory);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn poisoned_mutex_acquisition_is_reported_as_failed() {
        let (inventory, inbox, path) = observed_inventory();
        let _ = std::panic::catch_unwind(|| {
            let _database = inventory.database.lock().unwrap();
            panic!("poison inventory mutex for the controlled test");
        });
        assert!(inventory.database().is_err());
        assert!(inbox.recent().iter().any(|activity| {
            activity.kind == activity::Kind::Access && activity.state == ActivityState::Failed
        }));
        drop(inventory);
        std::fs::remove_dir_all(path).unwrap();
    }
}
