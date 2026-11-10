//! Direct REST mutations without retries or synchronization policy.
use super::*;

impl GoogleDriveClient {
    /// drive.drives.create: POST /drives. No automatic retry.
    pub async fn drives_create(
        &self,
        session: &GoogleSession,
        request_id: &RequestId,
        request: GoogleDriveWrite,
    ) -> GoogleResult<GoogleDrive> {
        let query = vec![("requestId".into(), request_id.0.clone())];
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode(
            &self
                .rest(session, reqwest::Method::POST, &["drives"], "", query, body)
                .await?,
        )
    }

    /// drive.drives.delete: DELETE /drives/{driveId}. No automatic retry.
    pub async fn drives_delete(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
        options: DriveDeleteOptions,
    ) -> GoogleResult<()> {
        let query = super::client::query_for(
            &options,
            &["useDomainAdminAccess", "allowItemDeletion", "fields"],
        )?;
        let body = None;
        let _ = self
            .rest(
                session,
                reqwest::Method::DELETE,
                &["drives", &drive_id.0],
                "",
                query,
                body,
            )
            .await?;
        Ok(())
    }

    /// drive.drives.hide: POST /drives/{driveId}/hide. No automatic retry.
    pub async fn drives_hide(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
    ) -> GoogleResult<GoogleDrive> {
        let query = Vec::new();
        let body = None;
        super::client::decode(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["drives", &drive_id.0, "hide"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.drives.unhide: POST /drives/{driveId}/unhide. No automatic retry.
    pub async fn drives_unhide(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
    ) -> GoogleResult<GoogleDrive> {
        let query = Vec::new();
        let body = None;
        super::client::decode(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["drives", &drive_id.0, "unhide"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.drives.update: PATCH /drives/{driveId}. No automatic retry.
    pub async fn drives_update(
        &self,
        session: &GoogleSession,
        drive_id: &DriveId,
        request: GoogleDriveWrite,
        options: DriveUpdateOptions,
    ) -> GoogleResult<GoogleDrive> {
        let query = super::client::query_for(&options, &["useDomainAdminAccess", "fields"])?;
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode(
            &self
                .rest(
                    session,
                    reqwest::Method::PATCH,
                    &["drives", &drive_id.0],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.files.copy: POST /files/{fileId}/copy. No automatic retry.
    pub async fn files_copy(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleFileWrite,
        options: FileCopyOptions,
    ) -> GoogleResult<GoogleFile> {
        let query = super::client::query_for(
            &options,
            &[
                "keepRevisionForever",
                "ocrLanguage",
                "supportsTeamDrives",
                "enforceSingleParent",
                "includePermissionsForView",
                "includeLabels",
                "supportsAllDrives",
                "ignoreDefaultVisibility",
                "copyComments",
                "fields",
            ],
        )?;
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["files", &file_id.0, "copy"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.files.create: POST /files. No automatic retry.
    pub async fn files_create(
        &self,
        session: &GoogleSession,
        request: GoogleFileWrite,
        options: FileCreateOptions,
    ) -> GoogleResult<GoogleFile> {
        let query = super::client::query_for(
            &options,
            &[
                "supportsTeamDrives",
                "ocrLanguage",
                "keepRevisionForever",
                "includeLabels",
                "enforceSingleParent",
                "useContentAsIndexableText",
                "ignoreDefaultVisibility",
                "includePermissionsForView",
                "supportsAllDrives",
                "fields",
            ],
        )?;
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode(
            &self
                .rest(session, reqwest::Method::POST, &["files"], "", query, body)
                .await?,
        )
    }

    /// drive.files.delete: DELETE /files/{fileId}. No automatic retry.
    pub async fn files_delete(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: FileDeleteOptions,
    ) -> GoogleResult<()> {
        let query = super::client::query_for(
            &options,
            &[
                "supportsAllDrives",
                "enforceSingleParent",
                "supportsTeamDrives",
                "fields",
            ],
        )?;
        let body = None;
        let _ = self
            .rest(
                session,
                reqwest::Method::DELETE,
                &["files", &file_id.0],
                "",
                query,
                body,
            )
            .await?;
        Ok(())
    }

    /// drive.files.download: POST /files/{fileId}/download. No automatic retry.
    pub async fn files_download(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: FileDownloadOptions,
    ) -> GoogleResult<GoogleOperation> {
        let query = super::client::query_for(&options, &["revisionId", "mimeType", "fields"])?;
        let body = None;
        super::client::decode(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["files", &file_id.0, "download"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.files.emptyTrash: DELETE /files/trash. No automatic retry.
    pub async fn files_empty_trash(
        &self,
        session: &GoogleSession,
        options: EmptyTrashOptions,
    ) -> GoogleResult<()> {
        let query =
            super::client::query_for(&options, &["driveId", "enforceSingleParent", "fields"])?;
        let body = None;
        let _ = self
            .rest(
                session,
                reqwest::Method::DELETE,
                &["files", "trash"],
                "",
                query,
                body,
            )
            .await?;
        Ok(())
    }

    /// drive.files.modifyLabels: POST /files/{fileId}/modifyLabels. No automatic retry.
    pub async fn files_modify_labels(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleModifyLabelsRequest,
    ) -> GoogleResult<GoogleModifyLabelsResponse> {
        let query = Vec::new();
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["files", &file_id.0, "modifyLabels"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.files.update: PATCH /files/{fileId}. No automatic retry.
    pub async fn files_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleFileWrite,
        options: FileUpdateOptions,
    ) -> GoogleResult<GoogleFile> {
        request.validate_update()?;
        let query = super::client::query_for(
            &options,
            &[
                "useContentAsIndexableText",
                "ocrLanguage",
                "supportsAllDrives",
                "supportsTeamDrives",
                "addParents",
                "includePermissionsForView",
                "removeParents",
                "enforceSingleParent",
                "keepRevisionForever",
                "includeLabels",
                "fields",
            ],
        )?;
        let body =
            Some(serde_json::to_vec(&request).map_err(|_| super::client::invalid_argument())?);
        super::client::decode(
            &self
                .rest(
                    session,
                    reqwest::Method::PATCH,
                    &["files", &file_id.0],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }

    /// drive.files.watch: POST /files/{fileId}/watch. No automatic retry.
    pub async fn files_watch(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: FileWatchOptions,
        channel: GoogleChannelRequest,
    ) -> GoogleResult<GoogleChannel> {
        let query = super::client::query_for(
            &options,
            &[
                "supportsTeamDrives",
                "supportsAllDrives",
                "includeLabels",
                "includePermissionsForView",
                "fields",
            ],
        )?;
        let body =
            Some(serde_json::to_vec(&channel).map_err(|_| super::client::invalid_argument())?);
        super::client::decode(
            &self
                .rest(
                    session,
                    reqwest::Method::POST,
                    &["files", &file_id.0, "watch"],
                    "",
                    query,
                    body,
                )
                .await?,
        )
    }
}
