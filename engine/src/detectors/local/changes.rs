//! Bounded native evidence correlation; events describe observations, not sync commands.
use crate::models::{FileEventKind, LocalRelativePath};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    time::{Duration, Instant},
};
#[derive(Clone)]
pub(super) struct Change {
    pub kind: FileEventKind,
    pub previous: Option<LocalRelativePath>,
}
#[derive(Clone)]
pub(super) struct Hint {
    pub path: LocalRelativePath,
    pub mask: u32,
    pub cookie: u32,
}
pub(super) struct Changes {
    known: BTreeSet<LocalRelativePath>,
    complete: bool,
    pub pending: BTreeMap<LocalRelativePath, Change>,
    moves: HashMap<u32, (LocalRelativePath, Instant)>,
}
impl Changes {
    pub fn new(known: BTreeSet<LocalRelativePath>, complete: bool) -> Self {
        Self {
            known,
            complete,
            pending: BTreeMap::new(),
            moves: HashMap::new(),
        }
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.moves.values().map(|(_, at)| *at).min()
    }
    pub fn expire(&mut self, now: Instant) {
        let expired: Vec<_> = self
            .moves
            .iter()
            .filter(|(_, (_, at))| *at <= now)
            .map(|(cookie, _)| *cookie)
            .collect();
        for cookie in expired {
            let (path, _) = self.moves.remove(&cookie).unwrap();
            self.pending.insert(
                path,
                Change {
                    kind: FileEventKind::RecheckRequired,
                    previous: None,
                },
            );
        }
    }
    pub fn push(&mut self, hint: Hint) {
        let path = hint.path;
        if hint.mask & libc::IN_MOVED_FROM != 0 {
            self.known.remove(&path);
            self.pending.remove(&path);
            if hint.cookie != 0 {
                self.moves.insert(
                    hint.cookie,
                    (path, Instant::now() + Duration::from_millis(100)),
                );
            } else {
                self.pending.insert(
                    path,
                    Change {
                        kind: FileEventKind::RecheckRequired,
                        previous: None,
                    },
                );
            }
            return;
        }
        let change = if hint.mask & libc::IN_MOVED_TO != 0 {
            let previous = self.moves.remove(&hint.cookie).map(|(p, _)| p);
            if self.known.contains(&path) {
                Change {
                    kind: FileEventKind::Modified,
                    previous: None,
                }
            } else if let Some(previous) = previous.filter(|_| self.complete) {
                let parent = |p: &LocalRelativePath| {
                    p.as_bytes()
                        .iter()
                        .rposition(|b| *b == b'/')
                        .map(|i| &p.as_bytes()[..i])
                        .unwrap_or(b"")
                        .to_vec()
                };
                let kind = if parent(&path) == parent(&previous) {
                    FileEventKind::Renamed
                } else {
                    FileEventKind::Moved
                };
                Change {
                    kind,
                    previous: Some(previous),
                }
            } else {
                Change {
                    kind: FileEventKind::RecheckRequired,
                    previous: None,
                }
            }
        } else if hint.mask & libc::IN_DELETE != 0 {
            self.known.remove(&path);
            self.pending.insert(
                path.clone(),
                Change {
                    kind: FileEventKind::Deleted,
                    previous: Some(path),
                },
            );
            return;
        } else if hint.mask & libc::IN_CREATE != 0 {
            Change {
                kind: FileEventKind::Created,
                previous: None,
            }
        } else if hint.mask & (libc::IN_MODIFY | libc::IN_CLOSE_WRITE) != 0 {
            Change {
                kind: FileEventKind::Modified,
                previous: None,
            }
        } else if hint.mask & libc::IN_ATTRIB != 0 {
            Change {
                kind: FileEventKind::MetadataChanged,
                previous: None,
            }
        } else {
            Change {
                kind: FileEventKind::RecheckRequired,
                previous: None,
            }
        };
        self.known.insert(path.clone());
        // Keep creation/move context while merging subsequent writes in this round.
        match self.pending.get(&path) {
            Some(old)
                if matches!(
                    old.kind,
                    FileEventKind::Created | FileEventKind::Renamed | FileEventKind::Moved
                ) && matches!(
                    change.kind,
                    FileEventKind::Modified | FileEventKind::MetadataChanged
                ) => {}
            _ => {
                self.pending.insert(path, change);
            }
        }
    }
    pub fn over_budget(&self, limit: usize) -> bool {
        self.pending.len().saturating_add(self.moves.len()) > limit
            || self.known.len() > 100_000
            || self
                .known
                .iter()
                .chain(self.pending.keys())
                .chain(self.moves.values().map(|(p, _)| p))
                .map(|p| p.as_bytes().len())
                .sum::<usize>()
                > 8 * 1024 * 1024
    }
    pub fn replace_known(&mut self, known: BTreeSet<LocalRelativePath>, complete: bool) {
        self.complete = complete;
        self.known = known;
    }
    pub fn reset(&mut self, known: BTreeSet<LocalRelativePath>) {
        self.known = known;
        self.complete = false;
        self.pending.clear();
        self.moves.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path(value: &[u8]) -> LocalRelativePath {
        LocalRelativePath::new(value.to_vec()).unwrap()
    }
    fn hint(value: &[u8], mask: u32, cookie: u32) -> Hint {
        Hint {
            path: path(value),
            mask,
            cookie,
        }
    }
    #[test]
    fn correlates_move_halves_across_reads_and_expires_unmatched_moves_conservatively() {
        let mut changes = Changes::new(BTreeSet::from([path(b"before")]), true);
        changes.push(hint(b"before", libc::IN_MOVED_FROM, 51));
        assert!(changes.pending.is_empty());
        changes.push(hint(b"after", libc::IN_MOVED_TO, 51));
        let change = &changes.pending[&path(b"after")];
        assert_eq!(change.kind, FileEventKind::Renamed);
        assert_eq!(change.previous, Some(path(b"before")));
        changes.pending.clear();
        changes.push(hint(b"after", libc::IN_MOVED_FROM, 52));
        changes.expire(changes.deadline().unwrap());
        assert_eq!(
            changes.pending[&path(b"after")].kind,
            FileEventKind::RecheckRequired
        );
        changes.push(hint(b"incoming", libc::IN_MOVED_TO, 99));
        assert_eq!(
            changes.pending[&path(b"incoming")].kind,
            FileEventKind::RecheckRequired
        );
    }
    #[test]
    fn atomic_save_coalesces_temporary_hints_and_keeps_target_identity_context() {
        let mut changes = Changes::new(BTreeSet::from([path(b"document")]), true);
        changes.push(hint(b"temporary", libc::IN_CREATE, 0));
        changes.push(hint(b"temporary", libc::IN_MODIFY, 0));
        changes.push(hint(b"temporary", libc::IN_MOVED_FROM, 123));
        changes.push(hint(b"document", libc::IN_MOVED_TO, 123));
        assert_eq!(changes.pending.len(), 1);
        assert_eq!(
            changes.pending[&path(b"document")].kind,
            FileEventKind::Modified
        );
        assert_eq!(changes.pending[&path(b"document")].previous, None);
    }
    #[test]
    fn incomplete_catalog_does_not_claim_rename_over_unknown_destination() {
        let mut changes = Changes::new(BTreeSet::new(), false);
        changes.push(hint(b"source", libc::IN_MOVED_FROM, 1));
        changes.push(hint(b"target", libc::IN_MOVED_TO, 1));
        assert_eq!(
            changes.pending[&path(b"target")].kind,
            FileEventKind::RecheckRequired
        );
        changes.push(hint(b"other", libc::IN_CREATE, 0));
        assert!(changes.over_budget(1));
    }
}
