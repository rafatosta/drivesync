//! Read-only lookup and listing of Google Drive folders.
//!
//! Validates remote identifiers, page tokens, metadata and pagination before
//! returning non-trashed Drive folders. This module does not transfer files.
use super::{
    client::request,
    models::{FolderPage, Metadata, Page, MIME},
};
use crate::{configuration::valid_id, AuthError, DriveFolder, EngineError};

/// Resolves and validates one Drive folder by ID.
///
/// The ID is checked before constructing the Drive URL. The returned
/// metadata must describe a non-trashed folder.
///
/// # Errors
///
/// Returns [`EngineError::RemoteFolderUnavailable`] for invalid IDs or
/// folder metadata, and propagates typed HTTP/authorization errors.
pub(crate) fn get(token: &str, id: &str) -> Result<DriveFolder, EngineError> {
    if !valid_id(id) {
        return Err(EngineError::RemoteFolderUnavailable);
    }
    let body = request(
        token,
        &format!("https://www.googleapis.com/drive/v3/files/{id}"),
        &[
            ("fields", "id,name,mimeType,trashed"),
            ("supportsAllDrives", "true"),
        ],
    )?;
    decode_folder(&body)
}
/// Converts a JSON response to a validated folder.
///
/// # Errors
///
/// Returns a typed error for malformed JSON or non-folder metadata.
pub(super) fn decode_folder(body: &[u8]) -> Result<DriveFolder, EngineError> {
    serde_json::from_slice::<Metadata>(body)
        .map_err(|_| EngineError::Authentication(AuthError::InvalidResponse))?
        .folder()
}
/// Lists a page of child folders within a Drive directory.
///
/// Accepts an opaque optional page token, applies Drive's folder-only query,
/// and rejects incomplete searches and invalid pagination metadata.
///
/// # Errors
///
/// Returns [`EngineError::RemoteFolderUnavailable`] for invalid parent IDs
/// or page-token inputs; other errors describe network, permission or
/// malformed provider responses.
pub(crate) fn list(
    token: &str,
    parent: &str,
    page: Option<&str>,
) -> Result<FolderPage, EngineError> {
    if !valid_id(parent) || page.is_some_and(|p| p.len() > 4096 || p.chars().any(char::is_control))
    {
        return Err(EngineError::RemoteFolderUnavailable);
    }
    let query = format!("'{parent}' in parents and mimeType = '{MIME}' and trashed = false");
    let mut params = vec![
        ("q", query.as_str()),
        (
            "fields",
            "nextPageToken,incompleteSearch,files(id,name,mimeType,trashed)",
        ),
        ("pageSize", "200"),
        ("orderBy", "name_natural"),
        ("supportsAllDrives", "true"),
        ("includeItemsFromAllDrives", "true"),
    ];
    if let Some(page) = page {
        params.push(("pageToken", page));
    }
    decode_page(&request(
        token,
        "https://www.googleapis.com/drive/v3/files",
        &params,
    )?)
}
/// Validates and decodes a Drive folder page.
///
/// # Errors
///
/// Returns typed errors for invalid JSON, incomplete searches, malformed
/// pagination tokens or entries that are not usable folders.
pub(super) fn decode_page(body: &[u8]) -> Result<FolderPage, EngineError> {
    let page: Page = serde_json::from_slice(body)
        .map_err(|_| EngineError::Authentication(AuthError::InvalidResponse))?;
    if page.incomplete_search
        || page
            .next_page_token
            .as_ref()
            .is_some_and(|p| p.is_empty() || p.len() > 4096 || p.chars().any(char::is_control))
    {
        return Err(EngineError::Authentication(AuthError::InvalidResponse));
    }
    Ok(FolderPage {
        folders: page
            .files
            .into_iter()
            .map(Metadata::folder)
            .collect::<Result<_, _>>()?,
        next_page: page.next_page_token,
    })
}
