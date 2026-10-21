//! Read-only Drive folder metadata for initial setup. No file transfers.
use crate::{configuration::valid_id, AuthError, DriveFolder, EngineError};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{io::Read, time::Duration};
const MIME: &str = "application/vnd.google-apps.folder";
const MAX_BODY: u64 = 1024 * 1024;
#[derive(Debug, Clone)]
pub struct FolderPage {
    pub folders: Vec<DriveFolder>,
    pub next_page: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    id: String,
    name: String,
    mime_type: String,
    trashed: bool,
}
impl Metadata {
    fn folder(self) -> Result<DriveFolder, EngineError> {
        let folder = DriveFolder {
            id: self.id,
            name: self.name,
        };
        if self.mime_type != MIME || self.trashed || !folder.valid() {
            return Err(EngineError::RemoteFolderUnavailable);
        }
        Ok(folder)
    }
}
fn request(token: &str, endpoint: &str, params: &[(&str, &str)]) -> Result<Vec<u8>, EngineError> {
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .user_agent("DriveSync/0.1")
        .build()
        .map_err(|_| EngineError::Authentication(AuthError::Network))?;
    let response = client
        .get(endpoint)
        .bearer_auth(token)
        .query(params)
        .send()
        .map_err(|_| EngineError::Authentication(AuthError::Network))?;
    let code = response.status().as_u16();
    let mut body = Vec::new();
    response
        .take(MAX_BODY + 1)
        .read_to_end(&mut body)
        .map_err(|_| EngineError::Authentication(AuthError::Network))?;
    if body.len() as u64 > MAX_BODY {
        return Err(EngineError::Authentication(AuthError::InvalidResponse));
    }
    response_status(code, &body)?;
    Ok(body)
}
fn response_status(code: u16, body: &[u8]) -> Result<(), EngineError> {
    if code != 403 {
        return status(code);
    }
    Err(forbidden_reason(body))
}
fn forbidden_reason(body: &[u8]) -> EngineError {
    // Read only allowlisted reason codes. Never show provider messages, links,
    // metadata or raw response bodies, which can include private identifiers.
    #[derive(Deserialize)]
    struct Reason {
        reason: Option<String>,
    }
    #[derive(Deserialize)]
    struct Error {
        #[serde(default)]
        errors: Vec<Reason>,
        #[serde(default)]
        details: Vec<Reason>,
    }
    #[derive(Deserialize)]
    struct Envelope {
        error: Error,
    }
    let Ok(envelope) = serde_json::from_slice::<Envelope>(body) else {
        return EngineError::RemoteForbidden;
    };
    let reasons: Vec<_> = envelope
        .error
        .errors
        .iter()
        .chain(envelope.error.details.iter())
        .filter_map(|r| r.reason.as_deref())
        .collect();
    if reasons.iter().any(|r| {
        matches!(
            *r,
            "SERVICE_DISABLED" | "accessNotConfigured" | "serviceDisabled"
        )
    }) {
        EngineError::DriveApiDisabled
    } else if reasons.iter().any(|r| {
        matches!(
            *r,
            "ACCESS_TOKEN_SCOPE_INSUFFICIENT" | "insufficientPermissions"
        )
    }) {
        EngineError::Authentication(AuthError::MissingPermissions)
    } else if reasons.iter().any(|r| {
        matches!(
            *r,
            "RATE_LIMIT_EXCEEDED"
                | "QUOTA_EXCEEDED"
                | "rateLimitExceeded"
                | "userRateLimitExceeded"
                | "dailyLimitExceeded"
                | "quotaExceeded"
        )
    }) {
        EngineError::RemoteRateLimited
    } else {
        EngineError::RemoteForbidden
    }
}
fn status(code: u16) -> Result<(), EngineError> {
    Err(match code {
        200 => return Ok(()),
        401 => EngineError::Authentication(AuthError::ReauthorizationRequired),
        403 => EngineError::RemoteForbidden,
        404 => EngineError::RemoteFolderUnavailable,
        429 => EngineError::RemoteRateLimited,
        500..=599 => EngineError::Authentication(AuthError::ProviderUnavailable),
        _ => EngineError::Authentication(AuthError::InvalidResponse),
    })
}
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
fn decode_folder(body: &[u8]) -> Result<DriveFolder, EngineError> {
    serde_json::from_slice::<Metadata>(body)
        .map_err(|_| EngineError::Authentication(AuthError::InvalidResponse))?
        .folder()
}
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
fn decode_page(body: &[u8]) -> Result<FolderPage, EngineError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Page {
        files: Vec<Metadata>,
        next_page_token: Option<String>,
        #[serde(default)]
        incomplete_search: bool,
    }
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folders_and_pagination_require_real_folder_metadata() {
        let page = decode_page(br#"{"files":[{"id":"folder-A","name":"Projects","mimeType":"application/vnd.google-apps.folder","trashed":false}],"nextPageToken":"page-2"}"#).unwrap();
        assert_eq!(page.folders[0].id, "folder-A");
        assert_eq!(page.next_page.as_deref(), Some("page-2"));
        assert!(decode_page(br#"{"files":[],"incompleteSearch":true}"#).is_err());
        assert!(decode_folder(
            br#"{"id":"x","name":"file","mimeType":"text/plain","trashed":false}"#
        )
        .is_err());
        assert!(decode_folder(br#"{"id":"x","name":"trash","mimeType":"application/vnd.google-apps.folder","trashed":true}"#).is_err());
    }
    #[test]
    fn provider_failures_are_distinct_and_sanitized() {
        assert_eq!(status(403), Err(EngineError::RemoteForbidden));
        assert_eq!(status(404), Err(EngineError::RemoteFolderUnavailable));
        assert_eq!(status(429), Err(EngineError::RemoteRateLimited));
        assert_eq!(
            status(401),
            Err(EngineError::Authentication(
                AuthError::ReauthorizationRequired
            ))
        );
        assert!(status(503).is_err());
    }
}

#[cfg(test)]
mod permission_tests {
    use super::*;
    #[test]
    fn forbidden_reasons_distinguish_disabled_api_scope_quota_and_file_access() {
        for body in [br#"{"error":{"errors":[{"reason":"accessNotConfigured"}]}}"#.as_slice(), br#"{"error":{"details":[{"reason":"SERVICE_DISABLED","metadata":{"private":"must-not-leak"}}]}}"#] {
            assert_eq!(response_status(403, body), Err(EngineError::DriveApiDisabled));
        }
        assert_eq!(
            response_status(
                403,
                br#"{"error":{"details":[{"reason":"ACCESS_TOKEN_SCOPE_INSUFFICIENT"}]}}"#
            ),
            Err(EngineError::Authentication(AuthError::MissingPermissions))
        );
        assert_eq!(
            response_status(
                403,
                br#"{"error":{"errors":[{"reason":"insufficientPermissions"}]}}"#
            ),
            Err(EngineError::Authentication(AuthError::MissingPermissions))
        );
        assert_eq!(
            response_status(
                403,
                br#"{"error":{"errors":[{"reason":"userRateLimitExceeded"}]}}"#
            ),
            Err(EngineError::RemoteRateLimited)
        );
        for body in [br#"{"error":{"errors":[{"reason":"insufficientFilePermissions"}],"message":"private-file-name"}}"#.as_slice(), b"not JSON", br#"{"error":{"message":"SERVICE_DISABLED"}}"#] {
            assert_eq!(response_status(403, body), Err(EngineError::RemoteForbidden));
        }
        assert_eq!(
            response_status(401, b""),
            Err(EngineError::Authentication(
                AuthError::ReauthorizationRequired
            ))
        );
        assert!(response_status(200, b"{}").is_ok());
    }
}
