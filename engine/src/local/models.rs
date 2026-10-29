//! Local evidence is scoped to one completed scan, not a synchronization base.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

/// Change detector only; timestamps never establish version precedence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Stamp {
    pub device: u64,
    pub inode: u64,
    pub size: u64,
    pub mtime: (i64, i64),
    pub ctime: (i64, i64),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Evidence {
    Directory,
    Stable { fingerprint: String, stamp: Stamp },
    Temporary,
    Writing,
    Unsupported,
    Unknown,
}
/// Only a successfully completed traversal can construct this complete inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Inventory {
    pub entries: BTreeMap<PathBuf, Evidence>,
}
impl Inventory {
    /// Missing descendants of inaccessible/special ancestors remain unknown.
    pub(crate) fn observe(&self, path: &std::path::Path) -> crate::LocalObservation {
        use crate::LocalObservation as L;
        match self.entries.get(path) {
            Some(Evidence::Directory) => L::Directory,
            Some(Evidence::Stable { fingerprint, .. }) => L::Verified(fingerprint.clone()),
            Some(Evidence::Unsupported) => L::SymlinkOrSpecial,
            Some(Evidence::Temporary | Evidence::Writing | Evidence::Unknown) => L::Unknown,
            None => {
                if path.ancestors().skip(1).any(|parent| {
                    self.entries
                        .get(parent)
                        .is_some_and(|e| *e != Evidence::Directory)
                        || super::ignored(parent)
                }) {
                    L::Unknown
                } else {
                    L::Absent
                }
            }
        }
    }
}
