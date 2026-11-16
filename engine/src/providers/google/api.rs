//! Resource-specific public contracts. Each forwards to the same tested client.
//! Network futures are Send; no universal provider trait or scheduler is introduced.
use super::*;

/// Public technical contract for About.
pub trait GoogleAboutApi {
    /// See [`GoogleDriveClient::about_get`] for protocol behavior and errors.
    fn about_get(
        &self,
        session: &GoogleSession,
        options: AboutOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleAbout>> + Send;
}
impl GoogleAboutApi for GoogleDriveClient {
    async fn about_get(
        &self,
        session: &GoogleSession,
        options: AboutOptions,
    ) -> GoogleResult<GoogleAbout> {
        GoogleDriveClient::about_get(self, session, options).await
    }
}

/// Public technical contract for Accessproposals.
pub trait GoogleAccessproposalsApi {
    /// See [`GoogleDriveClient::access_proposals_get`] for protocol behavior and errors.
    fn access_proposals_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        proposal_id: &ProposalId,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleAccessProposal>> + Send;
    /// See [`GoogleDriveClient::access_proposals_list`] for protocol behavior and errors.
    fn access_proposals_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: PageOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleAccessProposalList>> + Send;
    /// See [`GoogleDriveClient::access_proposals_resolve`] for protocol behavior and errors.
    fn access_proposals_resolve(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        proposal_id: &ProposalId,
        request: ResolveAccessProposalRequest,
    ) -> impl std::future::Future<Output = GoogleResult<()>> + Send;
}
impl GoogleAccessproposalsApi for GoogleDriveClient {
    async fn access_proposals_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        proposal_id: &ProposalId,
    ) -> GoogleResult<GoogleAccessProposal> {
        GoogleDriveClient::access_proposals_get(self, session, file_id, proposal_id).await
    }
    async fn access_proposals_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: PageOptions,
    ) -> GoogleResult<GoogleAccessProposalList> {
        GoogleDriveClient::access_proposals_list(self, session, file_id, options).await
    }
    async fn access_proposals_resolve(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        proposal_id: &ProposalId,
        request: ResolveAccessProposalRequest,
    ) -> GoogleResult<()> {
        GoogleDriveClient::access_proposals_resolve(self, session, file_id, proposal_id, request)
            .await
    }
}

/// Public technical contract for Approvals.
pub trait GoogleApprovalsApi {
    /// See [`GoogleDriveClient::approvals_approve`] for protocol behavior and errors.
    fn approvals_approve(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: ApproveApprovalRequest,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleApproval>> + Send;
    /// See [`GoogleDriveClient::approvals_cancel`] for protocol behavior and errors.
    fn approvals_cancel(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: CancelApprovalRequest,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleApproval>> + Send;
    /// See [`GoogleDriveClient::approvals_comment`] for protocol behavior and errors.
    fn approvals_comment(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: CommentApprovalRequest,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleApproval>> + Send;
    /// See [`GoogleDriveClient::approvals_decline`] for protocol behavior and errors.
    fn approvals_decline(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: DeclineApprovalRequest,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleApproval>> + Send;
    /// See [`GoogleDriveClient::approvals_get`] for protocol behavior and errors.
    fn approvals_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleApproval>> + Send;
    /// See [`GoogleDriveClient::approvals_list`] for protocol behavior and errors.
    fn approvals_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: PageOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleApprovalList>> + Send;
    /// See [`GoogleDriveClient::approvals_reassign`] for protocol behavior and errors.
    fn approvals_reassign(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: ReassignApprovalRequest,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleApproval>> + Send;
    /// See [`GoogleDriveClient::approvals_start`] for protocol behavior and errors.
    fn approvals_start(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: StartApprovalRequest,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleApproval>> + Send;
}
impl GoogleApprovalsApi for GoogleDriveClient {
    async fn approvals_approve(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: ApproveApprovalRequest,
    ) -> GoogleResult<GoogleApproval> {
        GoogleDriveClient::approvals_approve(self, session, file_id, approval_id, request).await
    }
    async fn approvals_cancel(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: CancelApprovalRequest,
    ) -> GoogleResult<GoogleApproval> {
        GoogleDriveClient::approvals_cancel(self, session, file_id, approval_id, request).await
    }
    async fn approvals_comment(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: CommentApprovalRequest,
    ) -> GoogleResult<GoogleApproval> {
        GoogleDriveClient::approvals_comment(self, session, file_id, approval_id, request).await
    }
    async fn approvals_decline(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: DeclineApprovalRequest,
    ) -> GoogleResult<GoogleApproval> {
        GoogleDriveClient::approvals_decline(self, session, file_id, approval_id, request).await
    }
    async fn approvals_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
    ) -> GoogleResult<GoogleApproval> {
        GoogleDriveClient::approvals_get(self, session, file_id, approval_id).await
    }
    async fn approvals_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: PageOptions,
    ) -> GoogleResult<GoogleApprovalList> {
        GoogleDriveClient::approvals_list(self, session, file_id, options).await
    }
    async fn approvals_reassign(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: ReassignApprovalRequest,
    ) -> GoogleResult<GoogleApproval> {
        GoogleDriveClient::approvals_reassign(self, session, file_id, approval_id, request).await
    }
    async fn approvals_start(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: StartApprovalRequest,
    ) -> GoogleResult<GoogleApproval> {
        GoogleDriveClient::approvals_start(self, session, file_id, request).await
    }
}

/// Public technical contract for Apps.
pub trait GoogleAppsApi {
    /// See [`GoogleDriveClient::apps_get`] for protocol behavior and errors.
    fn apps_get(
        &self,
        session: &GoogleSession,
        app_id: &AppId,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleApp>> + Send;
    /// See [`GoogleDriveClient::apps_list`] for protocol behavior and errors.
    fn apps_list(
        &self,
        session: &GoogleSession,
        options: AppListOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleAppList>> + Send;
}
impl GoogleAppsApi for GoogleDriveClient {
    async fn apps_get(&self, session: &GoogleSession, app_id: &AppId) -> GoogleResult<GoogleApp> {
        GoogleDriveClient::apps_get(self, session, app_id).await
    }
    async fn apps_list(
        &self,
        session: &GoogleSession,
        options: AppListOptions,
    ) -> GoogleResult<GoogleAppList> {
        GoogleDriveClient::apps_list(self, session, options).await
    }
}

/// Public technical contract for Changes.
pub trait GoogleChangesApi {
    /// See [`GoogleDriveClient::changes_get_start_page_token`] for protocol behavior and errors.
    fn changes_get_start_page_token(
        &self,
        session: &GoogleSession,
        options: ChangeScopeOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleStartPageToken>> + Send;
    /// See [`GoogleDriveClient::changes_list`] for protocol behavior and errors.
    fn changes_list(
        &self,
        session: &GoogleSession,
        options: ChangeListOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleChangeList>> + Send;
    /// See [`GoogleDriveClient::changes_watch`] for protocol behavior and errors.
    fn changes_watch(
        &self,
        session: &GoogleSession,
        options: ChangeWatchOptions,
        channel: GoogleChannelRequest,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleChannel>> + Send;
}
impl GoogleChangesApi for GoogleDriveClient {
    async fn changes_get_start_page_token(
        &self,
        session: &GoogleSession,
        options: ChangeScopeOptions,
    ) -> GoogleResult<GoogleStartPageToken> {
        GoogleDriveClient::changes_get_start_page_token(self, session, options).await
    }
    async fn changes_list(
        &self,
        session: &GoogleSession,
        options: ChangeListOptions,
    ) -> GoogleResult<GoogleChangeList> {
        GoogleDriveClient::changes_list(self, session, options).await
    }
    async fn changes_watch(
        &self,
        session: &GoogleSession,
        options: ChangeWatchOptions,
        channel: GoogleChannelRequest,
    ) -> GoogleResult<GoogleChannel> {
        GoogleDriveClient::changes_watch(self, session, options, channel).await
    }
}

/// Public technical contract for Channels.
pub trait GoogleChannelsApi {
    /// See [`GoogleDriveClient::channels_stop`] for protocol behavior and errors.
    fn channels_stop(
        &self,
        session: &GoogleSession,
        channel: GoogleChannelStopRequest,
    ) -> impl std::future::Future<Output = GoogleResult<()>> + Send;
}
impl GoogleChannelsApi for GoogleDriveClient {
    async fn channels_stop(
        &self,
        session: &GoogleSession,
        channel: GoogleChannelStopRequest,
    ) -> GoogleResult<()> {
        GoogleDriveClient::channels_stop(self, session, channel).await
    }
}

/// Public technical contract for Comments.
pub trait GoogleCommentsApi {
    /// See [`GoogleDriveClient::comments_create`] for protocol behavior and errors.
    fn comments_create(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleCommentWrite,
        options: CommentOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleComment>> + Send;
    /// See [`GoogleDriveClient::comments_delete`] for protocol behavior and errors.
    fn comments_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
    ) -> impl std::future::Future<Output = GoogleResult<()>> + Send;
    /// See [`GoogleDriveClient::comments_get`] for protocol behavior and errors.
    fn comments_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        options: CommentOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleComment>> + Send;
    /// See [`GoogleDriveClient::comments_list`] for protocol behavior and errors.
    fn comments_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: CommentListOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleCommentList>> + Send;
    /// See [`GoogleDriveClient::comments_update`] for protocol behavior and errors.
    fn comments_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        request: GoogleCommentWrite,
        options: CommentOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleComment>> + Send;
}
impl GoogleCommentsApi for GoogleDriveClient {
    async fn comments_create(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleCommentWrite,
        options: CommentOptions,
    ) -> GoogleResult<GoogleComment> {
        GoogleDriveClient::comments_create(self, session, file_id, request, options).await
    }
    async fn comments_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
    ) -> GoogleResult<()> {
        GoogleDriveClient::comments_delete(self, session, file_id, comment_id).await
    }
    async fn comments_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        options: CommentOptions,
    ) -> GoogleResult<GoogleComment> {
        GoogleDriveClient::comments_get(self, session, file_id, comment_id, options).await
    }
    async fn comments_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: CommentListOptions,
    ) -> GoogleResult<GoogleCommentList> {
        GoogleDriveClient::comments_list(self, session, file_id, options).await
    }
    async fn comments_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        request: GoogleCommentWrite,
        options: CommentOptions,
    ) -> GoogleResult<GoogleComment> {
        GoogleDriveClient::comments_update(self, session, file_id, comment_id, request, options)
            .await
    }
}

/// Public technical contract for Drives.
pub trait GoogleDrivesApi {
    /// See [`GoogleDriveClient::drives_create`] for protocol behavior and errors.
    fn drives_create(
        &self,
        session: &GoogleSession,
        request_id: &RequestId,
        request: GoogleDriveWrite,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleDrive>> + Send;
    /// See [`GoogleDriveClient::drives_delete`] for protocol behavior and errors.
    fn drives_delete(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
        options: DriveDeleteOptions,
    ) -> impl std::future::Future<Output = GoogleResult<()>> + Send;
    /// See [`GoogleDriveClient::drives_get`] for protocol behavior and errors.
    fn drives_get(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
        options: DriveGetOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleDrive>> + Send;
    /// See [`GoogleDriveClient::drives_hide`] for protocol behavior and errors.
    fn drives_hide(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleDrive>> + Send;
    /// See [`GoogleDriveClient::drives_list`] for protocol behavior and errors.
    fn drives_list(
        &self,
        session: &GoogleSession,
        options: DriveListOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleDriveList>> + Send;
    /// See [`GoogleDriveClient::drives_unhide`] for protocol behavior and errors.
    fn drives_unhide(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleDrive>> + Send;
    /// See [`GoogleDriveClient::drives_update`] for protocol behavior and errors.
    fn drives_update(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
        request: GoogleDriveWrite,
        options: DriveUpdateOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleDrive>> + Send;
}
impl GoogleDrivesApi for GoogleDriveClient {
    async fn drives_create(
        &self,
        session: &GoogleSession,
        request_id: &RequestId,
        request: GoogleDriveWrite,
    ) -> GoogleResult<GoogleDrive> {
        GoogleDriveClient::drives_create(self, session, request_id, request).await
    }
    async fn drives_delete(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
        options: DriveDeleteOptions,
    ) -> GoogleResult<()> {
        GoogleDriveClient::drives_delete(self, session, drive_id, options).await
    }
    async fn drives_get(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
        options: DriveGetOptions,
    ) -> GoogleResult<GoogleDrive> {
        GoogleDriveClient::drives_get(self, session, drive_id, options).await
    }
    async fn drives_hide(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
    ) -> GoogleResult<GoogleDrive> {
        GoogleDriveClient::drives_hide(self, session, drive_id).await
    }
    async fn drives_list(
        &self,
        session: &GoogleSession,
        options: DriveListOptions,
    ) -> GoogleResult<GoogleDriveList> {
        GoogleDriveClient::drives_list(self, session, options).await
    }
    async fn drives_unhide(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
    ) -> GoogleResult<GoogleDrive> {
        GoogleDriveClient::drives_unhide(self, session, drive_id).await
    }
    async fn drives_update(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
        request: GoogleDriveWrite,
        options: DriveUpdateOptions,
    ) -> GoogleResult<GoogleDrive> {
        GoogleDriveClient::drives_update(self, session, drive_id, request, options).await
    }
}

/// Public technical contract for Files.
pub trait GoogleFilesApi {
    /// See [`GoogleDriveClient::files_copy`] for protocol behavior and errors.
    fn files_copy(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleFileWrite,
        options: FileCopyOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleFile>> + Send;
    /// See [`GoogleDriveClient::files_create`] for protocol behavior and errors.
    fn files_create(
        &self,
        session: &GoogleSession,
        request: GoogleFileWrite,
        options: FileCreateOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleFile>> + Send;
    /// See [`GoogleDriveClient::files_delete`] for protocol behavior and errors.
    fn files_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: FileDeleteOptions,
    ) -> impl std::future::Future<Output = GoogleResult<()>> + Send;
    /// See [`GoogleDriveClient::files_download`] for protocol behavior and errors.
    fn files_download(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: FileDownloadOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleOperation>> + Send;
    /// See [`GoogleDriveClient::files_empty_trash`] for protocol behavior and errors.
    fn files_empty_trash(
        &self,
        session: &GoogleSession,
        options: EmptyTrashOptions,
    ) -> impl std::future::Future<Output = GoogleResult<()>> + Send;
    /// See [`GoogleDriveClient::files_export`] for protocol behavior and errors.
    fn files_export(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        mime_type: &MimeType,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleByteStream>> + Send;
    /// See [`GoogleDriveClient::files_generate_cse_token`] for protocol behavior and errors.
    fn files_generate_cse_token(
        &self,
        session: &GoogleSession,
        options: CseTokenOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleCseToken>> + Send;
    /// See [`GoogleDriveClient::files_generate_ids`] for protocol behavior and errors.
    fn files_generate_ids(
        &self,
        session: &GoogleSession,
        options: GenerateIdsOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleGeneratedIds>> + Send;
    /// See [`GoogleDriveClient::files_get`] for protocol behavior and errors.
    fn files_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: FileGetOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleFile>> + Send;
    /// See [`GoogleDriveClient::files_list`] for protocol behavior and errors.
    fn files_list(
        &self,
        session: &GoogleSession,
        options: FileListOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleFileList>> + Send;
    /// See [`GoogleDriveClient::files_list_labels`] for protocol behavior and errors.
    fn files_list_labels(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: LabelListOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleLabelList>> + Send;
    /// See [`GoogleDriveClient::files_modify_labels`] for protocol behavior and errors.
    fn files_modify_labels(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleModifyLabelsRequest,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleModifyLabelsResponse>> + Send;
    /// See [`GoogleDriveClient::files_update`] for protocol behavior and errors.
    fn files_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleFileWrite,
        options: FileUpdateOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleFile>> + Send;
    /// See [`GoogleDriveClient::files_watch`] for protocol behavior and errors.
    fn files_watch(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: FileWatchOptions,
        channel: GoogleChannelRequest,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleChannel>> + Send;
}
impl GoogleFilesApi for GoogleDriveClient {
    async fn files_copy(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleFileWrite,
        options: FileCopyOptions,
    ) -> GoogleResult<GoogleFile> {
        GoogleDriveClient::files_copy(self, session, file_id, request, options).await
    }
    async fn files_create(
        &self,
        session: &GoogleSession,
        request: GoogleFileWrite,
        options: FileCreateOptions,
    ) -> GoogleResult<GoogleFile> {
        GoogleDriveClient::files_create(self, session, request, options).await
    }
    async fn files_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: FileDeleteOptions,
    ) -> GoogleResult<()> {
        GoogleDriveClient::files_delete(self, session, file_id, options).await
    }
    async fn files_download(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: FileDownloadOptions,
    ) -> GoogleResult<GoogleOperation> {
        GoogleDriveClient::files_download(self, session, file_id, options).await
    }
    async fn files_empty_trash(
        &self,
        session: &GoogleSession,
        options: EmptyTrashOptions,
    ) -> GoogleResult<()> {
        GoogleDriveClient::files_empty_trash(self, session, options).await
    }
    async fn files_export(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        mime_type: &MimeType,
    ) -> GoogleResult<GoogleByteStream> {
        GoogleDriveClient::files_export(self, session, file_id, mime_type).await
    }
    async fn files_generate_cse_token(
        &self,
        session: &GoogleSession,
        options: CseTokenOptions,
    ) -> GoogleResult<GoogleCseToken> {
        GoogleDriveClient::files_generate_cse_token(self, session, options).await
    }
    async fn files_generate_ids(
        &self,
        session: &GoogleSession,
        options: GenerateIdsOptions,
    ) -> GoogleResult<GoogleGeneratedIds> {
        GoogleDriveClient::files_generate_ids(self, session, options).await
    }
    async fn files_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: FileGetOptions,
    ) -> GoogleResult<GoogleFile> {
        GoogleDriveClient::files_get(self, session, file_id, options).await
    }
    async fn files_list(
        &self,
        session: &GoogleSession,
        options: FileListOptions,
    ) -> GoogleResult<GoogleFileList> {
        GoogleDriveClient::files_list(self, session, options).await
    }
    async fn files_list_labels(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: LabelListOptions,
    ) -> GoogleResult<GoogleLabelList> {
        GoogleDriveClient::files_list_labels(self, session, file_id, options).await
    }
    async fn files_modify_labels(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleModifyLabelsRequest,
    ) -> GoogleResult<GoogleModifyLabelsResponse> {
        GoogleDriveClient::files_modify_labels(self, session, file_id, request).await
    }
    async fn files_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleFileWrite,
        options: FileUpdateOptions,
    ) -> GoogleResult<GoogleFile> {
        GoogleDriveClient::files_update(self, session, file_id, request, options).await
    }
    async fn files_watch(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: FileWatchOptions,
        channel: GoogleChannelRequest,
    ) -> GoogleResult<GoogleChannel> {
        GoogleDriveClient::files_watch(self, session, file_id, options, channel).await
    }
}

/// Public technical contract for Operations.
pub trait GoogleOperationsApi {
    /// See [`GoogleDriveClient::operations_get`] for protocol behavior and errors.
    fn operations_get(
        &self,
        session: &GoogleSession,
        name: &OperationName,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleOperation>> + Send;
}
impl GoogleOperationsApi for GoogleDriveClient {
    async fn operations_get(
        &self,
        session: &GoogleSession,
        name: &OperationName,
    ) -> GoogleResult<GoogleOperation> {
        GoogleDriveClient::operations_get(self, session, name).await
    }
}

/// Public technical contract for Permissions.
pub trait GooglePermissionsApi {
    /// See [`GoogleDriveClient::permissions_create`] for protocol behavior and errors.
    fn permissions_create(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GooglePermissionWrite,
        options: PermissionCreateOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GooglePermission>> + Send;
    /// See [`GoogleDriveClient::permissions_delete`] for protocol behavior and errors.
    fn permissions_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        permission_id: &PermissionId,
        options: PermissionDeleteOptions,
    ) -> impl std::future::Future<Output = GoogleResult<()>> + Send;
    /// See [`GoogleDriveClient::permissions_get`] for protocol behavior and errors.
    fn permissions_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        permission_id: &PermissionId,
        options: PermissionGetOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GooglePermission>> + Send;
    /// See [`GoogleDriveClient::permissions_list`] for protocol behavior and errors.
    fn permissions_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: PermissionListOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GooglePermissionList>> + Send;
    /// See [`GoogleDriveClient::permissions_update`] for protocol behavior and errors.
    fn permissions_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        permission_id: &PermissionId,
        request: GooglePermissionWrite,
        options: PermissionUpdateOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GooglePermission>> + Send;
}
impl GooglePermissionsApi for GoogleDriveClient {
    async fn permissions_create(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GooglePermissionWrite,
        options: PermissionCreateOptions,
    ) -> GoogleResult<GooglePermission> {
        GoogleDriveClient::permissions_create(self, session, file_id, request, options).await
    }
    async fn permissions_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        permission_id: &PermissionId,
        options: PermissionDeleteOptions,
    ) -> GoogleResult<()> {
        GoogleDriveClient::permissions_delete(self, session, file_id, permission_id, options).await
    }
    async fn permissions_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        permission_id: &PermissionId,
        options: PermissionGetOptions,
    ) -> GoogleResult<GooglePermission> {
        GoogleDriveClient::permissions_get(self, session, file_id, permission_id, options).await
    }
    async fn permissions_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: PermissionListOptions,
    ) -> GoogleResult<GooglePermissionList> {
        GoogleDriveClient::permissions_list(self, session, file_id, options).await
    }
    async fn permissions_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        permission_id: &PermissionId,
        request: GooglePermissionWrite,
        options: PermissionUpdateOptions,
    ) -> GoogleResult<GooglePermission> {
        GoogleDriveClient::permissions_update(
            self,
            session,
            file_id,
            permission_id,
            request,
            options,
        )
        .await
    }
}

/// Public technical contract for Replies.
pub trait GoogleRepliesApi {
    /// See [`GoogleDriveClient::replies_create`] for protocol behavior and errors.
    fn replies_create(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        request: GoogleReplyWrite,
        options: ReplyOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleReply>> + Send;
    /// See [`GoogleDriveClient::replies_delete`] for protocol behavior and errors.
    fn replies_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        reply_id: &ReplyId,
    ) -> impl std::future::Future<Output = GoogleResult<()>> + Send;
    /// See [`GoogleDriveClient::replies_get`] for protocol behavior and errors.
    fn replies_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        reply_id: &ReplyId,
        options: ReplyOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleReply>> + Send;
    /// See [`GoogleDriveClient::replies_list`] for protocol behavior and errors.
    fn replies_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        options: ReplyListOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleReplyList>> + Send;
    /// See [`GoogleDriveClient::replies_update`] for protocol behavior and errors.
    fn replies_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        reply_id: &ReplyId,
        request: GoogleReplyWrite,
        options: ReplyOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleReply>> + Send;
}
impl GoogleRepliesApi for GoogleDriveClient {
    async fn replies_create(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        request: GoogleReplyWrite,
        options: ReplyOptions,
    ) -> GoogleResult<GoogleReply> {
        GoogleDriveClient::replies_create(self, session, file_id, comment_id, request, options)
            .await
    }
    async fn replies_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        reply_id: &ReplyId,
    ) -> GoogleResult<()> {
        GoogleDriveClient::replies_delete(self, session, file_id, comment_id, reply_id).await
    }
    async fn replies_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        reply_id: &ReplyId,
        options: ReplyOptions,
    ) -> GoogleResult<GoogleReply> {
        GoogleDriveClient::replies_get(self, session, file_id, comment_id, reply_id, options).await
    }
    async fn replies_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        options: ReplyListOptions,
    ) -> GoogleResult<GoogleReplyList> {
        GoogleDriveClient::replies_list(self, session, file_id, comment_id, options).await
    }
    async fn replies_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        reply_id: &ReplyId,
        request: GoogleReplyWrite,
        options: ReplyOptions,
    ) -> GoogleResult<GoogleReply> {
        GoogleDriveClient::replies_update(
            self, session, file_id, comment_id, reply_id, request, options,
        )
        .await
    }
}

/// Public technical contract for Revisions.
pub trait GoogleRevisionsApi {
    /// See [`GoogleDriveClient::revisions_delete`] for protocol behavior and errors.
    fn revisions_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        revision_id: &RevisionId,
    ) -> impl std::future::Future<Output = GoogleResult<()>> + Send;
    /// See [`GoogleDriveClient::revisions_get`] for protocol behavior and errors.
    fn revisions_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        revision_id: &RevisionId,
        options: RevisionGetOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleRevision>> + Send;
    /// See [`GoogleDriveClient::revisions_list`] for protocol behavior and errors.
    fn revisions_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: RevisionListOptions,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleRevisionList>> + Send;
    /// See [`GoogleDriveClient::revisions_update`] for protocol behavior and errors.
    fn revisions_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        revision_id: &RevisionId,
        request: GoogleRevisionWrite,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleRevision>> + Send;
}
impl GoogleRevisionsApi for GoogleDriveClient {
    async fn revisions_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        revision_id: &RevisionId,
    ) -> GoogleResult<()> {
        GoogleDriveClient::revisions_delete(self, session, file_id, revision_id).await
    }
    async fn revisions_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        revision_id: &RevisionId,
        options: RevisionGetOptions,
    ) -> GoogleResult<GoogleRevision> {
        GoogleDriveClient::revisions_get(self, session, file_id, revision_id, options).await
    }
    async fn revisions_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: RevisionListOptions,
    ) -> GoogleResult<GoogleRevisionList> {
        GoogleDriveClient::revisions_list(self, session, file_id, options).await
    }
    async fn revisions_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        revision_id: &RevisionId,
        request: GoogleRevisionWrite,
    ) -> GoogleResult<GoogleRevision> {
        GoogleDriveClient::revisions_update(self, session, file_id, revision_id, request).await
    }
}

/// Public technical contract for OAuth.
pub trait GoogleOAuthApi {
    /// See [`GoogleDriveClient::load_google_config`] for protocol behavior and errors.
    fn load_google_config(
        &self,
        sources: GoogleConfigSources,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleOAuthConfig>> + Send;
    /// See [`GoogleDriveClient::authorization_request`] for protocol behavior and errors.
    fn authorization_request(
        &self,
        config: &GoogleOAuthConfig,
        request: GoogleAuthorizationRequest,
    ) -> GoogleResult<GoogleAuthorizationUrl>;
    /// See [`GoogleDriveClient::exchange_authorization_code`] for protocol behavior and errors.
    fn exchange_authorization_code(
        &self,
        config: &GoogleOAuthConfig,
        code: &str,
        redirect_uri: &str,
        pkce_verifier: &str,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleTokenResponse>> + Send;
    /// See [`GoogleDriveClient::refresh_access_token`] for protocol behavior and errors.
    fn refresh_access_token(
        &self,
        config: &GoogleOAuthConfig,
        refresh_token: &str,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleTokenResponse>> + Send;
    /// See [`GoogleDriveClient::revoke_token`] for protocol behavior and errors.
    fn revoke_token(
        &self,
        token: &str,
    ) -> impl std::future::Future<Output = GoogleResult<()>> + Send;
    /// See [`GoogleDriveClient::get_userinfo`] for protocol behavior and errors.
    fn get_userinfo(
        &self,
        session: &GoogleSession,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleUserInfo>> + Send;
    /// See [`GoogleDriveClient::validate_id_token`] for protocol behavior and errors.
    fn validate_id_token(
        &self,
        id_token: &str,
        validation: GoogleIdTokenValidation,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleIdClaims>> + Send;
    /// See [`GoogleDriveClient::get_oidc_discovery`] for protocol behavior and errors.
    fn get_oidc_discovery(
        &self,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleOidcDiscovery>> + Send;
    /// See [`GoogleDriveClient::get_oidc_keys`] for protocol behavior and errors.
    fn get_oidc_keys(&self)
        -> impl std::future::Future<Output = GoogleResult<GoogleJwkSet>> + Send;
}
impl GoogleOAuthApi for GoogleDriveClient {
    async fn load_google_config(
        &self,
        sources: GoogleConfigSources,
    ) -> GoogleResult<GoogleOAuthConfig> {
        GoogleDriveClient::load_google_config(self, sources).await
    }
    fn authorization_request(
        &self,
        config: &GoogleOAuthConfig,
        request: GoogleAuthorizationRequest,
    ) -> GoogleResult<GoogleAuthorizationUrl> {
        GoogleDriveClient::authorization_request(self, config, request)
    }
    async fn exchange_authorization_code(
        &self,
        config: &GoogleOAuthConfig,
        code: &str,
        redirect_uri: &str,
        pkce_verifier: &str,
    ) -> GoogleResult<GoogleTokenResponse> {
        GoogleDriveClient::exchange_authorization_code(
            self,
            config,
            code,
            redirect_uri,
            pkce_verifier,
        )
        .await
    }
    async fn refresh_access_token(
        &self,
        config: &GoogleOAuthConfig,
        refresh_token: &str,
    ) -> GoogleResult<GoogleTokenResponse> {
        GoogleDriveClient::refresh_access_token(self, config, refresh_token).await
    }
    async fn revoke_token(&self, token: &str) -> GoogleResult<()> {
        GoogleDriveClient::revoke_token(self, token).await
    }
    async fn get_userinfo(&self, session: &GoogleSession) -> GoogleResult<GoogleUserInfo> {
        GoogleDriveClient::get_userinfo(self, session).await
    }
    async fn validate_id_token(
        &self,
        id_token: &str,
        validation: GoogleIdTokenValidation,
    ) -> GoogleResult<GoogleIdClaims> {
        GoogleDriveClient::validate_id_token(self, id_token, validation).await
    }
    async fn get_oidc_discovery(&self) -> GoogleResult<GoogleOidcDiscovery> {
        GoogleDriveClient::get_oidc_discovery(self).await
    }
    async fn get_oidc_keys(&self) -> GoogleResult<GoogleJwkSet> {
        GoogleDriveClient::get_oidc_keys(self).await
    }
}

/// Public technical contract for Transfer.
pub trait GoogleTransferApi {
    /// See [`GoogleDriveClient::get_file_bytes`] for protocol behavior and errors.
    fn get_file_bytes(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        range: Option<GoogleByteRange>,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleDownloadStream>> + Send;
    /// See [`GoogleDriveClient::get_revision_bytes`] for protocol behavior and errors.
    fn get_revision_bytes(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        revision_id: &RevisionId,
        range: Option<GoogleByteRange>,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleDownloadStream>> + Send;
    /// See [`GoogleDriveClient::export_file_bytes`] for protocol behavior and errors.
    fn export_file_bytes(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        mime_type: &MimeType,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleDownloadStream>> + Send;
    /// See [`GoogleDriveClient::start_resumable_create`] for protocol behavior and errors.
    fn start_resumable_create(
        &self,
        session: &GoogleSession,
        request: GoogleFileWrite,
        options: FileCreateOptions,
        total_bytes: Option<u64>,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleUploadSession>> + Send;
    /// See [`GoogleDriveClient::start_resumable_update`] for protocol behavior and errors.
    fn start_resumable_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleFileWrite,
        options: FileUpdateOptions,
        total_bytes: Option<u64>,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleUploadSession>> + Send;
    /// See [`GoogleDriveClient::upload_chunk`] for protocol behavior and errors.
    fn upload_chunk(
        &self,
        session: &GoogleSession,
        upload: &GoogleUploadSession,
        chunk: GoogleUploadChunk,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleUploadProgress>> + Send;
    /// See [`GoogleDriveClient::query_upload_progress`] for protocol behavior and errors.
    fn query_upload_progress(
        &self,
        session: &GoogleSession,
        upload: &GoogleUploadSession,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleUploadProgress>> + Send;
    /// See [`GoogleDriveClient::trash_file`] for protocol behavior and errors.
    fn trash_file(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleFile>> + Send;
    /// See [`GoogleDriveClient::rename_file`] for protocol behavior and errors.
    fn rename_file(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        name: &str,
    ) -> impl std::future::Future<Output = GoogleResult<GoogleFile>> + Send;
    /// See [`GoogleDriveClient::move_file`] for protocol behavior and errors.
    fn move_file(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        add_parent: &FileId,
        remove_parents: &[FileId],
    ) -> impl std::future::Future<Output = GoogleResult<GoogleFile>> + Send;
}
impl GoogleTransferApi for GoogleDriveClient {
    async fn get_file_bytes(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        range: Option<GoogleByteRange>,
    ) -> GoogleResult<GoogleDownloadStream> {
        GoogleDriveClient::get_file_bytes(self, session, file_id, range).await
    }
    async fn get_revision_bytes(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        revision_id: &RevisionId,
        range: Option<GoogleByteRange>,
    ) -> GoogleResult<GoogleDownloadStream> {
        GoogleDriveClient::get_revision_bytes(self, session, file_id, revision_id, range).await
    }
    async fn export_file_bytes(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        mime_type: &MimeType,
    ) -> GoogleResult<GoogleDownloadStream> {
        GoogleDriveClient::export_file_bytes(self, session, file_id, mime_type).await
    }
    async fn start_resumable_create(
        &self,
        session: &GoogleSession,
        request: GoogleFileWrite,
        options: FileCreateOptions,
        total_bytes: Option<u64>,
    ) -> GoogleResult<GoogleUploadSession> {
        GoogleDriveClient::start_resumable_create(self, session, request, options, total_bytes)
            .await
    }
    async fn start_resumable_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleFileWrite,
        options: FileUpdateOptions,
        total_bytes: Option<u64>,
    ) -> GoogleResult<GoogleUploadSession> {
        GoogleDriveClient::start_resumable_update(
            self,
            session,
            file_id,
            request,
            options,
            total_bytes,
        )
        .await
    }
    async fn upload_chunk(
        &self,
        session: &GoogleSession,
        upload: &GoogleUploadSession,
        chunk: GoogleUploadChunk,
    ) -> GoogleResult<GoogleUploadProgress> {
        GoogleDriveClient::upload_chunk(self, session, upload, chunk).await
    }
    async fn query_upload_progress(
        &self,
        session: &GoogleSession,
        upload: &GoogleUploadSession,
    ) -> GoogleResult<GoogleUploadProgress> {
        GoogleDriveClient::query_upload_progress(self, session, upload).await
    }
    async fn trash_file(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
    ) -> GoogleResult<GoogleFile> {
        GoogleDriveClient::trash_file(self, session, file_id).await
    }
    async fn rename_file(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        name: &str,
    ) -> GoogleResult<GoogleFile> {
        GoogleDriveClient::rename_file(self, session, file_id, name).await
    }
    async fn move_file(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        add_parent: &FileId,
        remove_parents: &[FileId],
    ) -> GoogleResult<GoogleFile> {
        GoogleDriveClient::move_file(self, session, file_id, add_parent, remove_parents).await
    }
}
