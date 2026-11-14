//! Bounded FIFO of native observations; no history, pairing or action inference.
use crate::models::LocalRelativePath;
use std::collections::VecDeque;

/// One notification supplied by the kernel, preserving its path, mask and cookie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Hint {
    pub path: LocalRelativePath,
    pub mask: u32,
    pub cookie: u32,
}
/// A native observation or an explicit request to collect current metadata.
pub(super) enum Trigger {
    Native(Hint),
    Recheck(LocalRelativePath),
}
impl Trigger {
    fn path(&self) -> &LocalRelativePath {
        match self {
            Self::Native(hint) => &hint.path,
            Self::Recheck(path) => path,
        }
    }
}
#[derive(Default)]
pub(super) struct Signals {
    pending: VecDeque<Trigger>,
    path_bytes: usize,
}
impl Signals {
    pub fn request_recheck(&mut self, path: LocalRelativePath) {
        self.path_bytes = self.path_bytes.saturating_add(path.as_bytes().len());
        self.pending.push_back(Trigger::Recheck(path));
    }
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
    pub fn pop(&mut self) -> Option<Trigger> {
        let trigger = self.pending.pop_front()?;
        self.path_bytes -= trigger.path().as_bytes().len();
        Some(trigger)
    }
    pub fn push(&mut self, hint: Hint) {
        self.path_bytes = self.path_bytes.saturating_add(hint.path.as_bytes().len());
        self.pending.push_back(Trigger::Native(hint));
    }
    pub fn over_budget(&self, limit: usize) -> bool {
        self.pending.len() > limit || self.path_bytes > 8 * 1024 * 1024
    }
    /// Loss of volatile hints must be accompanied by a coverage warning in the runtime.
    pub fn reset(&mut self) {
        self.pending.clear();
        self.path_bytes = 0;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn hint(path: &[u8], mask: u32, cookie: u32) -> Hint {
        Hint {
            path: LocalRelativePath::new(path.to_vec()).unwrap(),
            mask,
            cookie,
        }
    }
    #[test]
    fn fifo_preserves_duplicates_temporary_paths_and_unpaired_move_halves() {
        let inputs = [
            hint(b"temporary", libc::IN_CREATE, 0),
            hint(b"temporary", libc::IN_MODIFY, 0),
            hint(b"temporary", libc::IN_MODIFY, 0),
            hint(b"temporary", libc::IN_MOVED_FROM, 7),
            hint(b"document", libc::IN_MOVED_TO, 7),
        ];
        let mut signals = Signals::default();
        for item in &inputs {
            signals.push(item.clone());
        }
        for expected in inputs {
            let Some(Trigger::Native(actual)) = signals.pop() else {
                panic!()
            };
            assert_eq!(actual, expected);
        }
        assert!(!signals.has_pending());
        assert_eq!(signals.path_bytes, 0);
    }
    #[test]
    fn explicit_rechecks_do_not_replace_native_evidence_and_limits_are_bounded() {
        let mut signals = Signals::default();
        let native = hint(b"file", libc::IN_DELETE, 0);
        signals.push(native.clone());
        signals.request_recheck(native.path.clone());
        assert!(signals.over_budget(1));
        assert!(matches!(signals.pop(), Some(Trigger::Native(_))));
        assert!(matches!(signals.pop(), Some(Trigger::Recheck(_))));
        assert!(!signals.over_budget(1));
        signals.push(native);
        signals.reset();
        assert!(!signals.has_pending());
        assert_eq!(signals.path_bytes, 0);
    }
}
