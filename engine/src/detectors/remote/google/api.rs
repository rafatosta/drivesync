//! Google-specific use of Provider operations with renewable Accounts sessions.
use super::{Api, Request, Response};
use crate::{
    detectors::remote::{
        Cancellation, RemoteError, RemoteFailure, RemoteResult, RemoteSessionContext,
    },
    providers::google::*,
    Engine,
};
pub(super) struct Access {
    engine: Engine,
    session: RemoteSessionContext,
}
impl Access {
    pub fn new(engine: Engine, session: RemoteSessionContext) -> Self {
        Self { engine, session }
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
        let client = self.engine.google_drive_client();
        let account = self.session.account_id.clone();
        self.engine.with_remote_session(&self.session, cancellation, move |session| async move {
            let session = session.google();
            if session.account_id != account.0 { return Err(RemoteFailure::new(RemoteError::InvalidScope)); }
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
                    fields: Some(FieldMask(format!("nextPageToken,newStartPageToken,changes(fileId,removed,time,changeType,driveId,file({FIELDS}))"))), ..Default::default()
                }).await.map_err(failure)?),
            })
        })
    }
}
