//! One manual, non-overwriting binary download cycle with durable replay evidence.
mod execution;
mod filesystem;
mod models;
mod stream;

pub(crate) use execution::{run, Context, Source};
pub use models::{
    DownloadIssue, DownloadItemProgress, DownloadProgress, DownloadReport, DownloadStage,
};
pub(crate) use models::{Operation, Phase};

#[cfg(test)]
mod tests;
