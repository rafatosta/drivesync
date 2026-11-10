//! Read-only metadata and cursor operations.
use super::*;

impl GoogleDriveClient {
    /// drive.about.get: GET /about. No automatic retry.
    pub async fn about_get(
        &self,
        session: &GoogleSession,
        options: AboutOptions,
    ) -> GoogleResult<GoogleAbout> {
        let query = super::client::query_for(&options, &["fields"])?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(session, reqwest::Method::GET, &["about"], "", query, body)
                .await?,
        )
    }

    /// drive.accessproposals.get: GET /files/{fileId}/accessproposals/{proposalId}. No automatic retry.
    pub async fn access_proposals_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        proposal_id: &ProposalId,
    ) -> GoogleResult<GoogleAccessProposal> {
        let query = Vec::new();
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["files", &file_id.0, "accessproposals", &proposal_id.0],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.accessproposals.list: GET /files/{fileId}/accessproposals. No automatic retry.
    pub async fn access_proposals_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: PageOptions,
    ) -> GoogleResult<GoogleAccessProposalList> {
        let query = super::client::query_for(&options, &["pageToken", "pageSize", "fields"])?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["files", &file_id.0, "accessproposals"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.approvals.get: GET /files/{fileId}/approvals/{approvalId}. No automatic retry.
    pub async fn approvals_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        approval_id: &ApprovalId,
    ) -> GoogleResult<GoogleApproval> {
        let query = Vec::new();
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["files", &file_id.0, "approvals", &approval_id.0],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.approvals.list: GET /files/{fileId}/approvals. No automatic retry.
    pub async fn approvals_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: PageOptions,
    ) -> GoogleResult<GoogleApprovalList> {
        let query = super::client::query_for(&options, &["pageToken", "pageSize", "fields"])?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["files", &file_id.0, "approvals"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.apps.get: GET /apps/{appId}. No automatic retry.
    pub async fn apps_get(
        &self,
        session: &GoogleSession,
        app_id: &AppId,
    ) -> GoogleResult<GoogleApp> {
        let query = Vec::new();
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["apps", &app_id.0],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.apps.list: GET /apps. No automatic retry.
    pub async fn apps_list(
        &self,
        session: &GoogleSession,
        options: AppListOptions,
    ) -> GoogleResult<GoogleAppList> {
        let query = super::client::query_for(
            &options,
            &[
                "languageCode",
                "appFilterExtensions",
                "appFilterMimeTypes",
                "fields",
            ],
        )?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(session, reqwest::Method::GET, &["apps"], "", query, body)
                .await?,
        )
    }

    /// drive.changes.getStartPageToken: GET /changes/startPageToken. No automatic retry.
    pub async fn changes_get_start_page_token(
        &self,
        session: &GoogleSession,
        options: ChangeScopeOptions,
    ) -> GoogleResult<GoogleStartPageToken> {
        let query = super::client::query_for(
            &options,
            &[
                "driveId",
                "teamDriveId",
                "supportsAllDrives",
                "supportsTeamDrives",
                "fields",
            ],
        )?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["changes", "startPageToken"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.changes.list: GET /changes. No automatic retry.
    pub async fn changes_list(
        &self,
        session: &GoogleSession,
        options: ChangeListOptions,
    ) -> GoogleResult<GoogleChangeList> {
        let query = super::client::query_for(
            &options,
            &[
                "spaces",
                "includeCorpusRemovals",
                "includePermissionsForView",
                "supportsTeamDrives",
                "teamDriveId",
                "supportsAllDrives",
                "restrictToMyDrive",
                "driveId",
                "includeItemsFromAllDrives",
                "includeTeamDriveItems",
                "pageSize",
                "includeRemoved",
                "pageToken",
                "includeLabels",
                "fields",
            ],
        )?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(session, reqwest::Method::GET, &["changes"], "", query, body)
                .await
                .map_err(super::client::cursor_failure)?,
        )
    }

    /// drive.comments.get: GET /files/{fileId}/comments/{commentId}. No automatic retry.
    pub async fn comments_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        options: CommentOptions,
    ) -> GoogleResult<GoogleComment> {
        let query = super::client::query_for(&options, &["includeDeleted", "fields"])?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["files", &file_id.0, "comments", &comment_id.0],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.comments.list: GET /files/{fileId}/comments. No automatic retry.
    pub async fn comments_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: CommentListOptions,
    ) -> GoogleResult<GoogleCommentList> {
        let query = super::client::query_for(
            &options,
            &[
                "pageSize",
                "includeDeleted",
                "pageToken",
                "startModifiedTime",
                "fields",
            ],
        )?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["files", &file_id.0, "comments"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.drives.get: GET /drives/{driveId}. No automatic retry.
    pub async fn drives_get(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
        options: DriveGetOptions,
    ) -> GoogleResult<GoogleDrive> {
        let query = super::client::query_for(&options, &["useDomainAdminAccess", "fields"])?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["drives", &drive_id.0],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.drives.list: GET /drives. No automatic retry.
    pub async fn drives_list(
        &self,
        session: &GoogleSession,
        options: DriveListOptions,
    ) -> GoogleResult<GoogleDriveList> {
        let query = super::client::query_for(
            &options,
            &[
                "pageToken",
                "pageSize",
                "useDomainAdminAccess",
                "q",
                "fields",
            ],
        )?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(session, reqwest::Method::GET, &["drives"], "", query, body)
                .await?,
        )
    }

    /// drive.files.generateCseToken: GET /files/generateCseToken. No automatic retry.
    pub async fn files_generate_cse_token(
        &self,
        session: &GoogleSession,
        options: CseTokenOptions,
    ) -> GoogleResult<GoogleCseToken> {
        let query = super::client::query_for(&options, &["fileId", "parent", "fields"])?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["files", "generateCseToken"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.files.generateIds: GET /files/generateIds. No automatic retry.
    pub async fn files_generate_ids(
        &self,
        session: &GoogleSession,
        options: GenerateIdsOptions,
    ) -> GoogleResult<GoogleGeneratedIds> {
        let query = super::client::query_for(&options, &["type", "count", "space", "fields"])?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["files", "generateIds"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.files.list: GET /files. No automatic retry.
    pub async fn files_list(
        &self,
        session: &GoogleSession,
        options: FileListOptions,
    ) -> GoogleResult<GoogleFileList> {
        let query = super::client::query_for(
            &options,
            &[
                "includeItemsFromAllDrives",
                "pageToken",
                "pageSize",
                "includeTeamDriveItems",
                "q",
                "corpora",
                "supportsAllDrives",
                "spaces",
                "supportsTeamDrives",
                "driveId",
                "includePermissionsForView",
                "orderBy",
                "teamDriveId",
                "corpus",
                "includeLabels",
                "fields",
            ],
        )?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(session, reqwest::Method::GET, &["files"], "", query, body)
                .await?,
        )
    }

    /// drive.files.listLabels: GET /files/{fileId}/listLabels. No automatic retry.
    pub async fn files_list_labels(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: LabelListOptions,
    ) -> GoogleResult<GoogleLabelList> {
        let mut query =
            super::client::query_for(&options, &["pageToken", "maxResults", "pageSize", "fields"])?;
        for (key, _) in &mut query {
            if key == "pageSize" {
                *key = "maxResults".into();
            }
        }
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["files", &file_id.0, "listLabels"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.operations.get: GET /operations/{name}. No automatic retry.
    pub async fn operations_get(
        &self,
        session: &GoogleSession,
        name: &OperationName,
    ) -> GoogleResult<GoogleOperation> {
        let query = Vec::new();
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &[
                        "operations",
                        name.0.strip_prefix("operations/").unwrap_or(&name.0),
                    ],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.permissions.get: GET /files/{fileId}/permissions/{permissionId}. No automatic retry.
    pub async fn permissions_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        permission_id: &PermissionId,
        options: PermissionGetOptions,
    ) -> GoogleResult<GooglePermission> {
        let query = super::client::query_for(
            &options,
            &[
                "supportsTeamDrives",
                "useDomainAdminAccess",
                "supportsAllDrives",
                "fields",
            ],
        )?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["files", &file_id.0, "permissions", &permission_id.0],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.permissions.list: GET /files/{fileId}/permissions. No automatic retry.
    pub async fn permissions_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: PermissionListOptions,
    ) -> GoogleResult<GooglePermissionList> {
        let query = super::client::query_for(
            &options,
            &[
                "useDomainAdminAccess",
                "includePermissionsForView",
                "pageToken",
                "supportsTeamDrives",
                "pageSize",
                "supportsAllDrives",
                "fields",
            ],
        )?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["files", &file_id.0, "permissions"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.replies.get: GET /files/{fileId}/comments/{commentId}/replies/{replyId}. No automatic retry.
    pub async fn replies_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        reply_id: &ReplyId,
        options: ReplyOptions,
    ) -> GoogleResult<GoogleReply> {
        let query = super::client::query_for(&options, &["includeDeleted", "fields"])?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
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

    /// drive.replies.list: GET /files/{fileId}/comments/{commentId}/replies. No automatic retry.
    pub async fn replies_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        comment_id: &CommentId,
        options: ReplyListOptions,
    ) -> GoogleResult<GoogleReplyList> {
        let query = super::client::query_for(
            &options,
            &["includeDeleted", "pageSize", "pageToken", "fields"],
        )?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["files", &file_id.0, "comments", &comment_id.0, "replies"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.revisions.get: GET /files/{fileId}/revisions/{revisionId}. No automatic retry.
    pub async fn revisions_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        revision_id: &RevisionId,
        options: RevisionGetOptions,
    ) -> GoogleResult<GoogleRevision> {
        let query = super::client::query_for(&options, &["acknowledgeAbuse", "fields"])?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["files", &file_id.0, "revisions", &revision_id.0],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.revisions.list: GET /files/{fileId}/revisions. No automatic retry.
    pub async fn revisions_list(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: RevisionListOptions,
    ) -> GoogleResult<GoogleRevisionList> {
        let query = super::client::query_for(&options, &["pageToken", "pageSize", "fields"])?;
        let body = None;
        super::client::decode_http(
            &self
                .rest(
                    session,
                    reqwest::Method::GET,
                    &["files", &file_id.0, "revisions"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }
}
