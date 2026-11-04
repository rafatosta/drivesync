//! Session scope, activity hints and explicit coverage limitations.

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

/// Immutable identity of one observation session; it does not identify a Drive item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalScope {
    client: String,
    account: String,
    root: PathBuf,
}

impl LocalScope {
    /// Validates identifiers and a normalized absolute root other than `/`.
    /// This does not authorize access or open the directory.
    pub fn new(client: &str, account: &str, root: &Path) -> Result<Self, LocalWatchError> {
        use std::os::unix::ffi::OsStrExt;
        let bytes = root.as_os_str().as_bytes();
        if client.is_empty()
            || account.is_empty()
            || client.chars().any(char::is_control)
            || account.chars().any(char::is_control)
            || !root.is_absolute()
            || bytes.contains(&0)
            || bytes.len() <= 1
            || bytes[1..]
                .split(|b| *b == b'/')
                .any(|p| p.is_empty() || p == b"." || p == b"..")
            || root
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(LocalWatchError::InvalidInput);
        }
        Ok(Self {
            client: client.into(),
            account: account.into(),
            root: root.into(),
        })
    }
    /// OAuth client identifier supplied by the caller.
    pub fn client(&self) -> &str {
        &self.client
    }
    /// Account identifier supplied by the caller.
    pub fn account(&self) -> &str {
        &self.account
    }
    /// Byte-preserving root path supplied by the caller.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Finite resource limits. Defaults suit an interactive diagnostic, not every tree.
#[derive(Clone, Debug)]
pub struct LocalWatchOptions {
    /// Maximum directory watches and separately retained removed-watch IDs, including the root.
    pub max_watches: usize,
    /// Maximum queued application notices, excluding one reserved loss marker.
    pub max_queued: usize,
    /// Maximum outstanding move cookies.
    pub max_pending_moves: usize,
    /// Maximum 4096-byte kernel reads in each poll.
    pub max_reads_per_poll: usize,
    /// Maximum directory entries examined in one subtree installation.
    pub max_scan_entries: usize,
    /// Time to wait for the other half of a move; expiration occurs on poll.
    pub move_timeout: Duration,
}
impl Default for LocalWatchOptions {
    fn default() -> Self {
        Self {
            max_watches: 8192,
            max_queued: 4096,
            max_pending_moves: 1024,
            max_reads_per_poll: 4,
            max_scan_entries: 100_000,
            move_timeout: Duration::from_millis(100),
        }
    }
}
impl LocalWatchOptions {
    pub(super) fn valid(&self) -> bool {
        self.max_watches > 0
            && self.max_queued > 0
            && self.max_pending_moves > 0
            && self.max_reads_per_poll > 0
            && self.max_scan_entries > 0
            && !self.move_timeout.is_zero()
    }
}

/// Coverage of this session. Incomplete never automatically returns to observing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum LocalWatchState {
    /// Installing directory watches.
    Initializing,
    /// Watches are active; historical state and notification blind spots remain unknown.
    Observing,
    /// At least one known coverage gap requires investigation.
    Incomplete,
    /// Root identity or the event source was lost; start a new session explicitly.
    Stopped,
}

/// Reasons to investigate state rather than infer operations from missing notices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum LocalCoverageGap {
    /// No historical events or baseline are available, including after restart.
    Startup,
    /// Descendants may have changed before watches were installed.
    SubtreeAttached,
    /// Kernel event queue overflowed.
    KernelOverflow,
    /// Application queue overflowed; one reserved loss notice is retained.
    ApplicationOverflow,
    /// A configured resource limit was reached.
    ResourceLimit,
    /// A watch disappeared unexpectedly.
    WatchLost,
    /// A directory could not be read or watched, including nested mount boundaries.
    Inaccessible,
    /// A watched path no longer identifies the expected directory.
    PathChanged,
    /// Root was moved, replaced, removed or became inaccessible.
    RootChanged,
    /// Events could not safely be assigned while a directory move was pending.
    MoveUncertain,
    /// Filesystem notification behavior is not established (including document-portal FUSE).
    FilesystemUnverified,
    /// Reading the event source failed.
    SourceFailed,
}

/// Observed activity, never an instruction to synchronize or proof of identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalActivity {
    Created,
    Removed,
    Written,
    ClosedAfterWrite,
    MetadataChanged,
    /// Entered the scope without an observed source.
    MovedIn,
    /// Left a watched directory without a matched destination in time.
    MovedOut,
    /// Kernel cookie correlated both names; does not establish durable identity.
    Moved {
        from: PathBuf,
    },
    /// State must be investigated for the given path/subtree.
    Recheck(LocalCoverageGap),
}

/// Activity at a relative historical path. Empty path denotes the root.
/// Non-directory subjects have unknown type and must not be opened blindly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalNotice {
    pub path: PathBuf,
    pub is_directory: Option<bool>,
    pub activity: LocalActivity,
}

/// One volatile transfer of notices, in processing order, for exactly one scope.
#[derive(Clone, Debug)]
pub struct LocalBatch {
    pub scope: LocalScope,
    pub state: LocalWatchState,
    pub notices: Vec<LocalNotice>,
}

/// Startup errors do not expose filesystem paths or account data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalWatchError {
    InvalidInput,
    Unavailable,
}
impl std::fmt::Display for LocalWatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "Invalid local observation scope or limits.",
            Self::Unavailable => "Local observation could not be started safely.",
        })
    }
}
impl std::error::Error for LocalWatchError {}
