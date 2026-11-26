//! Provider-neutral records exposed by the Operations facade.

pub(in crate::orchestrator) use crate::activity::{ActivityId, Scope};
use crate::{
    inventory::models::{RecoveryIssue, WriteStatus},
    models::{Id, ItemId, OperationAction},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::orchestrator) enum Kind {
    Execute(OperationAction),
    Recover,
    RecoverItem(OperationAction),
    RecordConflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::orchestrator) enum Direction {
    LocalToRemote,
    RemoteToLocal,
}

/// A display label is an opaque activity ID, never a path, name or remote identifier.
#[derive(Debug)]
pub(in crate::orchestrator) struct Execution {
    pub(in crate::orchestrator) scope: Arc<Scope>,
    pub(in crate::orchestrator) id: Option<Id>,
    pub(in crate::orchestrator) item: Option<ItemId>,
    pub(in crate::orchestrator) display_item: Option<ActivityId>,
    pub(in crate::orchestrator) kind: Kind,
    pub(in crate::orchestrator) direction: Option<Direction>,
}

impl Kind {
    pub(super) fn direction(self) -> Option<Direction> {
        use OperationAction::*;
        match self {
            Self::Execute(
                Upload
                | UpdateRemoteContent
                | CreateRemoteDirectory
                | RenameRemote
                | MoveRemote
                | TrashRemote,
            )
            | Self::RecoverItem(
                Upload
                | UpdateRemoteContent
                | CreateRemoteDirectory
                | RenameRemote
                | MoveRemote
                | TrashRemote,
            ) => Some(Direction::LocalToRemote),
            Self::Execute(
                Download | UpdateLocalContent | CreateLocalDirectory | CreateLocalShortcut
                | RenameLocal | MoveLocal | RemoveLocal,
            )
            | Self::RecoverItem(
                Download | UpdateLocalContent | CreateLocalDirectory | CreateLocalShortcut
                | RenameLocal | MoveLocal | RemoveLocal,
            ) => Some(Direction::RemoteToLocal),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::orchestrator) enum Phase {
    Receive,
    Acquire,
    Validate,
    Prepare,
    RecordIntent,
    Dispatch,
    VerifyEffect,
    PersistEffect,
    Confirm,
    ReadRecovery,
    Reconcile,
    Resume,
    Checkpoint,
    CheckpointTransfer,
    ReadLocal,
    WriteLocal,
    CheckIdentity,
    PrepareLocal,
    CopyLocal,
    CreateDirectory,
    PublishLocal,
    ReplaceLocal,
    MoveLocal,
    Checksum,
    VerifyContent,
    VerifyLocalScope,
    AcquireSession,
    ReadMetadata,
    ValidateVersion,
    CheckDestination,
    ResolveAncestors,
    PrepareUpload,
    Upload,
    DownloadHeaders,
    Download,
    ArchiveConflict,
    RecordConflict,
    VerifyArchive,
    Trash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::orchestrator) enum Wait {
    OperatorGate,
    Inventory,
    LocalIo,
    Authorization,
    Provider,
    Response,
    Retry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::orchestrator) enum Effect {
    NotApplied,
    Applied,
    Verified,
    Uncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::orchestrator) enum Outcome {
    Complete,
    Failed,
    Skipped,
    Cancelled,
    Unconfirmed,
    Inconclusive(RecoveryIssue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::orchestrator) enum State {
    Running,
    Waiting(Wait),
    Finished(Outcome),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::orchestrator) enum Algorithm {
    Sha256,
    Md5AndSha256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::orchestrator) enum Unit {
    Requests,
    Pages,
    Items,
    Ancestors,
    RecoveryRecords,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::orchestrator) struct Bytes {
    pub(in crate::orchestrator) total: Option<u64>,
    pub(in crate::orchestrator) read: Option<u64>,
    pub(in crate::orchestrator) received: Option<u64>,
    pub(in crate::orchestrator) written: Option<u64>,
    pub(in crate::orchestrator) submitted: Option<u64>,
    pub(in crate::orchestrator) submitted_this_attempt: Option<u64>,
    pub(in crate::orchestrator) confirmed: Option<u64>,
    pub(in crate::orchestrator) hashed: Option<u64>,
    pub(in crate::orchestrator) algorithm: Option<Algorithm>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::orchestrator) enum Progress {
    Bytes(Bytes),
    Count {
        unit: Unit,
        completed: u64,
        total: Option<u64>,
    },
    Page {
        requests: u64,
        pages: u64,
        items: u64,
    },
}

impl Progress {
    pub(super) fn rate(self, started: Instant, updated: Instant) -> Option<u64> {
        let Self::Bytes(bytes) = self else {
            return None;
        };
        let bytes = bytes.received.or(bytes.submitted_this_attempt)?;
        let elapsed = updated.saturating_duration_since(started).as_secs_f64();
        (elapsed > 0.0).then(|| (bytes as f64 / elapsed) as u64)
    }
}

/// Latest state or terminal record. A child completing never completes its execution.
#[derive(Debug, Clone)]
pub(in crate::orchestrator) struct Activity {
    pub(in crate::orchestrator) id: ActivityId,
    pub(in crate::orchestrator) parent: Option<ActivityId>,
    pub(in crate::orchestrator) execution: Arc<Execution>,
    pub(in crate::orchestrator) phase: Phase,
    pub(in crate::orchestrator) state: State,
    pub(in crate::orchestrator) progress: Option<Progress>,
    pub(in crate::orchestrator) effect: Effect,
    pub(in crate::orchestrator) effect_persisted: bool,
    pub(in crate::orchestrator) receipt: Option<WriteStatus>,
    pub(in crate::orchestrator) sequence: u64,
    pub(in crate::orchestrator) started: Instant,
    pub(in crate::orchestrator) updated: Instant,
    pub(in crate::orchestrator) last_wait: Option<(Wait, Duration)>,
    pub(super) trace: bool,
}

impl Activity {
    pub(in crate::orchestrator) fn bytes_per_second(&self) -> Option<u64> {
        self.progress?.rate(self.started, self.updated)
    }
}
