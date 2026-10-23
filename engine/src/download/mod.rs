//! Manual binary downloads and managed updates with preserved prior versions.
mod execution;
mod filesystem;
mod models;
mod stream;

pub(crate) use execution::{run, Context, Source};
pub use models::{
    DownloadIssue, DownloadItemProgress, DownloadProgress, DownloadReport, DownloadStage,
};
pub(crate) use models::{Operation, Phase, Previous};

#[cfg(test)]
mod tests;
