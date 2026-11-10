//! Public data types of the Google provider (identifiers, session, files).
//!
//! Only the types required by the implemented groups exist so far. Field
//! names follow `docs/engine/providers/google/INTERFACES.md`, verified against
//! the official Drive v3 Discovery document.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

macro_rules! string_id {
    ($($(#[$meta:meta])* $name:ident),+ $(,)?) => {
        $(
            $(#[$meta])*
            #[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
            #[serde(transparent)]
            pub struct $name(pub String);
        )+
    };
}

string_id!(
    /// Drive file or folder identifier.
    FileId,
    /// Shared drive identifier.
    DriveId,
    /// Permission identifier.
    PermissionId,
    /// Revision identifier.
    RevisionId,
    /// MIME type string.
    MimeType,
    /// Partial-response `fields` selector.
    FieldMask,
);

/// Authorized context bound to one account.
///
/// The access token is private and must never appear in `Debug` output,
/// errors or logs.
#[derive(Clone)]
pub struct GoogleSession {
    pub account_id: String,
    pub(super) access_token: String,
}

impl GoogleSession {
    /// Binds an access token to an account identifier.
    pub fn new(account_id: impl Into<String>, access_token: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            access_token: access_token.into(),
        }
    }
}

impl std::fmt::Debug for GoogleSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleSession")
            .field("account_id", &self.account_id)
            .finish_non_exhaustive()
    }
}

/// Query parameters of `files.get`.
///
/// `None` means the parameter is not sent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileGetOptions {
    pub fields: Option<FieldMask>,
    pub supports_all_drives: Option<bool>,
    /// Comma-separated label IDs for `labelInfo`.
    pub include_labels: Option<String>,
    /// Only `published` is supported by the API.
    pub include_permissions_for_view: Option<String>,
}

/// Query string pairs of `files.get`, in official parameter names.
pub(crate) fn file_get_query(options: &FileGetOptions) -> Vec<(String, String)> {
    let mut query = Vec::new();
    if let Some(fields) = &options.fields {
        query.push(("fields".into(), fields.0.clone()));
    }
    if let Some(value) = options.supports_all_drives {
        query.push(("supportsAllDrives".into(), value.to_string()));
    }
    if let Some(value) = &options.include_labels {
        query.push(("includeLabels".into(), value.clone()));
    }
    if let Some(value) = &options.include_permissions_for_view {
        query.push(("includePermissionsForView".into(), value.clone()));
    }
    query
}

/// Decodes a `File` resource from a successful response body.
pub(crate) fn parse_file(body: &[u8]) -> super::GoogleResult<GoogleFile> {
    // Reject non-object roots: serde structs can otherwise accept JSON sequences.
    if body.iter().copied().find(|b| !b.is_ascii_whitespace()) != Some(b'{') {
        return Err(super::GoogleApiFailure::new(
            super::GoogleApiError::InvalidResponse,
            None,
        ));
    }
    serde_json::from_slice(body)
        .map_err(|_| super::GoogleApiFailure::new(super::GoogleApiError::InvalidResponse, None))
}

/// Drive `File` resource. `None` means *not received*, never *absent in Drive*.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleFile {
    pub id: Option<FileId>,
    pub name: Option<String>,
    pub mime_type: Option<MimeType>,
    pub parents: Option<Vec<FileId>>,
    pub description: Option<String>,
    pub trashed: Option<bool>,
    pub explicitly_trashed: Option<bool>,
    pub trashed_time: Option<String>,
    pub version: Option<String>,
    pub created_time: Option<String>,
    pub modified_time: Option<String>,
    pub modified_by_me_time: Option<String>,
    pub viewed_by_me_time: Option<String>,
    pub size: Option<String>,
    pub quota_bytes_used: Option<String>,
    pub md5_checksum: Option<String>,
    pub sha1_checksum: Option<String>,
    pub sha256_checksum: Option<String>,
    pub head_revision_id: Option<RevisionId>,
    pub drive_id: Option<DriveId>,
    pub resource_key: Option<String>,
    pub web_view_link: Option<String>,
    pub web_content_link: Option<String>,
    pub thumbnail_link: Option<String>,
    pub icon_link: Option<String>,
    pub file_extension: Option<String>,
    pub full_file_extension: Option<String>,
    pub original_filename: Option<String>,
    pub shortcut_details: Option<GoogleShortcutDetails>,
    pub capabilities: Option<GoogleFileCapabilities>,
    pub permissions: Option<Vec<GooglePermission>>,
    pub owners: Option<Vec<GoogleUser>>,
    pub last_modifying_user: Option<GoogleUser>,
    pub sharing_user: Option<GoogleUser>,
    pub shared: Option<bool>,
    pub owned_by_me: Option<bool>,
    pub modified_by_me: Option<bool>,
    pub starred: Option<bool>,
    pub folder_color_rgb: Option<String>,
    pub properties: Option<BTreeMap<String, String>>,
    pub app_properties: Option<BTreeMap<String, String>>,
    pub content_hints: Option<GoogleContentHints>,
    pub image_media_metadata: Option<GoogleImageMediaMetadata>,
    pub video_media_metadata: Option<GoogleVideoMediaMetadata>,
    pub label_info: Option<GoogleLabelInfo>,
    pub content_restrictions: Option<Vec<GoogleContentRestriction>>,
    pub download_restrictions: Option<GoogleDownloadRestrictions>,
    pub client_encryption_details: Option<GoogleClientEncryptionDetails>,
    pub copy_requires_writer_permission: Option<bool>,
    pub writers_can_share: Option<bool>,
    pub has_thumbnail: Option<bool>,
    pub spaces: Option<Vec<String>>,
    pub permission_ids: Option<Vec<PermissionId>>,
    pub team_drive_id: Option<String>,
    pub inherited_permissions_disabled: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleShortcutDetails {
    pub target_id: Option<FileId>,
    pub target_mime_type: Option<MimeType>,
    pub target_resource_key: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleFileCapabilities {
    pub can_edit: Option<bool>,
    pub can_copy: Option<bool>,
    pub can_download: Option<bool>,
    pub can_delete: Option<bool>,
    pub can_rename: Option<bool>,
    pub can_move_item_within_drive: Option<bool>,
    pub can_move_item_out_of_drive: Option<bool>,
    pub can_add_children: Option<bool>,
    pub can_list_children: Option<bool>,
    pub can_share: Option<bool>,
    pub can_comment: Option<bool>,
    pub can_read_revisions: Option<bool>,
    pub can_change_copy_requires_writer_permission: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleUser {
    pub display_name: Option<String>,
    pub email_address: Option<String>,
    pub photo_link: Option<String>,
    pub permission_id: Option<String>,
    pub me: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GooglePermission {
    pub id: Option<PermissionId>,
    /// JSON `type`.
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub role: Option<String>,
    pub email_address: Option<String>,
    pub domain: Option<String>,
    pub allow_file_discovery: Option<bool>,
    pub expiration_time: Option<String>,
    pub display_name: Option<String>,
    pub photo_link: Option<String>,
    pub deleted: Option<bool>,
    pub pending_owner: Option<bool>,
    pub permission_details: Option<Vec<GooglePermissionDetails>>,
    pub view: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GooglePermissionDetails {
    pub permission_type: Option<String>,
    pub role: Option<String>,
    pub inherited: Option<bool>,
    pub inherited_from: Option<FileId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleContentHints {
    pub indexable_text: Option<String>,
    pub thumbnail: Option<GoogleThumbnail>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleThumbnail {
    pub image: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleImageMediaMetadata {
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub rotation: Option<i32>,
    pub time: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub exposure_time: Option<f64>,
    pub aperture: Option<f64>,
    pub iso_speed: Option<i32>,
    pub location: Option<GoogleGeoLocation>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleGeoLocation {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleVideoMediaMetadata {
    pub width: Option<i32>,
    pub height: Option<i32>,
    /// `int64` transported as a string.
    pub duration_millis: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleLabelInfo {
    pub labels: Option<Vec<GoogleLabel>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleLabel {
    pub id: Option<String>,
    pub revision_id: Option<String>,
    pub fields: Option<BTreeMap<String, GoogleLabelField>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleLabelField {
    pub value_type: Option<String>,
    pub text: Option<Vec<String>>,
    pub integer: Option<Vec<String>>,
    pub date_string: Option<Vec<String>>,
    pub selection: Option<Vec<String>>,
    pub user: Option<Vec<GoogleUser>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleContentRestriction {
    pub read_only: Option<bool>,
    pub reason: Option<String>,
    pub restricting_user: Option<GoogleUser>,
    pub restriction_time: Option<String>,
    pub owner_restricted: Option<bool>,
    pub system_restricted: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDownloadRestrictions {
    pub item_download_restriction: Option<GoogleDownloadRestriction>,
    /// JSON `effectiveDownloadRestrictionWithContext`.
    pub effective_download_restriction_with_context: Option<GoogleDownloadRestriction>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDownloadRestriction {
    pub restricted_for_readers: Option<bool>,
    pub restricted_for_writers: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleClientEncryptionDetails {
    pub encryption_state: Option<String>,
    pub decryption_metadata: Option<GoogleDecryptionMetadata>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDecryptionMetadata {
    pub wrapped_key: Option<String>,
    pub kacls_id: Option<String>,
}
