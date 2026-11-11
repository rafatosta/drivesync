//! Adapts credential-free engine folder results to the reusable picker contract.
use crate::ui::components::remote_folder_picker::{
    FolderFuture, FolderPage, FolderSourceError, RemoteFolder, RemoteFolderId, RemoteFolderSource,
};
use drivesync_engine::{
    AccountId, AccountsError, Engine, EngineError, EngineErrorKind, ListRemoteFoldersRequest,
    RemoteFolderErrorKind,
};
use std::{cell::Cell, rc::Rc};
/// Account binding is immutable for the lifetime of a picker.
pub(crate) struct EngineFolderSource {
    engine: Engine,
    account: AccountId,
    authorization_required: Rc<Cell<bool>>,
}
impl EngineFolderSource {
    /// Resolves the real root before opening the picker; no credentials enter GTK.
    pub async fn open(
        engine: Engine,
        account: AccountId,
    ) -> Result<(RemoteFolder, Self), EngineError> {
        let root = engine.remote_folder_root(account.clone()).await?;
        Ok((
            convert(root),
            Self {
                engine,
                account,
                authorization_required: Rc::default(),
            },
        ))
    }
    pub fn authorization_required(&self) -> Rc<Cell<bool>> {
        self.authorization_required.clone()
    }
}
impl RemoteFolderSource for EngineFolderSource {
    fn list_folders(
        &self,
        parent_id: &RemoteFolderId,
        page_token: Option<&str>,
    ) -> FolderFuture<'_, Result<FolderPage, FolderSourceError>> {
        let request = ListRemoteFoldersRequest {
            account_id: self.account.clone(),
            parent_id: parent_id.0.clone(),
            page_token: page_token.map(str::to_owned),
        };
        Box::pin(async move {
            match self.engine.list_remote_folders(request).await {
                Ok(page) => Ok(FolderPage {
                    folders: page.folders.into_iter().map(convert).collect(),
                    next_page_token: page.next_page_token,
                }),
                Err(error) => {
                    if needs_authorization(&error) {
                        self.authorization_required.set(true);
                    }
                    Err(map_error(&error))
                }
            }
        })
    }
}
fn convert(folder: drivesync_engine::RemoteFolder) -> RemoteFolder {
    RemoteFolder {
        id: RemoteFolderId(folder.id),
        name: folder.name,
        parent_id: folder.parent_id.map(RemoteFolderId),
    }
}
pub(crate) fn needs_authorization(error: &EngineError) -> bool {
    matches!(
        error.kind,
        EngineErrorKind::RemoteFolder(RemoteFolderErrorKind::AuthorizationRequired)
            | EngineErrorKind::Accounts(
                AccountsError::AuthorizationFailed | AccountsError::CredentialMissing
            )
    )
}
fn map_error(error: &EngineError) -> FolderSourceError {
    if needs_authorization(error) {
        return FolderSourceError::Unauthorized;
    }
    match error.kind {
        EngineErrorKind::RemoteFolder(RemoteFolderErrorKind::PermissionDenied) => {
            FolderSourceError::Other(gettextrs::gettext(
                "You do not have permission to access this folder.",
            ))
        }
        EngineErrorKind::RemoteFolder(RemoteFolderErrorKind::NotFound)
        | EngineErrorKind::Accounts(AccountsError::NotFound) => FolderSourceError::NotFound,
        _ => FolderSourceError::Unavailable,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conversion_preserves_opaque_ids_and_scope_failures_require_consent() {
        let folder = convert(drivesync_engine::RemoteFolder {
            id: "opaque:/id".into(),
            name: "<&>".into(),
            parent_id: Some("parent".into()),
        });
        assert_eq!(folder.id.0, "opaque:/id");
        assert_eq!(folder.name, "<&>");
        let error = |kind| EngineError {
            kind: EngineErrorKind::RemoteFolder(kind),
            retryable: false,
            message: "ignored".into(),
        };
        assert!(needs_authorization(&error(
            RemoteFolderErrorKind::AuthorizationRequired
        )));
        assert!(!needs_authorization(&error(
            RemoteFolderErrorKind::PermissionDenied
        )));
        assert_eq!(
            map_error(&error(RemoteFolderErrorKind::AuthorizationRequired)),
            FolderSourceError::Unauthorized
        );
        assert_eq!(
            map_error(&error(RemoteFolderErrorKind::NotFound)),
            FolderSourceError::NotFound
        );
    }
}
