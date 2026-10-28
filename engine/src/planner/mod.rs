//! Pure download decisions and conservative initial filesystem observations.
use crate::{AccountId, EngineError, InventoryKind, RemoteInventory, SkipReason};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

/// Local evidence attached to a plan; Unknown never authorizes a write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalObservation {
    Absent,
    Directory,
    UnmanagedFile,
    SymlinkOrSpecial,
    Unknown,
    /// Stable content hash and local device/inode from a descriptor-relative read.
    Verified(String),
}
/// A base comes only from a durable installed journal entry, never an inventory.
///
/// `local_fingerprint` binds the installed SHA-256 to its local device/inode.
/// The planner decodes these components independently. Device/inode equality is
/// observable evidence, not a permanent identity guarantee against inode reuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedBase {
    pub remote_version: u64,
    pub local_fingerprint: String,
}
/// A reason an item cannot be safely downloaded by the current plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictReason {
    ExistingLocalData,
    /// Content differs while the observed device/inode still matches B.
    LocalChanged,
    /// The local device/inode differs from B, even if the content is equal.
    LocalIdentityChanged,
    /// Both the local identity and remote revision differ from B.
    ConcurrentIdentityChanges,
    ConcurrentChanges,
    UnsafeOrAmbiguousPath,
    LocalTypeMismatch,
}
/// Decisions contain no upload/delete operations and are not executable permission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanAction {
    Download,
    CreateLink,
    NoOp,
    CreateDirectory,
    Blocked(ConflictReason),
    DeferredUnknown,
    Skipped(SkipReason),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanEntry {
    pub remote_id: String,
    pub relative_path: Option<PathBuf>,
    pub expected_remote_version: Option<u64>,
    pub expected_local: LocalObservation,
    pub action: PlanAction,
}
/// Prepared work scoped to a concrete account, remote root and local directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadPlan {
    pub account_id: AccountId,
    pub remote_root: String,
    pub local_root: PathBuf,
    pub entries: Vec<PlanEntry>,
}

/// Three-way rule with explicit verified evidence; no filesystem/provider effects.
pub fn decide(
    base: Option<&ConfirmedBase>,
    local: &LocalObservation,
    remote_version: u64,
) -> PlanAction {
    if remote_version == 0
        || base.is_some_and(|base| {
            fingerprint_parts(&base.local_fingerprint).is_none()
                || remote_version < base.remote_version
        })
    {
        return PlanAction::DeferredUnknown;
    }
    match local {
        LocalObservation::Unknown => PlanAction::DeferredUnknown,
        LocalObservation::Absent => PlanAction::Download,
        LocalObservation::Verified(fingerprint) => match base {
            Some(base) => {
                let Some((local_hash, local_device, local_inode)) = fingerprint_parts(fingerprint)
                else {
                    return PlanAction::DeferredUnknown;
                };
                let Some((base_hash, base_device, base_inode)) =
                    fingerprint_parts(&base.local_fingerprint)
                else {
                    return PlanAction::DeferredUnknown;
                };
                if (local_device, local_inode) != (base_device, base_inode) {
                    return PlanAction::Blocked(if base.remote_version == remote_version {
                        ConflictReason::LocalIdentityChanged
                    } else {
                        ConflictReason::ConcurrentIdentityChanges
                    });
                }
                if local_hash.eq_ignore_ascii_case(base_hash) {
                    if base.remote_version == remote_version {
                        PlanAction::NoOp
                    } else {
                        PlanAction::Download
                    }
                } else if base.remote_version == remote_version {
                    PlanAction::Blocked(ConflictReason::LocalChanged)
                } else {
                    PlanAction::Blocked(ConflictReason::ConcurrentChanges)
                }
            }
            None => PlanAction::Blocked(ConflictReason::ExistingLocalData),
        },
        LocalObservation::Directory | LocalObservation::SymlinkOrSpecial => {
            PlanAction::Blocked(ConflictReason::LocalTypeMismatch)
        }
        LocalObservation::UnmanagedFile => PlanAction::Blocked(ConflictReason::ExistingLocalData),
    }
}

/// Decodes the current internal evidence format without inferring identity from
/// malformed or incomplete data. Zero is valid for a device, but not an inode.
fn fingerprint_parts(value: &str) -> Option<(&str, u64, u64)> {
    let mut parts = value.split(':');
    let hash = parts.next()?;
    let device = parts.next()?;
    let inode = parts.next()?;
    if parts.next().is_some()
        || hash.len() != 64
        || !hash.bytes().all(|b| b.is_ascii_hexdigit())
        || device.is_empty()
        || !device.bytes().all(|b| b.is_ascii_digit())
        || inode.is_empty()
        || !inode.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let device = device.parse().ok()?;
    let inode = inode.parse().ok()?;
    (inode != 0).then_some((hash, device, inode))
}
fn safe(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name != ".drivesync"
        && name.len() <= 255
        && !name.contains('/')
        && !name.contains('\\')
        && !name.chars().any(char::is_control)
}
fn local_name(item: &crate::InventoryEntry) -> Option<String> {
    if !safe(&item.name) {
        return None;
    }
    // Keep browser shortcuts distinct from regular files with the same Drive name.
    let name = if item.kind == InventoryKind::GoogleWorkspaceLink {
        format!("{}.desktop", item.name)
    } else {
        item.name.clone()
    };
    safe(&name).then_some(name)
}

/// Builds paths by stable parent IDs and blocks ambiguity throughout descendants.
/// The planner performs no writes; the injected observer controls observation I/O.
pub(crate) fn plan(
    inventory: &RemoteInventory,
    local_root: &Path,
    mut observe: impl FnMut(&Path) -> LocalObservation,
) -> Result<DownloadPlan, EngineError> {
    plan_with_bases(inventory, local_root, &HashMap::new(), |_, path, _| {
        Ok(observe(path))
    })
}

/// Builds a plan using only durable, item-scoped installation evidence as B.
/// The observer supplies fresh local evidence and may perform reads or return
/// errors. The planner itself does not write files or persist installation evidence.
///
/// # Errors
///
/// Returns [`EngineError::InvalidPlan`] for invalid inventory/path structure and
/// propagates errors from the local observer.
pub(crate) fn plan_with_bases(
    inventory: &RemoteInventory,
    local_root: &Path,
    bases: &HashMap<String, ConfirmedBase>,
    mut observe: impl FnMut(
        &crate::InventoryEntry,
        &Path,
        Option<&ConfirmedBase>,
    ) -> Result<LocalObservation, EngineError>,
) -> Result<DownloadPlan, EngineError> {
    if !local_root.is_absolute() || inventory.entries.len() > 100_000 {
        return Err(EngineError::InvalidPlan);
    }
    let mut ids = HashSet::new();
    let mut siblings = HashMap::new();
    for e in &inventory.entries {
        if e.id == inventory.root_id || !ids.insert(e.id.as_str()) {
            return Err(EngineError::InvalidPlan);
        }
        if let Some(name) = local_name(e) {
            *siblings
                .entry((e.parent_id.clone(), name))
                .or_insert(0usize) += 1;
        }
    }
    let mut children: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, e) in inventory.entries.iter().enumerate() {
        children.entry(&e.parent_id).or_default().push(index);
    }
    let mut paths: Vec<Option<PathBuf>> = vec![None; inventory.entries.len()];
    let mut queue =
        std::collections::VecDeque::from([(inventory.root_id.as_str(), Some(PathBuf::new()))]);
    let mut visited = HashSet::new();
    while let Some((parent, parent_path)) = queue.pop_front() {
        if let Some(indices) = children.get(parent) {
            for &index in indices {
                if !visited.insert(index) {
                    return Err(EngineError::InvalidPlan);
                }
                let e = &inventory.entries[index];
                let path = local_name(e).and_then(|name| {
                    let key = (e.parent_id.clone(), name.clone());
                    (siblings.get(&key) == Some(&1))
                        .then(|| parent_path.as_ref().map(|p| p.join(&name)))
                        .flatten()
                        .filter(|p| p.as_os_str().len() <= 4096)
                });
                paths[index] = path.clone();
                if e.kind == InventoryKind::Folder {
                    queue.push_back((&e.id, path));
                }
            }
        }
    }
    let mut entries = Vec::new();
    for (e, path) in inventory.entries.iter().zip(paths) {
        let base = bases.get(&e.id);
        let local = match path.as_ref() {
            Some(path) => observe(e, path, base)?,
            None => LocalObservation::Unknown,
        };
        let action = match (&e.kind, &path) {
            (InventoryKind::Skipped(reason), _) => PlanAction::Skipped(reason.clone()),
            (_, None) => PlanAction::Blocked(ConflictReason::UnsafeOrAmbiguousPath),
            (InventoryKind::Folder, Some(_)) => match local {
                LocalObservation::Absent => PlanAction::CreateDirectory,
                LocalObservation::Directory => PlanAction::NoOp,
                LocalObservation::Unknown => PlanAction::DeferredUnknown,
                _ => PlanAction::Blocked(ConflictReason::LocalTypeMismatch),
            },
            (InventoryKind::GoogleWorkspaceLink, Some(_)) => PlanAction::CreateLink,
            (InventoryKind::Binary, Some(_)) => match e.version {
                Some(version) => decide(base, &local, version),
                None => PlanAction::DeferredUnknown,
            },
        };
        entries.push(PlanEntry {
            remote_id: e.id.clone(),
            relative_path: path,
            expected_remote_version: e.version,
            expected_local: local,
            action,
        });
    }
    Ok(DownloadPlan {
        account_id: inventory.account_id.clone(),
        remote_root: inventory.root_id.clone(),
        local_root: local_root.into(),
        entries,
    })
}

/// Metadata-only observation. Static symlinks/special files block all descendants.
/// This does not remove races; the future executor must revalidate before writing.
pub(crate) fn observe(root: &Path, relative: &Path) -> LocalObservation {
    if !root.is_absolute()
        || relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return LocalObservation::Unknown;
    }
    let mut current = root.to_path_buf();
    let parts: Vec<_> = relative.components().collect();
    for (index, part) in parts.iter().enumerate() {
        current.push(part);
        match std::fs::symlink_metadata(&current) {
            Ok(m) if m.file_type().is_symlink() => return LocalObservation::SymlinkOrSpecial,
            Ok(m) if index + 1 < parts.len() => {
                if !m.is_dir() {
                    return LocalObservation::SymlinkOrSpecial;
                }
            }
            Ok(m) => {
                return if m.is_dir() {
                    LocalObservation::Directory
                } else if m.is_file() {
                    LocalObservation::UnmanagedFile
                } else {
                    LocalObservation::SymlinkOrSpecial
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return LocalObservation::Absent,
            Err(_) => return LocalObservation::Unknown,
        }
    }
    LocalObservation::Unknown
}

#[cfg(test)]
mod tests;
