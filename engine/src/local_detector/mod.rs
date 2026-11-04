//! Local inventory observations and conservative change reconciliation.
//!
//! This module does not read the filesystem, start a watcher, or persist state.
//! Callers must provide complete inventories established by a trusted scanner.

use std::collections::BTreeMap;

/// Metadata used to distinguish observed filesystem object versions.
///
/// Device and inode identify an observation, not a durable identity: inodes
/// can be reused and hard links can share them. Timestamps use nanoseconds
/// since the Unix epoch, including negative values for earlier timestamps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalMetadata {
    /// Filesystem device of the observed object.
    pub device: u64,
    /// Inode number on that filesystem.
    pub inode: u64,
    /// Raw Unix mode, including file type and permissions.
    pub mode: u32,
    /// File size in bytes.
    pub size: u64,
    /// Modification time in nanoseconds.
    pub modified_ns: i128,
    /// Metadata change time in nanoseconds.
    pub changed_ns: i128,
}

/// The object observed at a path without following symbolic links.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalObject {
    /// A directory which can contain further inventory entries.
    Directory,
    /// A regular file with its SHA-256 content digest.
    File {
        /// Digest of a complete, stable content read.
        sha256: [u8; 32],
    },
    /// A symbolic link; the target is stored as Unix path bytes.
    Symlink {
        /// Unresolved link target; it must never be treated as a path to scan.
        target: Vec<u8>,
    },
    /// Another filesystem object, which must not be opened as a regular file.
    Special,
}

/// A scanner's observation at a single relative path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalEntry {
    /// Object metadata observed without following symbolic links.
    pub metadata: LocalMetadata,
    /// Object kind and relevant content observation.
    pub object: LocalObject,
}

/// A structurally valid inventory whose completeness is established by its caller.
///
/// Paths contain raw Unix bytes separated by `/`; the empty path identifies
/// the root. This type validates structure, not the completeness or stability
/// of a filesystem scan. Callers must not publish incomplete observations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalInventory {
    entries: BTreeMap<Vec<u8>, LocalEntry>,
}

/// A malformed inventory is rejected rather than interpreted as an empty one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidLocalInventory;

impl LocalInventory {
    /// Validates and constructs an inventory without discarding duplicate paths.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidLocalInventory`] for a missing or non-directory root,
    /// duplicate or invalid paths, or entries whose parent is not a directory.
    pub fn new(
        entries: impl IntoIterator<Item = (Vec<u8>, LocalEntry)>,
    ) -> Result<Self, InvalidLocalInventory> {
        let mut inventory = BTreeMap::new();
        for (path, entry) in entries {
            if !valid_path(&path) || inventory.insert(path, entry).is_some() {
                return Err(InvalidLocalInventory);
            }
        }
        if !matches!(
            inventory.get(&[][..]).map(|entry| &entry.object),
            Some(LocalObject::Directory)
        ) {
            return Err(InvalidLocalInventory);
        }
        for path in inventory.keys().filter(|path| !path.is_empty()) {
            let parent = path
                .iter()
                .rposition(|byte| *byte == b'/')
                .map_or(&[][..], |index| &path[..index]);
            if !matches!(
                inventory.get(parent).map(|entry| &entry.object),
                Some(LocalObject::Directory)
            ) {
                return Err(InvalidLocalInventory);
            }
        }
        Ok(Self { entries: inventory })
    }
}

fn valid_path(path: &[u8]) -> bool {
    path.is_empty()
        || (!path.contains(&0)
            && path
                .split(|byte| *byte == b'/')
                .all(|part| !part.is_empty() && part != b"." && part != b".."))
}

/// A baseline observation or an item requiring later re-evaluation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalObservation {
    /// An existing item in the initial inventory, not an inferred creation.
    Initial {
        /// Raw relative path; the empty path identifies the root.
        path: Vec<u8>,
        /// Initial observation.
        current: LocalEntry,
    },
    /// The initial inventory has been fully enumerated.
    InitialComplete,
    /// A path changed between two complete inventories.
    Reevaluate {
        /// Raw relative path; the empty path identifies the root.
        path: Vec<u8>,
        /// Previous observation, absent if the path was not in the inventory.
        previous: Option<LocalEntry>,
        /// Current observation, absent if the path is no longer in the inventory.
        current: Option<LocalEntry>,
    },
}

/// Compares structurally valid, complete inventories without deciding actions.
///
/// A missing previous inventory yields baseline observations and a completion
/// marker. A changed path yields one re-evaluation; renames and deletions are
/// not inferred. This pure comparison provides neither scanning completeness
/// nor durable event delivery; those remain the caller's responsibilities.
pub fn reconcile_local_inventories(
    previous: Option<&LocalInventory>,
    current: &LocalInventory,
) -> Vec<LocalObservation> {
    let Some(previous) = previous else {
        let mut events: Vec<_> = current
            .entries
            .iter()
            .map(|(path, entry)| LocalObservation::Initial {
                path: path.clone(),
                current: entry.clone(),
            })
            .collect();
        events.push(LocalObservation::InitialComplete);
        return events;
    };
    let mut entries = previous.entries.iter().peekable();
    let mut next = current.entries.iter().peekable();
    let mut events = Vec::new();
    while entries.peek().is_some() || next.peek().is_some() {
        let (path, before, after) = match (entries.peek(), next.peek()) {
            (Some((left, _)), Some((right, _))) if left == right => {
                let (path, before) = entries.next().expect("peeked entry");
                let (_, after) = next.next().expect("peeked entry");
                (path, Some(before), Some(after))
            }
            (Some((left, _)), Some((right, _))) if left < right => {
                let (path, before) = entries.next().expect("peeked entry");
                (path, Some(before), None)
            }
            (Some(_), None) => {
                let (path, before) = entries.next().expect("peeked entry");
                (path, Some(before), None)
            }
            _ => {
                let (path, after) = next.next().expect("peeked entry");
                (path, None, Some(after))
            }
        };
        if before != after {
            events.push(LocalObservation::Reevaluate {
                path: path.clone(),
                previous: before.cloned(),
                current: after.cloned(),
            });
        }
    }
    events
}

#[cfg(test)]
mod tests;
