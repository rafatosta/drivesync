//! Pure revalidation of a conflict resolution proposal against fresh evidence.
//!
//! A decision made while the user inspected a comparison is only valid for the
//! exact remote identity/revision and local fingerprint it was based on. Any
//! difference, or missing evidence, aborts the proposal; it never authorizes a
//! write and always requires a new comparison.
use super::{ConflictRecord, ConflictStatus};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Why a proposal can no longer be applied. Stable for durable audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum StaleResolutionReason {
    /// The stored conflict was settled or lacks the evidence a proposal needs.
    NotPending,
    RemoteMissing,
    RemoteIdentityChanged,
    RemoteRevisionChanged,
    LocalMissing,
    LocalChanged,
    /// Current evidence could not be collected, so nothing may be applied.
    EvidenceUnavailable,
}

/// Latest remote metadata for the item, collected after the user decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteState {
    Unknown,
    Missing,
    Present {
        id: String,
        parent_id: String,
        name: String,
        version: u64,
    },
}

/// Latest verified local state of the original path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalState {
    Unknown,
    Missing,
    Present { fingerprint: String },
}

/// Fresh evidence gathered immediately before applying a resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionObservation {
    pub relative_path: PathBuf,
    pub remote: RemoteState,
    pub local: LocalState,
}

/// Evidence the user's decision was based on, taken from the durable record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionProposal {
    remote_id: String,
    remote_parent_id: String,
    remote_name: String,
    remote_version: u64,
    relative_path: PathBuf,
    local_fingerprint: String,
}

impl ResolutionProposal {
    /// Builds a proposal only from a pending record with complete evidence.
    pub fn from_record(record: &ConflictRecord) -> Result<Self, StaleResolutionReason> {
        match (
            record.status,
            &record.remote_id,
            &record.remote_parent_id,
            &record.remote_name,
            record.remote_version,
            &record.relative_path,
            &record.local_fingerprint,
        ) {
            (
                ConflictStatus::Pending,
                Some(remote_id),
                Some(remote_parent_id),
                Some(remote_name),
                Some(remote_version),
                Some(relative_path),
                Some(local_fingerprint),
            ) => Ok(Self {
                remote_id: remote_id.clone(),
                remote_parent_id: remote_parent_id.clone(),
                remote_name: remote_name.clone(),
                remote_version,
                relative_path: relative_path.clone(),
                local_fingerprint: local_fingerprint.clone(),
            }),
            _ => Err(StaleResolutionReason::NotPending),
        }
    }

    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    /// Compares the proposal with current evidence; `Ok` is not a write permission.
    ///
    /// Unknown evidence is reported before differences so that a transient
    /// failure is not mistaken for proof of change, yet still blocks the action.
    pub fn revalidate(&self, current: &ResolutionObservation) -> Result<(), StaleResolutionReason> {
        use StaleResolutionReason as S;
        let remote = match &current.remote {
            RemoteState::Unknown => return Err(S::EvidenceUnavailable),
            RemoteState::Missing => return Err(S::RemoteMissing),
            RemoteState::Present {
                id,
                parent_id,
                name,
                version,
            } => (id, parent_id, name, *version),
        };
        let fingerprint = match &current.local {
            LocalState::Unknown => return Err(S::EvidenceUnavailable),
            LocalState::Missing => return Err(S::LocalMissing),
            LocalState::Present { fingerprint } => fingerprint,
        };
        if *remote.0 != self.remote_id
            || *remote.1 != self.remote_parent_id
            || *remote.2 != self.remote_name
            || current.relative_path != self.relative_path
        {
            return Err(S::RemoteIdentityChanged);
        }
        if remote.3 != self.remote_version {
            return Err(S::RemoteRevisionChanged);
        }
        if *fingerprint != self.local_fingerprint {
            return Err(S::LocalChanged);
        }
        Ok(())
    }
}
