//! Exact durable plans; remote conditional tags are private and have no Debug.
use crate::{ConfirmedBase, ConflictRecord, EngineError, InventoryEntry};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

/// Explicit content selection. Both keeps local at the original identity and
/// publishes the observed remote version under a distinct synchronized name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionChoice {
    KeepLocal,
    KeepRemote,
    KeepBoth,
    /// A separately reviewed and verified plain-text candidate.
    MergeText,
}

/// A decision bound to the exact saved evidence displayed by the interface.
/// Construction performs no I/O and grants no permission to mutate a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionRequest {
    pub(crate) record: ConflictRecord,
    pub(crate) choice: ResolutionChoice,
    pub(crate) merge: Option<super::text::MergeEvidence>,
}
impl ResolutionRequest {
    /// Requires a pending binary-file proposal with both observed identities.
    /// Same-path replacements require explicit consent and fresh executor checks.
    /// The executor independently revalidates the record and live versions.
    pub fn from_record(
        record: &ConflictRecord,
        choice: ResolutionChoice,
    ) -> Result<Self, EngineError> {
        if choice == ResolutionChoice::MergeText {
            return Err(EngineError::InvalidPlan);
        }
        crate::ResolutionProposal::from_record(record).map_err(|_| EngineError::InvalidPlan)?;
        if !matches!(
            record.diagnosis,
            crate::ChangeKind::ConcurrentChanges
                | crate::ChangeKind::LocalIdentityChanged
                | crate::ChangeKind::ConcurrentIdentityChanges
                | crate::ChangeKind::LocalChanged
                | crate::ChangeKind::RemoteChanged
                | crate::ChangeKind::LocationConflict
        ) {
            return Err(EngineError::InvalidPlan);
        }
        Ok(Self {
            record: record.clone(),
            choice,
            merge: None,
        })
    }
}

/// Opaque server precondition, never reconstructed from a numeric revision.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoteVersion {
    pub item: InventoryEntry,
    pub etag: String,
}
impl RemoteVersion {
    pub fn valid(&self) -> bool {
        let item = &self.item;
        crate::configuration::valid_id(&item.id)
            && crate::configuration::valid_id(&item.parent_id)
            && item.kind == crate::InventoryKind::Binary
            && item.version.is_some_and(|v| v > 0)
            && item.size.is_some()
            && item.md5_checksum.as_deref().is_some_and(|v| hex(v, 32))
            && crate::planner::safe(&item.name)
            && self.etag.len() <= 1024
            && self.etag.starts_with('"')
            && self.etag.ends_with('"')
            && self.etag.len() > 2
            && self.etag[1..self.etag.len() - 1]
                .bytes()
                .all(|b| b == 0x21 || (0x23..=0x7e).contains(&b))
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Plan {
    pub conflict: Option<String>,
    pub origin: PathBuf,
    pub target: PathBuf,
    pub local_fingerprint: String,
    pub remote: RemoteVersion,
    pub choice: ResolutionChoice,
    pub base: Option<ConfirmedBase>,
    pub merge: Option<super::text::MergeEvidence>,
    pub desired_name: String,
    pub copy_name: Option<String>,
    pub copy_id: Option<String>,
}

impl Plan {
    /// Consent may bind a new local inode only at the exact confirmed path and
    /// unchanged remote location. Automatic plans and text merges cannot adopt it.
    pub(crate) fn explicit_replacement(&self) -> bool {
        self.conflict.is_some()
            && self.merge.is_none()
            && self.choice != ResolutionChoice::MergeText
            && self.origin == self.target
            && self
                .base
                .as_ref()
                .and_then(|b| b.location.as_ref())
                .is_some_and(|l| {
                    l.relative_path == self.origin
                        && l.remote_name == self.remote.item.name
                        && l.remote_parent == self.remote.item.parent_id
                        && self.origin.file_name().and_then(|v| v.to_str())
                            == Some(l.remote_name.as_str())
                })
    }
}

/// Ordered durable boundaries. Unknown remote effects remain Pending on retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum Stage {
    Aborted,
    Preparing,
    Preserved,
    CopyPending,
    RemotePending,
    RemoteVerified,
    LocalPending,
    Verified,
    Confirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Blob {
    pub identity: crate::download::Identity,
    pub size: u64,
    pub sha256: String,
    pub md5: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Intent {
    pub id: String,
    pub plan: Plan,
    pub root: crate::download::Identity,
    pub parent: crate::download::Identity,
    pub stage: Stage,
    pub local: Option<Blob>,
    pub remote: Option<Blob>,
    pub merged: Option<Blob>,
    pub result: Option<RemoteVersion>,
    pub copy_result: Option<RemoteVersion>,
    pub installed: Option<crate::download::Identity>,
    pub copy_installed: Option<crate::download::Identity>,
    pub retry: crate::upload::Retry,
}
impl Intent {
    pub fn temporary(&self, kind: &str) -> String {
        format!(".drivesync-{}-{kind}.part", self.id)
    }
    pub fn backup(&self) -> String {
        format!("DriveSync backup conflict-{}.bak", self.id)
    }
    pub fn winner(&self) -> Result<&Blob, EngineError> {
        if self.plan.choice == ResolutionChoice::MergeText {
            self.merged.as_ref()
        } else if self.plan.choice == ResolutionChoice::KeepRemote {
            self.remote.as_ref()
        } else {
            self.local.as_ref()
        }
        .ok_or(EngineError::StateStorage)
    }
    pub fn valid(&self) -> bool {
        ((self.plan.choice == ResolutionChoice::MergeText) == self.plan.merge.is_some())
            && self
                .plan
                .merge
                .as_ref()
                .is_none_or(|proof| proof.valid(&self.plan))
            && self.merged.as_ref().is_none_or(|blob| {
                self.plan.merge.as_ref().is_some_and(|proof| {
                    blob.sha256 == super::text::sha(&proof.result)
                        && blob.md5 == super::text::md5(&proof.result)
                        && blob.size == proof.result.len() as u64
                        && blob.identity.inode > 0
                })
            })
            && (self.stage < Stage::Preserved
                || self.plan.choice != ResolutionChoice::MergeText
                || self.merged.is_some())
            && hex(&self.id, 64)
            && self.plan.conflict.as_deref().is_none_or(|id| hex(id, 64))
            && (self.plan.choice != ResolutionChoice::KeepBoth || self.plan.conflict.is_some())
            && self.root.inode > 0
            && self.parent.inode > 0
            && relative(&self.plan.origin)
            && relative(&self.plan.target)
            && self.plan.origin.parent() == self.plan.target.parent()
            && crate::planner::fingerprint_parts(&self.plan.local_fingerprint).is_some()
            && self.plan.remote.valid()
            && crate::planner::safe(&self.plan.desired_name)
            && self.plan.base.as_ref().is_some_and(|base| {
                let old = crate::planner::fingerprint_parts(&base.local_fingerprint);
                let current = crate::planner::fingerprint_parts(&self.plan.local_fingerprint);
                old.is_some()
                    && current.is_some()
                    && (matches!((old,current),(Some((_,d,i)),Some((_,cd,ci))) if (d,i)==(cd,ci))
                        || self.plan.explicit_replacement())
                    && self
                        .plan
                        .remote
                        .item
                        .version
                        .is_some_and(|v| v >= base.remote_version && base.remote_version > 0)
                    && base.location.as_ref().is_some_and(|location| {
                        relative(&location.relative_path)
                            && location.relative_path.parent() == self.plan.origin.parent()
                            && location.remote_parent == self.plan.remote.item.parent_id
                    })
            })
            && self.plan.target.file_name().and_then(|v| v.to_str())
                == Some(self.plan.desired_name.as_str())
            && self
                .plan
                .copy_name
                .as_ref()
                .is_none_or(|v| crate::planner::safe(v))
            && self
                .plan
                .copy_id
                .as_deref()
                .is_none_or(crate::configuration::valid_id)
            && self
                .plan
                .copy_id
                .as_ref()
                .is_none_or(|v| v != &self.plan.remote.item.id)
            && self.plan.copy_name.as_ref().is_none_or(|v| {
                crate::planner::path_key(v) != crate::planner::path_key(&self.plan.desired_name)
            })
            && (self.plan.choice == ResolutionChoice::KeepBoth) == self.plan.copy_name.is_some()
            && self.plan.copy_id.is_some() == self.plan.copy_name.is_some()
            && self.retry.attempts <= 5
            && [&self.local, &self.remote]
                .into_iter()
                .flatten()
                .all(|v| v.identity.inode > 0 && hex(&v.sha256, 64) && hex(&v.md5, 32))
            && (self.stage < Stage::Preserved || (self.local.is_some() && self.remote.is_some()))
            && self.local.as_ref().is_none_or(|v| {
                crate::planner::fingerprint_parts(&self.plan.local_fingerprint)
                    .is_some_and(|(hash, _, _)| hash == v.sha256)
            })
            && self.remote.as_ref().is_none_or(|v| {
                Some(v.size) == self.plan.remote.item.size
                    && Some(&v.md5) == self.plan.remote.item.md5_checksum.as_ref()
            })
            && (self.stage < Stage::RemoteVerified
                || self.result.as_ref().is_some_and(RemoteVersion::valid))
            && (self.stage < Stage::Verified || self.installed.is_some_and(|v| v.inode > 0))
            && (self.stage < Stage::Verified
                || self.plan.choice != ResolutionChoice::KeepBoth
                || self.copy_installed.is_some_and(|v| v.inode > 0))
            && self.result.as_ref().is_none_or(|v| {
                v.valid()
                    && v.item.id == self.plan.remote.item.id
                    && v.item.parent_id == self.plan.remote.item.parent_id
                    && v.item.name == self.plan.desired_name
                    && self.winner().is_ok_and(|blob| {
                        v.item.size == Some(blob.size)
                            && v.item.md5_checksum.as_ref() == Some(&blob.md5)
                    })
            })
            && self.copy_result.as_ref().is_none_or(|v| {
                v.valid()
                    && Some(&v.item.id) == self.plan.copy_id.as_ref()
                    && Some(&v.item.name) == self.plan.copy_name.as_ref()
                    && v.item.parent_id == self.plan.remote.item.parent_id
                    && self.remote.as_ref().is_some_and(|blob| {
                        v.item.size == Some(blob.size)
                            && v.item.md5_checksum.as_ref() == Some(&blob.md5)
                    })
            })
    }
}
fn hex(value: &str, size: usize) -> bool {
    value.len() == size && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.components().all(|p| matches!(p, Component::Normal(_)))
        && !crate::local::ignored(path)
}
