//! Public resource traits remain usable with Send network futures.
use crate::google_provider::support::{session, Reply, Server};
use drivesync_engine::providers::google::*;
fn assert_send<T: Send>(value: T) -> T {
    value
}

#[test]
fn the_client_satisfies_every_documented_resource_contract() {
    fn assert_contracts<
        T: GoogleAboutApi
            + GoogleAccessproposalsApi
            + GoogleApprovalsApi
            + GoogleAppsApi
            + GoogleChangesApi
            + GoogleChannelsApi
            + GoogleCommentsApi
            + GoogleDrivesApi
            + GoogleFilesApi
            + GoogleOperationsApi
            + GooglePermissionsApi
            + GoogleRepliesApi
            + GoogleRevisionsApi
            + GoogleOAuthApi
            + GoogleTransferApi
            + Send
            + Sync,
    >() {
    }
    assert_contracts::<GoogleDriveClient>();
}

#[tokio::test]
async fn public_trait_calls_use_the_real_async_transport() {
    let server = Server::start(vec![Reply::json(
        200,
        r#"{"files":[{"id":"f"}],"nextPageToken":"next"}"#,
    )]);
    let client = server.client();
    let session = session();
    let page = assert_send(GoogleFilesApi::files_list(
        &client,
        &session,
        FileListOptions::default(),
    ))
    .await
    .unwrap();
    assert_eq!(page.files[0].id, Some(FileId("f".into())));
    assert_eq!(page.next_page_token.as_deref(), Some("next"));
    assert_eq!(server.recorded()[0].path(), "/drive/v3/files");
}
