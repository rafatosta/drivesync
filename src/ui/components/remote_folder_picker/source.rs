//! Local-thread asynchronous query boundary; implementations need not be Send.
use super::model::*;
use std::{future::Future, pin::Pin};
pub type FolderFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;
/// One page of immediate child folders and an opaque continuation token.
#[derive(Debug, Clone)]
pub struct FolderPage {
    pub folders: Vec<RemoteFolder>,
    pub next_page_token: Option<String>,
}
/// Recoverable query failures, displayed without changing external state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderSourceError {
    Unavailable,
    Unauthorized,
    NotFound,
    Other(String),
}
/// UI consumption contract; no authentication or mutation operations.
pub trait RemoteFolderSource {
    fn list_folders(
        &self,
        parent_id: &RemoteFolderId,
        page_token: Option<&str>,
    ) -> FolderFuture<'_, Result<FolderPage, FolderSourceError>>;
}
