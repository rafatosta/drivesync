//! Typed Google resource projections and endpoint-specific request options.

use super::*;
use serde::{Deserialize, Serialize};

/// Google ApprovalId identifier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ApprovalId(pub String);

/// Google ProposalId identifier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProposalId(pub String);

/// Google AppId identifier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AppId(pub String);

/// Google CommentId identifier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommentId(pub String);

/// Google ReplyId identifier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReplyId(pub String);

/// Google OperationName identifier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OperationName(pub String);

/// Google ChannelId identifier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChannelId(pub String);

/// Google RequestId identifier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(pub String);

/// Explicit partial-write intent. Unchanged fields are omitted by their parent.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum GooglePatch<T> {
    #[default]
    Unchanged,
    Set(T),
    Clear,
}
impl<T> GooglePatch<T> {
    fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged)
    }
}
impl<T: Serialize> Serialize for GooglePatch<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Set(value) => value.serialize(serializer),
            _ => serializer.serialize_none(),
        }
    }
}

/// General pagination options; sent only when supported by a method.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for About.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAbout {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<GoogleUser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_quota: Option<GoogleStorageQuota>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import_formats: Option<std::collections::BTreeMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_formats: Option<std::collections::BTreeMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_import_sizes: Option<std::collections::BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_installed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_color_palette: Option<Vec<String>>,
}

/// Typed contract for StorageQuota.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleStorageQuota {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_in_drive: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_in_drive_trash: Option<String>,
}

/// Typed contract for FileList.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleFileList {
    #[serde(default)]
    pub files: Vec<GoogleFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incomplete_search: Option<bool>,
}

/// Typed contract for Change.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleChange {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<FileId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<GoogleFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive_id: Option<DriveId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive: Option<GoogleDrive>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_id: Option<String>,
}

/// Typed contract for ChangeList.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleChangeList {
    #[serde(default)]
    pub changes: Vec<GoogleChange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_start_page_token: Option<String>,
}

/// Typed contract for StartPageToken.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleStartPageToken {
    pub start_page_token: String,
}

/// Typed contract for Drive.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDrive {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<DriveId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restrictions: Option<GoogleDriveRestrictions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<GoogleDriveCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_rgb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_image_link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_unit_id: Option<String>,
}

/// Typed contract for DriveRestrictions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDriveRestrictions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy_requires_writer_permission: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_users_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive_members_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_managed_restrictions: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sharing_folders_requires_organizer_permission: Option<bool>,
}

/// Typed contract for DriveCapabilities.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDriveCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_add_children: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_comment: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_copy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_delete_drive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_download: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_edit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_list_children: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_manage_members: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_read_revisions: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_rename: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_share: Option<bool>,
}

/// Typed contract for Revision.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleRevision {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RevisionId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<MimeType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_forever: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5_checksum: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_auto: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_outside_domain: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modifying_user: Option<GoogleUser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_links: Option<std::collections::BTreeMap<String, String>>,
}

/// Typed contract for Comment.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleComment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<CommentId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<GoogleUser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replies: Option<Vec<GoogleReply>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_file_content: Option<GoogleQuotedContent>,
}

/// Typed contract for QuotedContent.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleQuotedContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Typed contract for Reply.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleReply {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<ReplyId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<GoogleUser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
}

/// Typed contract for Channel.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleChannel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<ChannelId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<std::collections::BTreeMap<String, String>>,
}
impl std::fmt::Debug for GoogleChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleChannel").finish_non_exhaustive()
    }
}

/// Typed contract for App.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleApp {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<AppId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_url_template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_create: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_import: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorized: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_by_default: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_mime_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary_mime_types: Option<Vec<String>>,
}

/// Typed contract for AccessProposal.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAccessProposal {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<FileId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal_id: Option<ProposalId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requester_email_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient_email_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roles_and_views: Option<Vec<GoogleRoleAndView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_time: Option<String>,
}

/// Typed contract for RoleAndView.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleRoleAndView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
}

/// Typed contract for Approval.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleApproval {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<ApprovalId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_file_id: Option<FileId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modify_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complete_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initiator: Option<GoogleUser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewer_responses: Option<Vec<GoogleApprovalReviewerResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_content_change_behavior: Option<String>,
}

/// Typed contract for ApprovalReviewerResponse.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleApprovalReviewerResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<GoogleUser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
}

/// Typed contract for Operation.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleOperation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<OperationName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<GoogleOperationMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<GoogleDownloadFileResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<GoogleRpcStatus>,
}
impl std::fmt::Debug for GoogleOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleOperation").finish_non_exhaustive()
    }
}

/// Typed contract for OperationMetadata.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleOperationMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_percent: Option<i32>,
}

/// Typed contract for RpcStatus.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleRpcStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<GoogleRpcDetail>>,
}
impl std::fmt::Debug for GoogleRpcStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleRpcStatus").finish_non_exhaustive()
    }
}

/// Typed contract for RpcDetail.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleRpcDetail {
    #[serde(rename = "@type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::BTreeMap<String, String>>,
}
impl std::fmt::Debug for GoogleRpcDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleRpcDetail").finish_non_exhaustive()
    }
}

/// Typed contract for DownloadFileResponse.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDownloadFileResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_download_allowed: Option<bool>,
}
impl std::fmt::Debug for GoogleDownloadFileResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleDownloadFileResponse")
            .finish_non_exhaustive()
    }
}

/// Typed contract for CseToken.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleCseToken {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<FileId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_kacls_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_kacls_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jwt: Option<String>,
}
impl std::fmt::Debug for GoogleCseToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleCseToken").finish_non_exhaustive()
    }
}

/// Typed contract for GeneratedIds.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleGeneratedIds {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ids: Option<Vec<FileId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space: Option<String>,
}

/// Typed contract for ModifyLabelsResponse.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleModifyLabelsResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_labels: Option<Vec<GoogleLabel>>,
}

/// Typed contract for FileWrite.
#[derive(Clone, Default, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleFileWrite {
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub name: GooglePatch<String>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub mime_type: GooglePatch<MimeType>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub description: GooglePatch<String>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub trashed: GooglePatch<bool>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub parents: GooglePatch<Vec<FileId>>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub starred: GooglePatch<bool>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub properties: GooglePatch<std::collections::BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub app_properties: GooglePatch<std::collections::BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub copy_requires_writer_permission: GooglePatch<bool>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub folder_color_rgb: GooglePatch<String>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub modified_time: GooglePatch<String>,
}

/// Typed contract for DriveWrite.
#[derive(Clone, Default, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDriveWrite {
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub name: GooglePatch<String>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub restrictions: GooglePatch<GoogleDriveRestrictions>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub color_rgb: GooglePatch<String>,
}

/// Typed contract for PermissionWrite.
#[derive(Clone, Default, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GooglePermissionWrite {
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub type_: GooglePatch<String>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub role: GooglePatch<String>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub email_address: GooglePatch<String>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub domain: GooglePatch<String>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub allow_file_discovery: GooglePatch<bool>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub expiration_time: GooglePatch<String>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub view: GooglePatch<String>,
}

/// Typed contract for CommentWrite.
#[derive(Clone, Default, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleCommentWrite {
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub content: GooglePatch<String>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub anchor: GooglePatch<String>,
}

/// Typed contract for ReplyWrite.
#[derive(Clone, Default, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleReplyWrite {
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub content: GooglePatch<String>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub action: GooglePatch<String>,
}

/// Typed contract for RevisionWrite.
#[derive(Clone, Default, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleRevisionWrite {
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub keep_forever: GooglePatch<bool>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub published: GooglePatch<bool>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub publish_auto: GooglePatch<bool>,
    #[serde(skip_serializing_if = "GooglePatch::is_unchanged")]
    pub published_outside_domain: GooglePatch<bool>,
}

/// Typed contract for AccessProposalList.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAccessProposalList {
    #[serde(default)]
    pub access_proposals: Vec<GoogleAccessProposal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Typed contract for AppList.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAppList {
    #[serde(default)]
    pub items: Vec<GoogleApp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Typed contract for ApprovalList.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleApprovalList {
    #[serde(default)]
    pub items: Vec<GoogleApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Typed contract for CommentList.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleCommentList {
    #[serde(default)]
    pub comments: Vec<GoogleComment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Typed contract for DriveList.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDriveList {
    #[serde(default)]
    pub drives: Vec<GoogleDrive>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Typed contract for LabelList.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleLabelList {
    #[serde(default)]
    pub labels: Vec<GoogleLabel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Typed contract for PermissionList.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GooglePermissionList {
    #[serde(default)]
    pub permissions: Vec<GooglePermission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Typed contract for ReplyList.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleReplyList {
    #[serde(default)]
    pub replies: Vec<GoogleReply>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Typed contract for RevisionList.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleRevisionList {
    #[serde(default)]
    pub revisions: Vec<GoogleRevision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Typed contract for AboutOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AboutOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for AppListOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppListOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_filter_extensions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_filter_mime_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for ChangeScopeOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChangeScopeOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive_id: Option<DriveId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_all_drives: Option<bool>,
}

/// Typed contract for ChangeListOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChangeListOptions {
    pub page_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive_id: Option<DriveId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spaces: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_removed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_corpus_removals: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_items_from_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for ChangeWatchOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChangeWatchOptions {
    pub page_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive_id: Option<DriveId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spaces: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_removed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_items_from_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for CommentOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CommentOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_deleted: Option<bool>,
}

/// Typed contract for CommentListOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CommentListOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_deleted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_modified_time: Option<String>,
}

/// Typed contract for CseTokenOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CseTokenOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<FileId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<FileId>,
}

/// Typed contract for DriveDeleteOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DriveDeleteOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_item_deletion: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_domain_admin_access: Option<bool>,
}

/// Typed contract for DriveGetOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DriveGetOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_domain_admin_access: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for DriveListOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DriveListOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_domain_admin_access: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for DriveUpdateOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DriveUpdateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_domain_admin_access: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for EmptyTrashOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EmptyTrashOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive_id: Option<DriveId>,
}

/// Typed contract for FileCopyOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileCopyOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_default_visibility: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for FileCreateOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileCreateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_default_visibility: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_content_as_indexable_text: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_revision_forever: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for FileDeleteOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileDeleteOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enforce_single_parent: Option<bool>,
}

/// Typed contract for FileDownloadOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileDownloadOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<MimeType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision_id: Option<RevisionId>,
}

/// Typed contract for FileListOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileListOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corpora: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drive_id: Option<DriveId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spaces: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_items_from_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_permissions_for_view: Option<String>,
}

/// Typed contract for FileUpdateOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileUpdateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_parents: Option<Vec<FileId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_parents: Option<Vec<FileId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_revision_forever: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_content_as_indexable_text: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for FileWatchOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileWatchOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acknowledge_abuse: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_all_drives: Option<bool>,
}

/// Typed contract for GenerateIdsOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenerateIdsOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space: Option<String>,
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
}

/// Typed contract for LabelListOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LabelListOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_token: Option<String>,
}

/// Typed contract for PermissionCreateOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PermissionCreateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_notification_email: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transfer_ownership: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_domain_admin_access: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for PermissionDeleteOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PermissionDeleteOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_domain_admin_access: Option<bool>,
}

/// Typed contract for PermissionGetOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PermissionGetOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_domain_admin_access: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for PermissionListOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PermissionListOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_domain_admin_access: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for PermissionUpdateOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PermissionUpdateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_all_drives: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_domain_admin_access: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_expiration: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transfer_ownership: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
}

/// Typed contract for ReplyOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ReplyOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_deleted: Option<bool>,
}

/// Typed contract for ReplyListOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ReplyListOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_deleted: Option<bool>,
}

/// Typed contract for RevisionGetOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RevisionGetOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acknowledge_abuse: Option<bool>,
}

/// Typed contract for RevisionListOptions.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RevisionListOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldMask>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_token: Option<String>,
}

/// Typed contract for ChannelRequest.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleChannelRequest {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<std::collections::BTreeMap<String, String>>,
}
impl std::fmt::Debug for GoogleChannelRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleChannelRequest")
            .finish_non_exhaustive()
    }
}

/// Typed contract for ChannelStopRequest.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleChannelStopRequest {
    pub id: ChannelId,
    pub resource_id: String,
}

/// Typed contract for ModifyLabelsRequest.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleModifyLabelsRequest {
    #[serde(default)]
    pub label_modifications: Vec<GoogleLabelModification>,
}

/// Typed contract for LabelModification.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleLabelModification {
    pub label_id: String,
    #[serde(default)]
    pub field_modifications: Vec<GoogleLabelFieldModification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_label: Option<bool>,
}

/// Typed contract for LabelFieldModification.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleLabelFieldModification {
    pub field_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_text_values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_integer_values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_date_values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_selection_values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unset_values: Option<bool>,
}

/// Typed contract for ApproveApprovalRequest.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApproveApprovalRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Typed contract for CancelApprovalRequest.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CancelApprovalRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Typed contract for CommentApprovalRequest.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CommentApprovalRequest {
    pub message: String,
}

/// Typed contract for DeclineApprovalRequest.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeclineApprovalRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Typed contract for ReassignApprovalRequest.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ReassignApprovalRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replace_reviewers: Option<Vec<GoogleReplaceReviewer>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_reviewers: Option<Vec<GoogleAddReviewer>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Typed contract for ReplaceReviewer.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleReplaceReviewer {
    pub removed_reviewer_email: String,
    pub added_reviewer_email: String,
}

/// Typed contract for AddReviewer.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAddReviewer {
    pub added_reviewer_email: String,
}

/// Typed contract for StartApprovalRequest.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StartApprovalRequest {
    #[serde(default)]
    pub reviewer_emails: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock_file: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_content_change_behavior: Option<String>,
}

/// Typed contract for ApprovalReviewer.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GoogleApprovalReviewer {
    pub email_address: String,
}

/// Typed contract for ResolveAccessProposalRequest.
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResolveAccessProposalRequest {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_notification: Option<bool>,
}

impl GoogleFileWrite {
    /// Updating parents in the JSON body can be ignored remotely. Require the
    /// official addParents/removeParents query mechanism instead.
    pub(super) fn validate_update(&self) -> GoogleResult<()> {
        if !matches!(self.parents, GooglePatch::Unchanged) {
            return Err(super::client::invalid_argument());
        }
        Ok(())
    }
}

/// General typed page retained by the public contract. Endpoint-specific pages
/// expose their official collection names and completeness fields separately.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GooglePage<T> {
    pub items: Vec<T>,
    pub next_page_token: Option<String>,
}
