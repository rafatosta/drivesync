//! Sanitized saved recovery state; no conditional tags or credentials leave core.
use super::{Intent, ResolutionChoice, Stage};
use std::path::PathBuf;

/// User-facing recovery boundary, without transport/session implementation data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionState {
    Preparing,
    ReconcilingDrive,
    InstallingLocal,
    AuditPending,
}

/// Saved locations and choice for an accepted decision. Paths describe planned
/// or previously verified preservation; reading this does not check their bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionProgress {
    pub operation_id: String,
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    pub choice: ResolutionChoice,
    pub state: ResolutionState,
    pub preserved_paths: Vec<PathBuf>,
    pub additional_path: Option<PathBuf>,
}
pub(crate) fn saved(intent: &Intent) -> Option<ResolutionProgress> {
    let state = match intent.stage {
        Stage::Aborted => return None,
        Stage::Preparing => ResolutionState::Preparing,
        Stage::Preserved | Stage::CopyPending | Stage::RemotePending => {
            ResolutionState::ReconcilingDrive
        }
        Stage::RemoteVerified | Stage::LocalPending | Stage::Verified => {
            ResolutionState::InstallingLocal
        }
        Stage::Confirmed => ResolutionState::AuditPending,
    };
    Some(ResolutionProgress {
        operation_id: intent.id.clone(),
        source_path: intent.plan.origin.clone(),
        target_path: intent.plan.target.clone(),
        choice: intent.plan.choice,
        state,
        preserved_paths: vec![
            intent
                .plan
                .origin
                .with_file_name(super::local::remote_name(intent)),
            intent.plan.origin.with_file_name(intent.backup()),
        ],
        additional_path: intent
            .plan
            .copy_name
            .as_ref()
            .map(|name| intent.plan.target.with_file_name(name)),
    })
}
