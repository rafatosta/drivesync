//! Accounts-managed sessions for the remote detector, using existing Provider calls only.
use super::*;
use crate::detectors::remote::{
    google::{Api, Request, Response},
    Cancellation, RemoteError, RemoteFailure, RemoteResult,
};
struct Access {
    engine: Engine,
    account: AccountId,
}
impl Engine {
    pub(crate) fn remote_detector_api(&self, account: AccountId) -> Arc<dyn Api> {
        Arc::new(Access {
            engine: self.clone(),
            account,
        })
    }
}
fn failure(error: GoogleApiFailure) -> RemoteFailure {
    use GoogleApiError::*;
    let kind = match error.kind {
        ExpiredCursor => RemoteError::CursorExpired,
        AuthenticationRequired | ReauthorizationRequired | InvalidGrant | InsufficientScope => {
            RemoteError::AuthorizationRequired
        }
        PermissionDenied | NotFoundOrInaccessible => RemoteError::PermissionDenied,
        InvalidResponse | InvalidArgument => RemoteError::PageIncomplete,
        Cancelled => RemoteError::Cancelled,
        _ => RemoteError::RemoteUnavailable,
    };
    RemoteFailure {
        kind,
        retryable: error.retryable,
        retry_after: error
            .retry_after_seconds
            .map(std::time::Duration::from_secs),
    }
}
const FIELDS: &str = "id,name,mimeType,parents,trashed,version,createdTime,modifiedTime,size,md5Checksum,sha256Checksum";
impl Api for Access {
    fn call(&self, request: Request, cancellation: &Cancellation) -> RemoteResult<Response> {
        cancellation.check()?;
        let inner = self.engine.inner.clone();
        let account = self.account.clone();
        let cancel = cancellation.clone();
        // Only the detector's worker blocks here; HTTP and Accounts run on the owned executor.
        self.engine.runtime().block_on(async {
            let task = self.engine.runtime().spawn(async move {
                let operation = async {
                    let _lease = inner.ownership.gate.clone().read_owned().await;
                    let session = inner.accounts.acquire_session(AcquireSessionRequest { account_id: account.clone() }).await.map_err(|error| {
                        let kind = match error.kind {
                            AccountsError::NotFound | AccountsError::AuthorizationFailed | AccountsError::CredentialMissing => RemoteError::AuthorizationRequired,
                            _ => RemoteError::RemoteUnavailable,
                        };
                        RemoteFailure::new(kind)
                    })?;
                    let session = session.session.google();
                    if session.account_id != account.0 { return Err(RemoteFailure::new(RemoteError::InvalidScope)); }
                    let client = &inner.client;
                    Ok(match request {
                        Request::Start => Response::Start(client.changes_get_start_page_token(session, ChangeScopeOptions { supports_all_drives: Some(true), ..Default::default() }).await.map_err(failure)?.start_page_token),
                        Request::Get(id) => Response::File(Box::new(client.files_get(session, &FileId(id), FileGetOptions { fields: Some(FieldMask(FIELDS.into())), supports_all_drives: Some(true), ..Default::default() }).await.map_err(failure)?)),
                        Request::List { parent, token } => {
                            let quoted = parent.replace('\\', "\\\\").replace('\'', "\\'");
                            Response::List(client.files_list(session, FileListOptions {
                                q: Some(format!("'{quoted}' in parents and trashed = false")),
                                spaces: Some("drive".into()), page_size: Some(200), page_token: token,
                                supports_all_drives: Some(true), include_items_from_all_drives: Some(true),
                                fields: Some(FieldMask(format!("nextPageToken,incompleteSearch,files({FIELDS})"))), ..Default::default()
                            }).await.map_err(failure)?)
                        }
                        Request::Changes(token) => Response::Changes(client.changes_list(session, ChangeListOptions {
                            page_token: token, page_size: Some(200), include_removed: Some(true), spaces: Some("drive".into()),
                            supports_all_drives: Some(true), include_items_from_all_drives: Some(true),
                            fields: Some(FieldMask(format!("nextPageToken,newStartPageToken,changes(fileId,removed,file({FIELDS}))"))), ..Default::default()
                        }).await.map_err(failure)?),
                    })
                };
                tokio::select! { biased;
                    _ = cancel.cancelled() => Err(RemoteFailure::new(RemoteError::Cancelled)),
                    result = operation => result,
                }
            });
            task.await.map_err(|_| RemoteFailure::new(RemoteError::Internal))?
        })
    }
}
