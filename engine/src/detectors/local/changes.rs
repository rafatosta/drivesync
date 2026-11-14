//! Bounded, evidence-based write-cycle and atomic-replacement normalization.
use crate::models::{FileEventKind, LocalRelativePath, ObservationUncertainty};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    time::{Duration, Instant},
};
#[derive(Clone)]
pub(super) struct Change {
    pub kind: FileEventKind,
    pub previous: Option<LocalRelativePath>,
    pub uncertainty: Option<ObservationUncertainty>,
    writing: bool,
    closed: bool,
}
impl Change {
    fn new(kind: FileEventKind) -> Self {
        Self {
            kind,
            previous: None,
            uncertainty: (kind == FileEventKind::RecheckRequired)
                .then_some(ObservationUncertainty::UnclassifiedSignal),
            writing: false,
            closed: false,
        }
    }
    fn uncertain(reason: ObservationUncertainty) -> Self {
        Self {
            uncertainty: Some(reason),
            ..Self::new(FileEventKind::RecheckRequired)
        }
    }
}
#[derive(Clone)]
pub(super) struct Hint {
    pub path: LocalRelativePath,
    pub mask: u32,
    pub cookie: u32,
}
struct Move {
    source: LocalRelativePath,
    expires: Instant,
    change: Option<Change>,
    fresh: bool,
}
pub(super) struct Changes {
    known: BTreeSet<LocalRelativePath>,
    complete: bool,
    pending: BTreeMap<LocalRelativePath, Change>,
    ready: VecDeque<(LocalRelativePath, Change)>,
    open: BTreeMap<LocalRelativePath, Change>,
    moves: HashMap<u32, Move>,
    fresh: BTreeSet<LocalRelativePath>,
    retired: HashMap<LocalRelativePath, LocalRelativePath>,
}
impl Changes {
    pub fn new(known: BTreeSet<LocalRelativePath>, complete: bool) -> Self {
        Self {
            known,
            complete,
            pending: BTreeMap::new(),
            ready: VecDeque::new(),
            open: BTreeMap::new(),
            moves: HashMap::new(),
            fresh: BTreeSet::new(),
            retired: HashMap::new(),
        }
    }
    /// Explicit observation never overwrites or reclassifies an existing native occurrence.
    pub fn request_recheck(&mut self, path: LocalRelativePath) {
        self.ready.push_back((
            path,
            Change::uncertain(ObservationUncertainty::CoverageIncomplete),
        ));
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.moves.values().map(|entry| entry.expires).min()
    }
    pub fn has_open_write(&self) -> bool {
        self.pending
            .values()
            .chain(self.ready.iter().map(|(_, change)| change))
            .any(|change| change.writing)
    }
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty() || !self.ready.is_empty()
    }
    pub fn expire(&mut self, now: Instant) {
        let expired: Vec<_> = self
            .moves
            .iter()
            .filter(|(_, entry)| entry.expires <= now)
            .map(|(cookie, _)| *cookie)
            .collect();
        for cookie in expired {
            let entry = self.moves.remove(&cookie).unwrap();
            self.ready.push_back((
                entry.source,
                Change::uncertain(ObservationUncertainty::UnpairedMove),
            ));
        }
    }
    fn seal(&mut self, path: &LocalRelativePath) {
        if let Some(change) = self.pending.remove(path) {
            self.ready.push_back((path.clone(), change));
        }
    }
    /// Called only after the bounded settling window. Each close/write cycle stays distinct.
    pub fn pop(&mut self) -> Option<(LocalRelativePath, Change)> {
        let (path, mut change) = self
            .ready
            .pop_front()
            .or_else(|| self.pending.pop_first())?;
        if change.writing {
            self.open.insert(path.clone(), change);
            change = Change::uncertain(ObservationUncertainty::WriteInProgress);
        } else {
            self.fresh.remove(&path);
        }
        if !self.has_pending() && self.open.is_empty() && self.moves.is_empty() {
            self.retired.clear();
        }
        Some((path, change))
    }
    pub fn push(&mut self, hint: Hint) {
        use FileEventKind::*;
        let mut path = hint.path;
        let mask = hint.mask;
        let mut routed = false;
        if mask & (libc::IN_CREATE | libc::IN_MOVED_TO) != 0 {
            self.retired.remove(&path);
        }
        if mask & (libc::IN_MODIFY | libc::IN_CLOSE_WRITE | libc::IN_ATTRIB) != 0 {
            // A cookie-proven relocation can leave late notifications on the old name.
            for _ in 0..16 {
                let Some(target) = self.retired.get(&path) else {
                    break;
                };
                path = target.clone();
                routed = true;
            }
        }
        if !self.known.contains(&path)
            && mask & (libc::IN_MODIFY | libc::IN_CLOSE_WRITE | libc::IN_ATTRIB) != 0
        {
            if let Some(entry) = self.moves.values_mut().find(|entry| entry.source == path) {
                let change = entry.change.get_or_insert_with(|| Change::new(Modified));
                if mask & libc::IN_MODIFY != 0 {
                    change.writing = true;
                }
                if mask & libc::IN_CLOSE_WRITE != 0 {
                    change.writing = false;
                    change.closed = true;
                }
                return;
            }
        }
        if mask & libc::IN_MOVED_FROM != 0 {
            self.known.remove(&path);
            let change = self
                .pending
                .remove(&path)
                .or_else(|| self.open.remove(&path));
            let fresh = self.fresh.remove(&path);
            if fresh {
                self.ready.retain(|(source, _)| source != &path);
            }
            if hint.cookie != 0 {
                self.moves.insert(
                    hint.cookie,
                    Move {
                        source: path,
                        expires: Instant::now() + Duration::from_millis(100),
                        change,
                        fresh,
                    },
                );
            } else {
                self.ready.push_back((
                    path,
                    Change::uncertain(ObservationUncertainty::UnpairedMove),
                ));
            }
            return;
        }
        if mask & libc::IN_MOVED_TO != 0 {
            let source = self.moves.remove(&hint.cookie);
            let target_exists = self.known.contains(&path);
            let mut change = match source {
                Some(mut source) => {
                    let kind = if source.fresh {
                        if target_exists {
                            Modified
                        } else {
                            Created
                        }
                    } else if self.complete {
                        if parent(&path) == parent(&source.source) {
                            Renamed
                        } else {
                            Moved
                        }
                    } else {
                        RecheckRequired
                    };
                    let mut change = Change::new(kind);
                    if matches!(kind, Renamed | Moved) {
                        change.previous = Some(source.source.clone());
                    }
                    if kind == RecheckRequired {
                        change.uncertainty = Some(ObservationUncertainty::CoverageIncomplete);
                    }
                    if let Some(prior) = source.change.take() {
                        if source.fresh {
                            change.writing = prior.writing;
                            change.closed = prior.closed;
                        } else if prior.writing {
                            change = Change::uncertain(ObservationUncertainty::ConflictingSequence);
                            change.previous = Some(source.source.clone());
                            change.writing = true;
                        } else {
                            self.ready.push_back((source.source.clone(), prior));
                        }
                    }
                    self.retired.insert(source.source, path.clone());
                    if kind == Created {
                        self.fresh.insert(path.clone());
                    }
                    change
                }
                None => Change::uncertain(ObservationUncertainty::UnpairedMove),
            };
            if self.open.remove(&path).is_some() {
                change = Change::uncertain(ObservationUncertainty::ConflictingSequence);
            }
            if let Some(old) = self.pending.get(&path) {
                if old.closed {
                    self.seal(&path);
                } else if old.writing {
                    change = Change::uncertain(ObservationUncertainty::ConflictingSequence);
                }
            }
            self.known.insert(path.clone());
            self.pending.insert(path, change);
            return;
        }
        if mask & libc::IN_DELETE != 0 {
            self.seal(&path);
            self.open.remove(&path);
            self.fresh.remove(&path);
            self.known.remove(&path);
            let mut change = Change::new(Deleted);
            change.previous = Some(path.clone());
            self.pending.insert(path, change);
            return;
        }
        if mask & libc::IN_CREATE != 0 {
            self.seal(&path);
            self.fresh.insert(path.clone());
            self.known.insert(path.clone());
            self.pending.insert(path, Change::new(Created));
            return;
        }
        if mask & libc::IN_CLOSE_WRITE != 0 {
            if let Some(change) = self.open.remove(&path) {
                self.pending.entry(path.clone()).or_insert(change);
            }
            if let Some(change) = self.pending.get_mut(&path) {
                change.writing = false;
                change.closed = true;
            }
            return;
        }
        if mask & libc::IN_MODIFY != 0 {
            if let Some(change) = self.open.remove(&path) {
                self.pending.entry(path.clone()).or_insert(change);
            }
            if self.pending.get(&path).is_some_and(|old| {
                (!routed && old.closed)
                    || matches!(old.kind, Deleted | MetadataChanged)
                    || (!routed && matches!(old.kind, Renamed | Moved) && !old.writing)
            }) {
                self.seal(&path);
            }
            let change = self
                .pending
                .entry(path.clone())
                .or_insert_with(|| Change::new(Modified));
            change.writing = true;
            change.closed = false;
            self.known.insert(path);
            return;
        }
        if mask & libc::IN_ATTRIB != 0 {
            if self.pending.get(&path).is_some_and(|old| old.closed) {
                self.seal(&path);
            }
            self.pending
                .entry(path.clone())
                .or_insert_with(|| Change::new(MetadataChanged));
            self.known.insert(path);
            return;
        }
        self.pending.insert(
            path,
            Change::uncertain(ObservationUncertainty::UnclassifiedSignal),
        );
    }
    pub fn over_budget(&self, limit: usize) -> bool {
        self.pending
            .len()
            .saturating_add(self.ready.len())
            .saturating_add(self.open.len())
            .saturating_add(self.moves.len())
            .saturating_add(self.retired.len())
            > limit
            || self.known.len() > 100_000
            || self
                .known
                .iter()
                .chain(self.pending.keys())
                .chain(self.ready.iter().map(|(p, _)| p))
                .chain(self.open.keys())
                .chain(self.moves.values().map(|entry| &entry.source))
                .chain(self.fresh.iter())
                .chain(self.retired.keys())
                .chain(self.retired.values())
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
        self.ready.clear();
        self.open.clear();
        self.moves.clear();
        self.fresh.clear();
        self.retired.clear();
    }
}
fn parent(path: &LocalRelativePath) -> &[u8] {
    path.as_bytes()
        .iter()
        .rposition(|b| *b == b'/')
        .map(|i| &path.as_bytes()[..i])
        .unwrap_or(b"")
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
            changes.pop().unwrap().1.kind,
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
    #[test]
    fn completed_write_cycles_and_delete_recreate_remain_distinct() {
        let mut changes = Changes::new(BTreeSet::from([path(b"file")]), true);
        for _ in 0..2 {
            changes.push(hint(b"file", libc::IN_MODIFY, 0));
            changes.push(hint(b"file", libc::IN_CLOSE_WRITE, 0));
        }
        assert_eq!(changes.pop().unwrap().1.kind, FileEventKind::Modified);
        assert_eq!(changes.pop().unwrap().1.kind, FileEventKind::Modified);
        assert!(changes.pop().is_none());
        changes.push(hint(b"file", libc::IN_DELETE, 0));
        changes.push(hint(b"file", libc::IN_CREATE, 0));
        assert_eq!(changes.pop().unwrap().1.kind, FileEventKind::Deleted);
        assert_eq!(changes.pop().unwrap().1.kind, FileEventKind::Created);
    }
    #[test]
    fn late_source_writes_follow_cookie_proven_replacement_and_new_creates_reset_alias() {
        let mut changes = Changes::new(BTreeSet::from([path(b"target")]), true);
        changes.push(hint(b"temp", libc::IN_CREATE, 0));
        changes.push(hint(b"temp", libc::IN_MODIFY, 0));
        changes.push(hint(b"temp", libc::IN_MOVED_FROM, 7));
        changes.push(hint(b"temp", libc::IN_CLOSE_WRITE, 0));
        changes.push(hint(b"target", libc::IN_MOVED_TO, 7));
        let (target, change) = changes.pop().unwrap();
        assert_eq!(target, path(b"target"));
        assert_eq!(change.kind, FileEventKind::Modified);
        assert!(changes.pop().is_none());
        changes.push(hint(b"temp", libc::IN_CREATE, 0));
        assert_eq!(changes.pop().unwrap().0, path(b"temp"));
    }
    #[test]
    fn existing_source_overwrite_is_a_move_not_a_provisional_save() {
        let mut changes = Changes::new(BTreeSet::from([path(b"old"), path(b"target")]), true);
        changes.push(hint(b"old", libc::IN_MOVED_FROM, 9));
        changes.push(hint(b"target", libc::IN_MOVED_TO, 9));
        let change = changes.pop().unwrap().1;
        assert_eq!(change.kind, FileEventKind::Renamed);
        assert_eq!(change.previous, Some(path(b"old")));
    }
    #[test]
    fn late_retired_name_notifications_belong_to_the_proven_replacement() {
        let mut changes = Changes::new(BTreeSet::from([path(b"target")]), true);
        for mask in [libc::IN_CREATE, libc::IN_MODIFY, libc::IN_CLOSE_WRITE] {
            changes.push(hint(b"temp", mask, 0));
        }
        changes.push(hint(b"temp", libc::IN_MOVED_FROM, 77));
        changes.push(hint(b"target", libc::IN_MOVED_TO, 77));
        changes.push(hint(b"temp", libc::IN_MODIFY, 0));
        changes.push(hint(b"temp", libc::IN_CLOSE_WRITE, 0));
        assert_eq!(changes.pop().unwrap().0, path(b"target"));
        assert!(changes.pop().is_none());
    }
    #[test]
    fn temporary_looking_names_are_not_filtered_without_replacement_evidence() {
        let mut changes = Changes::new(BTreeSet::new(), true);
        for mask in [libc::IN_CREATE, libc::IN_MODIFY, libc::IN_CLOSE_WRITE] {
            changes.push(hint(b".goutputstream-real", mask, 0));
        }
        let (name, event) = changes.pop().unwrap();
        assert_eq!(name, path(b".goutputstream-real"));
        assert_eq!(event.kind, FileEventKind::Created);
    }
    #[test]
    fn moving_an_existing_resource_during_an_open_write_is_explicitly_uncertain() {
        let mut changes = Changes::new(BTreeSet::from([path(b"old")]), true);
        changes.push(hint(b"old", libc::IN_MODIFY, 0));
        changes.push(hint(b"old", libc::IN_MOVED_FROM, 11));
        changes.push(hint(b"new", libc::IN_MOVED_TO, 11));
        changes.push(hint(b"new", libc::IN_CLOSE_WRITE, 0));
        let (name, event) = changes.pop().unwrap();
        assert_eq!(name, path(b"new"));
        assert_eq!(
            event.uncertainty,
            Some(ObservationUncertainty::ConflictingSequence)
        );
        assert!(changes.pop().is_none());
        assert!(changes.open.is_empty());
    }
}
