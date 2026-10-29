//! Manual binary downloads and managed updates with preserved prior versions.
mod equivalence;
mod execution;
pub(crate) use equivalence::content_fingerprint;
pub(crate) mod filesystem;
mod links;
mod models;
mod stream;

pub(crate) use execution::{run, Context, Source};
pub use models::{
    DownloadIssue, DownloadItemProgress, DownloadProgress, DownloadReport, DownloadStage,
    PreparationProgress, PreviousPath,
};
pub(crate) use models::{Identity, Operation, Phase, Previous};

#[cfg(test)]
mod tests;
