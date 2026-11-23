//! Consolidates one normalized observation with persisted facts into an Analyzer-ready file.

mod audit;
mod cache;
mod capture;
mod models;

use crate::{
    detectors::observation::{
        LocalObservation, LocalObservationSource, Observation, ObservationCoverage,
        RemoteObservation,
    },
    inventory::{
        models::{Baseline, InventoryLookup, InventoryLookupStatus, LocalState, RemoteState},
        Inventory, InventoryConfig,
    },
    models::{Evidence, FileIdentity, FileState, ObservationOrigin, Path, SyncRootId},
};
use std::path::Component;

#[allow(unused_imports)]
// Environment is part of the File contract constructed by Context callers.
pub(crate) use models::Environment;
pub(crate) use models::{AssociationCandidate, ContextConfig, File, Observations, PathRoot};

/// Builds Analyzer input from one observation and its account-scoped persisted snapshot.
pub(crate) struct Context {
    inventory: Inventory,
    config: ContextConfig,
    cache: std::sync::Mutex<cache::Cache>,
    captures: std::sync::Mutex<capture::Captures>,
    rounds: std::sync::Mutex<()>,
}

impl Context {
    /// Opens the configured Inventory and binds this Context to one account and sync root.
    pub(crate) fn open(config: ContextConfig) -> Result<Self, ContextError> {
        let local_root: &PathRoot = &config.local_root;
        if !local_root.0.is_absolute()
            || config.account_id.0.trim().is_empty()
            || config.provider_id.0.trim().is_empty()
            || config.sync_root_id.0.trim().is_empty()
            || config.remote_root_id.0.trim().is_empty()
        {
            return Err(ContextError::InvalidConfiguration);
        }
        let inventory = Inventory::open(InventoryConfig {
            directory: config.inventory_directory.clone(),
            account_id: config.account_id.clone(),
        })
        .map_err(|_| ContextError::InventoryUnavailable)?;
        Ok(Self {
            inventory,
            config,
            cache: std::sync::Mutex::new(cache::Cache::default()),
            captures: std::sync::Mutex::new(capture::Captures::default()),
            rounds: std::sync::Mutex::new(()),
        })
    }

    /// Consumes only normalized observations; a completed scope can recheck many known items.
    pub(crate) fn consolidate(&self, observation: Observation) -> Result<Vec<File>, ContextError> {
        match observation {
            Observation::Capture(capture) => self.build_capture(*capture),
            observation => self.build(observation).map(|file| vec![file]),
        }
    }

    /// Consolidates one observation without inferring missing evidence or choosing an action.
    fn build(&self, observation: Observation) -> Result<File, ContextError> {
        let mut file = self.build_item(observation)?;
        self.enrich(&mut file)?;
        Ok(file)
    }

    fn build_item(&self, observation: Observation) -> Result<File, ContextError> {
        let (sync_root_id, local, remote, observations, remote_id, lookup_local_identity, path) =
            match observation {
                Observation::Capture(_) => return Err(ContextError::InvalidCapture),
                Observation::Local { source, data } => {
                    self.validate_scope(&data.sync_root_id)?;
                    validate_local_paths(&data)?;
                    let origin = match source {
                        LocalObservationSource::Scanner => ObservationOrigin::Scanner,
                        LocalObservationSource::Watcher => ObservationOrigin::Local,
                    };
                    let observations = Observations {
                        local_epoch: None,
                        remote_epoch: None,
                        remote_generation: None,
                        events: data.event.iter().cloned().collect(),
                        latest_origin: origin,
                        local_coverage: data.coverage,
                        remote_coverage: ObservationCoverage::Unavailable,
                        local_generation: None,
                        remote_page_token: None,
                        observed_at: data.observed_at,
                    };
                    let path = data.path.clone();
                    let lookup_local_identity = data.identity.clone();
                    let local = local_state(data);
                    (
                        self.config.sync_root_id.clone(),
                        local,
                        unknown_remote_state(),
                        observations,
                        None,
                        lookup_local_identity,
                        path,
                    )
                }
                Observation::Remote(data) => {
                    self.validate_remote_scope(&data)?;
                    let observations = Observations {
                        local_epoch: None,
                        remote_epoch: None,
                        remote_generation: None,
                        events: data.event.iter().cloned().collect(),
                        latest_origin: ObservationOrigin::Remote,
                        local_coverage: ObservationCoverage::Unavailable,
                        remote_coverage: data.coverage,
                        local_generation: None,
                        remote_page_token: data.page_token.clone(),
                        observed_at: data.observed_at,
                    };
                    let remote_id = data.remote_id.clone();
                    let remote = remote_state(data);
                    (
                        self.config.sync_root_id.clone(),
                        unknown_local_state(),
                        remote,
                        observations,
                        remote_id,
                        Evidence::NotCollected,
                        None,
                    )
                }
            };

        let lookup = match (&remote_id, &lookup_local_identity, &path) {
            (Some(remote_id), local_identity, local_path) => Some(InventoryLookup {
                account_id: self.config.account_id.clone(),
                sync_root_id: sync_root_id.clone(),
                item_id: None,
                remote_id: Some(remote_id.clone()),
                local_identity: local_identity.clone(),
                local_path: local_path.clone(),
            }),
            (None, Evidence::Known(_), local_path) => Some(InventoryLookup {
                account_id: self.config.account_id.clone(),
                sync_root_id: sync_root_id.clone(),
                item_id: None,
                remote_id: None,
                local_identity: lookup_local_identity.clone(),
                local_path: local_path.clone(),
            }),
            (None, _, Some(local_path)) => Some(InventoryLookup {
                account_id: self.config.account_id.clone(),
                sync_root_id: sync_root_id.clone(),
                item_id: None,
                remote_id: None,
                local_identity: Evidence::NotCollected,
                local_path: Some(local_path.clone()),
            }),
            (None, _, None) => None,
        };
        let lookup_result = lookup
            .map(|request| self.inventory.lookup(request))
            .map(|result| (result.status, result.snapshot));
        let (lookup_status, snapshot) =
            match lookup_result.unwrap_or((InventoryLookupStatus::Error, None)) {
                (InventoryLookupStatus::Found, Some(snapshot)) => {
                    (InventoryLookupStatus::Found, Some(snapshot))
                }
                (InventoryLookupStatus::NotFound, None) => (InventoryLookupStatus::NotFound, None),
                (status, None) if status != InventoryLookupStatus::Found => (status, None),
                _ => (InventoryLookupStatus::Error, None),
            };

        let registered_path = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.local_path.clone());
        let (identity, baseline, pending_operations, prior_conflict, inventory_revision) =
            match (lookup_status, snapshot) {
                (InventoryLookupStatus::Found, Some(snapshot)) => {
                    let mut identity = snapshot.identity;
                    identity.remote_id = remote_id.or(identity.remote_id);
                    (
                        identity,
                        snapshot.baseline,
                        snapshot.pending_operations,
                        snapshot.prior_conflict,
                        Some(snapshot.revision),
                    )
                }
                (status, _) => (
                    FileIdentity {
                        account_id: self.config.account_id.clone(),
                        sync_root_id,
                        item_id: None,
                        remote_id,
                        local_identity: lookup_local_identity,
                    },
                    match status {
                        InventoryLookupStatus::NotFound => Baseline::never_synced(),
                        _ => Baseline::unavailable(),
                    },
                    Vec::new(),
                    None,
                    None,
                ),
            };

        let mut local = local;
        if local.path.is_none() {
            local.path = registered_path;
        }
        let registration_id = if lookup_status == InventoryLookupStatus::NotFound {
            use std::io::Read;
            let mut bytes = [0u8; 16];
            std::fs::File::open("/dev/urandom")
                .and_then(|mut source| source.read_exact(&mut bytes))
                .map_err(|_| ContextError::InventoryUnavailable)?;
            Some(crate::models::ItemId(
                bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
            ))
        } else {
            None
        };
        let mut file = File {
            identity,
            registration_id,
            local,
            remote,
            baseline,
            observations,
            environment: self.config.environment(),
            inventory_lookup: lookup_status,
            pending_operations,
            prior_conflict,
            initial_association_authorized: self.config.initial_association_authorized,
            inventory_revision,
            association_candidates: Vec::new(),
        };
        if let Ok(mut cache) = self.cache.lock() {
            cache.consolidate(&mut file);
        }
        Ok(file)
    }

    fn validate_scope(&self, sync_root_id: &SyncRootId) -> Result<(), ContextError> {
        if sync_root_id != &self.config.sync_root_id {
            return Err(ContextError::ScopeMismatch);
        }
        Ok(())
    }

    fn validate_remote_scope(&self, observation: &RemoteObservation) -> Result<(), ContextError> {
        self.validate_scope(&observation.sync_root_id)?;
        if observation.account_id != self.config.account_id
            || observation.provider_id != self.config.provider_id
        {
            return Err(ContextError::ScopeMismatch);
        }
        if let Some(event) = &observation.event {
            for path in event.old_path.iter().chain(event.new_path.iter()) {
                validate_relative_path(path)?;
            }
        }
        Ok(())
    }
}

fn validate_local_paths(observation: &LocalObservation) -> Result<(), ContextError> {
    for path in observation
        .path
        .iter()
        .chain(observation.previous_path.iter())
        .chain(
            observation
                .event
                .iter()
                .flat_map(|event| event.old_path.iter()),
        )
        .chain(
            observation
                .event
                .iter()
                .flat_map(|event| event.new_path.iter()),
        )
    {
        validate_relative_path(path)?;
    }
    if let Evidence::Known(parent) = &observation.parent {
        validate_relative_path(parent)?;
    }
    Ok(())
}

fn validate_relative_path(path: &Path) -> Result<(), ContextError> {
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ContextError::PathOutsideRoot);
    }
    Ok(())
}

fn local_state(observation: LocalObservation) -> LocalState {
    LocalState {
        state: observation.state,
        path: observation.path,
        name: observation.name,
        parent: observation.parent,
        kind: observation.kind,
        identity: observation.identity,
        size: observation.size,
        modified_at: observation.modified_at,
        changed_at: observation.changed_at,
        checksum: observation.checksum,
        capabilities: observation.capabilities,
    }
}

fn remote_state(observation: RemoteObservation) -> RemoteState {
    RemoteState {
        state: observation.state,
        id: observation.remote_id,
        name: observation.name,
        parents: observation.parents,
        mime_type: observation.mime_type,
        size: observation.size,
        checksum: observation.checksum,
        version: observation.version,
        modified_time: observation.modified_time,
        trashed: observation.trashed,
        capabilities: observation.capabilities,
    }
}

fn unknown_local_state() -> LocalState {
    LocalState {
        state: FileState::Unknown,
        path: None,
        name: Evidence::NotCollected,
        parent: Evidence::NotCollected,
        kind: Evidence::NotCollected,
        identity: Evidence::NotCollected,
        size: Evidence::NotCollected,
        modified_at: Evidence::NotCollected,
        changed_at: Evidence::NotCollected,
        checksum: Evidence::NotCollected,
        capabilities: Evidence::NotCollected,
    }
}

fn unknown_remote_state() -> RemoteState {
    RemoteState {
        state: FileState::Unknown,
        id: None,
        name: Evidence::NotCollected,
        parents: Evidence::NotCollected,
        mime_type: Evidence::NotCollected,
        size: Evidence::NotCollected,
        checksum: Evidence::NotCollected,
        version: Evidence::NotCollected,
        modified_time: Evidence::NotCollected,
        trashed: Evidence::NotCollected,
        capabilities: Evidence::NotCollected,
    }
}

/// Failure to construct a File from an observation and its configured synchronization scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextError {
    /// A required root or configured identity is empty or malformed.
    InvalidConfiguration,
    /// A capture is malformed, out of order or cannot be represented safely.
    InvalidCapture,
    /// The configured Inventory could not be opened.
    InventoryUnavailable,
    /// Observation account, provider, or sync root differs from this Context.
    ScopeMismatch,
    /// An observed local path is absolute or escapes the configured root.
    PathOutsideRoot,
}

impl std::fmt::Display for ContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Context could not build a File: {self:?}")
    }
}

impl std::error::Error for ContextError {}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod capture_tests;
