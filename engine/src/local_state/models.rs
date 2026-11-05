//! Verified identity records and conservative, durable investigation work.
use crate::{LocalActivity, LocalBatch, LocalItemKind, LocalWatchState};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::os::unix::ffi::OsStrExt;

pub(super) const MAX_ITEMS: usize = 10_000;
pub(super) const MAX_PENDING: usize = 4096;
pub(super) const MAX_PAYLOAD: usize = 16 * 1024 * 1024;

/// Stable application identity, independent of paths, inode numbers and Drive IDs.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemId(String);
impl ItemId {
    pub(super) fn new() -> Self {
        Self(format!(
            "local-{}",
            oauth2::CsrfToken::new_random().secret()
        ))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A caller-verified location and version references; neither version is calculated here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemSnapshot {
    /// None and an empty name identify the one root record.
    pub parent: Option<ItemId>,
    /// One raw Unix filename component; empty only for the root.
    pub name: Vec<u8>,
    pub kind: LocalItemKind,
    /// Opaque, verified content fingerprint supplied by an investigator.
    pub content_version: Option<String>,
    /// Opaque provider version supplied by a remote reader/executor.
    pub remote_version: Option<String>,
}

/// Absence from this scope is not proof of physical deletion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemState {
    Present(ItemSnapshot),
    Absent,
}

/// Last confirmed state remains separate from the current verified observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackedItem {
    pub id: ItemId,
    pub remote_id: Option<String>,
    /// None means no confirmed baseline exists, rather than confirmed absence.
    pub confirmed: Option<ItemState>,
    pub observed: ItemState,
}

/// Producer-supplied connectivity; OAuth success is not a connectivity oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalConnectivity {
    Online,
    Offline,
    Unknown,
}

/// Causes are grouped, not interpreted as a complete operation history.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum InvestigationReason {
    Created,
    Removed,
    Written,
    WriteClosed,
    Metadata,
    MoveSource,
    MoveDestination,
    Recheck,
    CapacityLimit,
}

/// Scheduling hints never bypass identity, reconciliation or confirmation checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WorkPriority {
    LocalChange,
    Reconciliation,
}

/// A need to investigate, never an executable upload/move/delete command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingInvestigation {
    pub id: i64,
    pub revision: i64,
    /// Raw relative path; empty means the entire scope.
    pub path: Vec<u8>,
    /// A record found at that path, not a verified association with this event.
    pub candidate_item: Option<ItemId>,
    pub reasons: BTreeSet<InvestigationReason>,
    pub priority: WorkPriority,
    pub requires_confirmation: bool,
    pub coverage_uncertain: bool,
}

/// One atomically published scope generation and its bounded work queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalStateCheckpoint {
    pub generation: i64,
    pub items: Vec<TrackedItem>,
    pub pending: Vec<PendingInvestigation>,
}

/// Sanitized persistence and state validation failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalStateError {
    Storage,
    Incompatible,
    Invalid,
    Stale,
    Limit,
    Missing,
}
impl std::fmt::Display for LocalStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Storage => "Local change state could not be stored or read.",
            Self::Incompatible => "Local change state has an unsupported or invalid format.",
            Self::Invalid => "The local identity, tree or pending work is invalid.",
            Self::Stale => "Local change state was updated concurrently; reload before deciding.",
            Self::Limit => "The local change state resource limit was reached.",
            Self::Missing => "The requested local record or pending work does not exist.",
        })
    }
}
impl std::error::Error for LocalStateError {}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Document {
    pub root: Option<ItemId>,
    pub items: Vec<TrackedItem>,
    pub pending: Vec<PendingInvestigation>,
    pub next_pending: i64,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            root: None,
            items: Vec::new(),
            pending: Vec::new(),
            next_pending: 1,
        }
    }
}

pub(super) fn text_valid(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control)
}
pub(super) fn relative_valid(path: &[u8]) -> bool {
    path.len() <= 4096
        && !path.contains(&0)
        && (path.is_empty()
            || path
                .split(|c| *c == b'/')
                .all(|p| !p.is_empty() && p != b"." && p != b".."))
}
impl Document {
    pub fn validate(&self) -> Result<(), LocalStateError> {
        if self.items.len() > MAX_ITEMS || self.pending.len() > MAX_PENDING {
            return Err(LocalStateError::Limit);
        }
        if self.next_pending < 1 || self.root.is_some() != !self.items.is_empty() {
            return Err(LocalStateError::Invalid);
        }
        let mut ids = HashSet::new();
        let mut remotes = HashSet::new();
        for item in &self.items {
            if !text_valid(item.id.as_str()) || !ids.insert(&item.id) {
                return Err(LocalStateError::Invalid);
            }
            if let Some(remote) = &item.remote_id {
                if !text_valid(remote) || !remotes.insert(remote) {
                    return Err(LocalStateError::Invalid);
                }
            }
        }
        if self.root.as_ref().is_some_and(|id| !ids.contains(id)) {
            return Err(LocalStateError::Invalid);
        }
        self.paths(false)?;
        self.paths(true)?;
        let mut deliveries = HashSet::new();
        let mut paths = HashSet::new();
        for work in &self.pending {
            if work.id <= 0
                || work.id >= self.next_pending
                || work.revision <= 0
                || work.reasons.is_empty()
                || !deliveries.insert(work.id)
                || !paths.insert(&work.path)
                || !relative_valid(&work.path)
                || work
                    .candidate_item
                    .as_ref()
                    .is_some_and(|id| !ids.contains(id))
            {
                return Err(LocalStateError::Invalid);
            }
        }
        Ok(())
    }

    /// Derives paths from parent IDs with bounded depth; validates both trees.
    pub fn paths(&self, confirmed: bool) -> Result<BTreeMap<Vec<u8>, ItemId>, LocalStateError> {
        let nodes: HashMap<_, _> = self.items.iter().map(|item| (&item.id, item)).collect();
        let mut result = BTreeMap::new();
        let state = |item: &TrackedItem| {
            if confirmed {
                item.confirmed.as_ref()
            } else {
                Some(&item.observed)
            }
            .cloned()
        };
        for item in &self.items {
            if !matches!(state(item), Some(ItemState::Present(_))) {
                continue;
            }
            let mut current = &item.id;
            let mut components = Vec::new();
            let mut seen = HashSet::new();
            loop {
                if seen.len() >= 128 || !seen.insert(current) {
                    return Err(LocalStateError::Invalid);
                }
                let node = nodes.get(current).ok_or(LocalStateError::Invalid)?;
                let Some(ItemState::Present(snapshot)) = state(node) else {
                    return Err(LocalStateError::Invalid);
                };
                if snapshot.kind == LocalItemKind::Unknown
                    || snapshot
                        .content_version
                        .as_ref()
                        .is_some_and(|v| !text_valid(v))
                    || snapshot
                        .remote_version
                        .as_ref()
                        .is_some_and(|v| !text_valid(v))
                {
                    return Err(LocalStateError::Invalid);
                }
                match snapshot.parent {
                    None if self.root.as_ref() == Some(current)
                        && snapshot.name.is_empty()
                        && snapshot.kind == LocalItemKind::Directory =>
                    {
                        break
                    }
                    Some(ref parent)
                        if !snapshot.name.is_empty()
                            && !snapshot.name.contains(&b'/')
                            && relative_valid(&snapshot.name) =>
                    {
                        components.push(snapshot.name);
                        let parent = nodes
                            .get_key_value(parent)
                            .ok_or(LocalStateError::Invalid)?
                            .0;
                        if !matches!(
                            state(nodes[parent]),
                            Some(ItemState::Present(ItemSnapshot {
                                kind: LocalItemKind::Directory,
                                ..
                            }))
                        ) {
                            return Err(LocalStateError::Invalid);
                        }
                        current = parent;
                    }
                    _ => return Err(LocalStateError::Invalid),
                }
            }
            components.reverse();
            let path = components.join(&b'/');
            if !relative_valid(&path) || result.insert(path, item.id.clone()).is_some() {
                return Err(LocalStateError::Invalid);
            }
        }
        Ok(result)
    }

    pub fn enqueue(
        &mut self,
        path: Vec<u8>,
        reason: InvestigationReason,
        confirmation: bool,
        uncertain: bool,
        candidates: &BTreeMap<Vec<u8>, ItemId>,
    ) -> Result<(), LocalStateError> {
        if !relative_valid(&path) {
            return Err(LocalStateError::Invalid);
        }
        let priority = if matches!(reason, InvestigationReason::Recheck) {
            WorkPriority::Reconciliation
        } else {
            WorkPriority::LocalChange
        };
        if let Some(work) = self.pending.iter_mut().find(|w| w.path == path) {
            work.revision = work.revision.checked_add(1).ok_or(LocalStateError::Limit)?;
            work.reasons.insert(reason);
            work.requires_confirmation |= confirmation;
            work.coverage_uncertain |= uncertain;
            work.priority = work.priority.min(priority);
            return Ok(());
        }
        if self.pending.len() == MAX_PENDING {
            let confirm = confirmation || self.pending.iter().any(|w| w.requires_confirmation);
            self.pending.clear();
            return self.enqueue(
                Vec::new(),
                InvestigationReason::CapacityLimit,
                confirm,
                true,
                candidates,
            );
        }
        let id = self.next_pending;
        self.next_pending = id.checked_add(1).ok_or(LocalStateError::Limit)?;
        self.pending.push(PendingInvestigation {
            id,
            revision: 1,
            candidate_item: candidates.get(&path).cloned(),
            path,
            reasons: BTreeSet::from([reason]),
            priority,
            requires_confirmation: confirmation,
            coverage_uncertain: uncertain,
        });
        Ok(())
    }

    pub fn ingest(
        &mut self,
        batch: &LocalBatch,
        connectivity: LocalConnectivity,
    ) -> Result<(), LocalStateError> {
        let candidates = self.paths(false)?;
        let confirmation = connectivity != LocalConnectivity::Online;
        for notice in &batch.notices {
            let path = notice.path.as_os_str().as_bytes().to_vec();
            let (reason, uncertain) = match &notice.activity {
                LocalActivity::Created => (InvestigationReason::Created, false),
                LocalActivity::Removed => (InvestigationReason::Removed, false),
                LocalActivity::Written => (InvestigationReason::Written, false),
                LocalActivity::ClosedAfterWrite => (InvestigationReason::WriteClosed, false),
                LocalActivity::MetadataChanged => (InvestigationReason::Metadata, false),
                LocalActivity::MovedIn => (InvestigationReason::MoveDestination, true),
                LocalActivity::MovedOut => (InvestigationReason::MoveSource, true),
                LocalActivity::Recheck(_) => (InvestigationReason::Recheck, true),
                LocalActivity::Moved { from } => {
                    self.enqueue(
                        from.as_os_str().as_bytes().to_vec(),
                        InvestigationReason::MoveSource,
                        confirmation,
                        true,
                        &candidates,
                    )?;
                    (InvestigationReason::MoveDestination, true)
                }
            };
            self.enqueue(path, reason, confirmation, uncertain, &candidates)?;
            if notice.object_type == LocalItemKind::Directory
                && matches!(
                    notice.activity,
                    LocalActivity::Moved { .. }
                        | LocalActivity::MovedIn
                        | LocalActivity::MovedOut
                        | LocalActivity::Removed
                )
            {
                self.enqueue(
                    Vec::new(),
                    InvestigationReason::Recheck,
                    confirmation,
                    true,
                    &candidates,
                )?;
            }
        }
        if batch.state != LocalWatchState::Observing {
            self.enqueue(
                Vec::new(),
                InvestigationReason::Recheck,
                confirmation,
                true,
                &candidates,
            )?;
        }
        Ok(())
    }
}
