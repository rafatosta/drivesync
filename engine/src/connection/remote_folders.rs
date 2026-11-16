//! Account-scoped read-only browsing through existing Google Provider operations.
use super::*;
use std::future::Future;
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";

/// Dropping a UI query cancels its Tokio HTTP task. Accounts still owns any
/// credential transaction already running on a blocking worker.
struct QueryTask<T>(tokio::task::JoinHandle<EngineResult<T>>);
impl<T> Drop for QueryTask<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}
impl Engine {
    /// Revalidates one canonical folder ID under this account without changing Drive.
    /// Files, trashed folders, mismatched responses and inaccessible IDs are errors.
    pub async fn remote_folder(
        &self,
        account_id: AccountId,
        id: String,
    ) -> EngineResult<RemoteFolder> {
        if id.is_empty() {
            return Err(remote_error(RemoteFolderErrorKind::InvalidRequest));
        }
        self.remote_query(account_id, move |client, session| async move {
            let file = client
                .files_get(
                    &session,
                    &FileId(id.clone()),
                    FileGetOptions {
                        fields: Some(FieldMask("id,name,mimeType,trashed".into())),
                        supports_all_drives: Some(true),
                        ..Default::default()
                    },
                )
                .await
                .map_err(provider_error)?;
            let result = folder(file, None)?;
            if result.id != id {
                return Err(remote_error(RemoteFolderErrorKind::InvalidResponse));
            }
            Ok(result)
        })
        .await
    }

    /// Resolves the canonical root ID using the selected account's credentials.
    /// Awaitable from GLib; never returns tokens or mutates remote files.
    pub async fn remote_folder_root(&self, account_id: AccountId) -> EngineResult<RemoteFolder> {
        self.remote_query(account_id, |client, session| async move {
            let file = client
                .files_get(
                    &session,
                    &FileId("root".into()),
                    FileGetOptions {
                        fields: Some(FieldMask("id,name,mimeType,trashed".into())),
                        ..Default::default()
                    },
                )
                .await
                .map_err(provider_error)?;
            folder(file, None)
        })
        .await
    }
    /// Lists one page of non-trashed immediate folders. Missing metadata,
    /// unexpected parentage and incomplete searches fail rather than invent data.
    pub async fn list_remote_folders(
        &self,
        request: ListRemoteFoldersRequest,
    ) -> EngineResult<RemoteFolderPage> {
        if request.parent_id.is_empty()
            || request
                .page_token
                .as_ref()
                .is_some_and(|token| token.is_empty())
        {
            return Err(remote_error(RemoteFolderErrorKind::InvalidRequest));
        }
        self.remote_query(request.account_id, move |client, session| async move {
            let parent = request.parent_id;
            let quoted = parent.replace('\\', "\\\\").replace('\'', "\\'");
            let page = client.files_list(&session, FileListOptions {
                q: Some(format!("'{quoted}' in parents and mimeType = '{FOLDER_MIME}' and trashed = false")),
                spaces: Some("drive".into()), corpora: Some("user".into()),
                page_size: Some(100), page_token: request.page_token,
                order_by: Some("name".into()),
                fields: Some(FieldMask("nextPageToken,incompleteSearch,files(id,name,mimeType,parents,trashed)".into())),
                supports_all_drives: Some(true), include_items_from_all_drives: Some(true),
                ..Default::default()
            }).await.map_err(provider_error)?;
            if page.incomplete_search == Some(true) || page.next_page_token.as_ref().is_some_and(|token| token.is_empty()) {
                return Err(remote_error(RemoteFolderErrorKind::InvalidResponse));
            }
            let folders = page.files.into_iter().map(|file| folder(file, Some(&parent))).collect::<EngineResult<Vec<_>>>()?;
            Ok(RemoteFolderPage { folders, next_page_token: page.next_page_token })
        }).await
    }
    /// The connection read lease excludes logout until the query ends; each
    /// request obtains fresh Accounts-managed credentials on the engine executor.
    async fn remote_query<T, F, Q>(&self, account_id: AccountId, query: F) -> EngineResult<T>
    where
        T: Send + 'static,
        F: FnOnce(GoogleDriveClient, GoogleSession) -> Q + Send + 'static,
        Q: Future<Output = EngineResult<T>> + Send + 'static,
    {
        let _lease = self.inner.ownership.gate.clone().read_owned().await;
        let inner = self.inner.clone();
        let mut task = QueryTask(self.runtime().spawn(async move {
            let session = inner
                .accounts
                .acquire_session(AcquireSessionRequest { account_id })
                .await?;
            query(inner.client.clone(), session.session.google().clone()).await
        }));
        (&mut task.0)
            .await
            .map_err(|_| EngineError::new(EngineErrorKind::Internal))?
    }
}
fn remote_error(kind: RemoteFolderErrorKind) -> EngineError {
    let mut error = EngineError::new(EngineErrorKind::RemoteFolder(kind));
    error.retryable = kind == RemoteFolderErrorKind::Unavailable;
    error
}
fn provider_error(failure: GoogleApiFailure) -> EngineError {
    use GoogleApiError as G;
    let kind = match failure.kind {
        G::AuthenticationRequired
        | G::ReauthorizationRequired
        | G::InvalidGrant
        | G::InsufficientScope => RemoteFolderErrorKind::AuthorizationRequired,
        G::PermissionDenied => RemoteFolderErrorKind::PermissionDenied,
        G::NotFoundOrInaccessible => RemoteFolderErrorKind::NotFound,
        G::Network | G::Timeout | G::RateLimited | G::QuotaExceeded | G::ProviderUnavailable => {
            RemoteFolderErrorKind::Unavailable
        }
        _ => RemoteFolderErrorKind::InvalidResponse,
    };
    let mut error = remote_error(kind);
    error.retryable = failure.retryable;
    error
}
fn folder(file: GoogleFile, parent: Option<&str>) -> EngineResult<RemoteFolder> {
    let invalid = || remote_error(RemoteFolderErrorKind::InvalidResponse);
    if file.mime_type.as_ref().map(|mime| mime.0.as_str()) != Some(FOLDER_MIME)
        || file.trashed != Some(false)
    {
        return Err(invalid());
    }
    if let Some(parent) = parent {
        if !file
            .parents
            .as_ref()
            .is_some_and(|parents| parents.iter().any(|id| id.0 == parent))
        {
            return Err(invalid());
        }
    }
    let id = file.id.ok_or_else(invalid)?.0;
    let name = file.name.ok_or_else(invalid)?;
    if id.is_empty() || name.is_empty() {
        return Err(invalid());
    }
    Ok(RemoteFolder {
        id,
        name,
        parent_id: parent.map(str::to_owned),
    })
}
