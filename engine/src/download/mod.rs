//! Manual binary downloads and managed updates with preserved prior versions.
mod equivalence;
mod execution;
pub(crate) use equivalence::content_fingerprint;
pub(crate) mod filesystem;
mod links;
mod models;
mod stream;

#[cfg(test)]
pub(crate) use execution::run;
pub(crate) use execution::{run_with_root, Context, Source};
pub use models::{
    ConflictCopy, DownloadAttention, DownloadIssue, DownloadItemProgress, DownloadProgress,
    DownloadReport, DownloadStage, PreparationProgress, PreviousPath,
};
pub(crate) use models::{Identity, Operation, Phase, Preservation, Previous};

#[cfg(test)]
mod tests;
