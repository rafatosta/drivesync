//! Direct REST mutations without retries or synchronization policy.
use super::*;

impl GoogleDriveClient {
    /// drive.accessproposals.resolve: POST /files/{fileId}/accessproposals/{proposalId}:resolve. No automatic retry.
    pub async fn access_proposals_resolve(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        proposal_id: &ProposalId,
        request: ResolveAccessProposalRequest,
    ) -> GoogleResult<()> {
        let query = Vec::new();
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        let _ = self
            .rest(
                session,
                reqwest::Method::POST,
                &["files", &file_id.0, "accessproposals", &proposal_id.0],
                ":resolve",
                query,
                body,
            )
            .await?;
        Ok(())
    }

    /// drive.approvals.approve: POST /files/{fileId}/approvals/{approvalId}:approve. No automatic retry.
    pub async fn approvals_approve(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: ApproveApprovalRequest,
    ) -> GoogleResult<GoogleApproval> {
        let query = Vec::new();
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["files", &file_id.0, "approvals", &approval_id.0],
                    ":approve",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.approvals.cancel: POST /files/{fileId}/approvals/{approvalId}:cancel. No automatic retry.
    pub async fn approvals_cancel(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: CancelApprovalRequest,
    ) -> GoogleResult<GoogleApproval> {
        let query = Vec::new();
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["files", &file_id.0, "approvals", &approval_id.0],
                    ":cancel",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.approvals.comment: POST /files/{fileId}/approvals/{approvalId}:comment. No automatic retry.
    pub async fn approvals_comment(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: CommentApprovalRequest,
    ) -> GoogleResult<GoogleApproval> {
        let query = Vec::new();
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["files", &file_id.0, "approvals", &approval_id.0],
                    ":comment",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.approvals.decline: POST /files/{fileId}/approvals/{approvalId}:decline. No automatic retry.
    pub async fn approvals_decline(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: DeclineApprovalRequest,
    ) -> GoogleResult<GoogleApproval> {
        let query = Vec::new();
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["files", &file_id.0, "approvals", &approval_id.0],
                    ":decline",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.approvals.reassign: POST /files/{fileId}/approvals/{approvalId}:reassign. No automatic retry.
    pub async fn approvals_reassign(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
        request: ReassignApprovalRequest,
    ) -> GoogleResult<GoogleApproval> {
        let query = Vec::new();
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["files", &file_id.0, "approvals", &approval_id.0],
                    ":reassign",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.approvals.start: POST /files/{fileId}/approvals:start. No automatic retry.
    pub async fn approvals_start(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: StartApprovalRequest,
    ) -> GoogleResult<GoogleApproval> {
        let query = Vec::new();
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["files", &file_id.0, "approvals"],
                    ":start",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.changes.watch: POST /changes/watch. No automatic retry.
    pub async fn changes_watch(
        &self,
        session: &GoogleSession,
        options: ChangeWatchOptions,
        channel: GoogleChannelRequest,
    ) -> GoogleResult<GoogleChannel> {
        let query = super::client::query_for(
            &options,
            &[
                "includePermissionsForView",
                "supportsTeamDrives",
                "includeRemoved",
                "includeCorpusRemovals",
                "includeLabels",
                "spaces",
                "pageToken",
                "includeItemsFromAllDrives",
                "driveId",
                "pageSize",
                "supportsAllDrives",
                "includeTeamDriveItems",
                "restrictToMyDrive",
                "teamDriveId",
                "fields",
            ],
        )?;
        let body =
            Some(serde_json::to_vec(&channel).map_err(|_| super::client::invalid_argument())?);
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["changes", "watch"],
                    "",
                    query,
                    body,
                )
                .await
                .map_err(super::client::cursor_failure)?,
        )
    }

    /// drive.channels.stop: POST /channels/stop. No automatic retry.
    pub async fn channels_stop(
        &self,
        session: &GoogleSession,
        channel: GoogleChannelStopRequest,
    ) -> GoogleResult<()> {
        let query = Vec::new();
        let body =
            Some(serde_json::to_vec(&channel).map_err(|_| super::client::invalid_argument())?);
        let _ = self
            .rest(
                session,
                reqwest::Method::POST,
                &["channels", "stop"],
                "",
                query,
                body,
            )
            .await?;
        Ok(())
    }

    /// drive.comments.create: POST /files/{fileId}/comments. No automatic retry.
    pub async fn comments_create(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleCommentWrite,
        options: CommentOptions,
    ) -> GoogleResult<GoogleComment> {
        let query = super::client::query_for(&options, &["fields"])?;
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["files", &file_id.0, "comments"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.comments.delete: DELETE /files/{fileId}/comments/{commentId}. No automatic retry.
    pub async fn comments_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
    ) -> GoogleResult<()> {
        let query = Vec::new();
        let body = None;
        let _ = self
            .rest(
                session,
                reqwest::Method::DELETE,
                &["files", &file_id.0, "comments", &comment_id.0],
                "",
                query,
                body,
            )
            .await?;
        Ok(())
    }

    /// drive.comments.update: PATCH /files/{fileId}/comments/{commentId}. No automatic retry.
    pub async fn comments_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        request: GoogleCommentWrite,
        options: CommentOptions,
    ) -> GoogleResult<GoogleComment> {
        let query = super::client::query_for(&options, &["fields"])?;
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::PATCH,
                    &["files", &file_id.0, "comments", &comment_id.0],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.permissions.create: POST /files/{fileId}/permissions. No automatic retry.
    pub async fn permissions_create(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GooglePermissionWrite,
        options: PermissionCreateOptions,
    ) -> GoogleResult<GooglePermission> {
        let query = super::client::query_for(
            &options,
            &[
                "useDomainAdminAccess",
                "supportsAllDrives",
                "transferOwnership",
                "moveToNewOwnersRoot",
                "sendNotificationEmail",
                "emailMessage",
                "enforceSingleParent",
                "supportsTeamDrives",
                "enforceExpansiveAccess",
                "fields",
            ],
        )?;
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["files", &file_id.0, "permissions"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.permissions.delete: DELETE /files/{fileId}/permissions/{permissionId}. No automatic retry.
    pub async fn permissions_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        permission_id: &PermissionId,
        options: PermissionDeleteOptions,
    ) -> GoogleResult<()> {
        let query = super::client::query_for(
            &options,
            &[
                "useDomainAdminAccess",
                "supportsTeamDrives",
                "enforceExpansiveAccess",
                "supportsAllDrives",
                "fields",
            ],
        )?;
        let body = None;
        let _ = self
            .rest(
                session,
                reqwest::Method::DELETE,
                &["files", &file_id.0, "permissions", &permission_id.0],
                "",
                query,
                body,
            )
            .await?;
        Ok(())
    }

    /// drive.permissions.update: PATCH /files/{fileId}/permissions/{permissionId}. No automatic retry.
    pub async fn permissions_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        permission_id: &PermissionId,
        request: GooglePermissionWrite,
        options: PermissionUpdateOptions,
    ) -> GoogleResult<GooglePermission> {
        let query = super::client::query_for(
            &options,
            &[
                "transferOwnership",
                "removeExpiration",
                "supportsAllDrives",
                "useDomainAdminAccess",
                "enforceExpansiveAccess",
                "supportsTeamDrives",
                "fields",
            ],
        )?;
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::PATCH,
                    &["files", &file_id.0, "permissions", &permission_id.0],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.replies.create: POST /files/{fileId}/comments/{commentId}/replies. No automatic retry.
    pub async fn replies_create(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        request: GoogleReplyWrite,
        options: ReplyOptions,
    ) -> GoogleResult<GoogleReply> {
        let query = super::client::query_for(&options, &["fields"])?;
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["files", &file_id.0, "comments", &comment_id.0, "replies"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.replies.delete: DELETE /files/{fileId}/comments/{commentId}/replies/{replyId}. No automatic retry.
    pub async fn replies_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        reply_id: &ReplyId,
    ) -> GoogleResult<()> {
        let query = Vec::new();
        let body = None;
        let _ = self
            .rest(
                session,
                reqwest::Method::DELETE,
                &[
                    "files",
                    &file_id.0,
                    "comments",
                    &comment_id.0,
                    "replies",
                    &reply_id.0,
                ],
                "",
                query,
                body,
            )
            .await?;
        Ok(())
    }

    /// drive.replies.update: PATCH /files/{fileId}/comments/{commentId}/replies/{replyId}. No automatic retry.
    pub async fn replies_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        reply_id: &ReplyId,
        request: GoogleReplyWrite,
        options: ReplyOptions,
    ) -> GoogleResult<GoogleReply> {
        let query = super::client::query_for(&options, &["fields"])?;
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::PATCH,
                    &[
                        "files",
                        &file_id.0,
                        "comments",
                        &comment_id.0,
                        "replies",
                        &reply_id.0,
                    ],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.revisions.delete: DELETE /files/{fileId}/revisions/{revisionId}. No automatic retry.
    pub async fn revisions_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        revision_id: &RevisionId,
    ) -> GoogleResult<()> {
        let query = Vec::new();
        let body = None;
        let _ = self
            .rest(
                session,
                reqwest::Method::DELETE,
                &["files", &file_id.0, "revisions", &revision_id.0],
                "",
                query,
                body,
            )
            .await?;
        Ok(())
    }

    /// drive.revisions.update: PATCH /files/{fileId}/revisions/{revisionId}. No automatic retry.
    pub async fn revisions_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        revision_id: &RevisionId,
        request: GoogleRevisionWrite,
    ) -> GoogleResult<GoogleRevision> {
        let query = Vec::new();
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::PATCH,
                    &["files", &file_id.0, "revisions", &revision_id.0],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }
}
