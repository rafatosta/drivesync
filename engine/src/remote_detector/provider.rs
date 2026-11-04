//! Read-only, bounded Google Drive change and file metadata requests.
//!
//! The caller obtains the access token using the existing account-scoped
//! authentication capability. This module never persists or logs it.
use crate::{configuration::valid_id, remote, EngineError};
use serde::{Deserialize, Serialize};

const API: &str = "https://www.googleapis.com/drive/v3";
const FILE_FIELDS: &str =
    "id,name,mimeType,parents,trashed,version,modifiedTime,driveId,capabilities(canListChildren)";

/// Change-log identity. A shared-drive log must be used for a member's drive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Corpus {
    /// The authenticated user's own change log.
    User,
    /// The change log of a shared drive for which the account is a member.
    SharedDrive(String),
}

/// A provider item is a metadata observation, not an operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    /// Stable Drive item identifier.
    pub id: String,
    /// Available name, which is not an identity.
    pub name: Option<String>,
    /// Drive MIME type, when returned.
    pub mime_type: Option<String>,
    /// Observed parent identifiers, when returned.
    pub parents: Option<Vec<String>>,
    /// Trashed state, when returned.
    pub trashed: Option<bool>,
    /// Provider version, when returned.
    pub version: Option<String>,
    /// Provider modification timestamp, when returned.
    pub modified_time: Option<String>,
    /// Shared-drive identifier, when applicable.
    pub drive_id: Option<String>,
    /// Listing capability for folders, when returned.
    pub capabilities: Option<FileCapabilities>,
}

/// Provider-reported folder-listing capability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCapabilities {
    /// Whether this account can list the folder's children.
    pub can_list_children: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Change {
    pub file_id: Option<String>,
    pub file: Option<File>,
    #[serde(default)]
    pub removed: bool,
    pub change_type: Option<String>,
    pub drive_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ChangePage {
    #[serde(default)]
    pub changes: Vec<Change>,
    pub next_page_token: Option<String>,
    pub new_start_page_token: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FilePage {
    #[serde(default)]
    pub files: Vec<File>,
    pub next_page_token: Option<String>,
    #[serde(default)]
    pub incomplete_search: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ProviderError {
    Request(EngineError),
    InvalidResponse,
    RejectedCursor,
    Incomplete,
}

/// Allows deterministic tests without an authenticated network connection.
trait Transport {
    fn get(&self, endpoint: &str, params: &[(&str, &str)]) -> Result<Vec<u8>, ProviderError>;
}

struct Http<'a> {
    token: &'a str,
}

impl Transport for Http<'_> {
    fn get(&self, endpoint: &str, params: &[(&str, &str)]) -> Result<Vec<u8>, ProviderError> {
        let (status, body) =
            remote::request_status(self.token, endpoint, params).map_err(ProviderError::Request)?;
        if endpoint.ends_with("/changes") && status == 410 {
            return Err(ProviderError::RejectedCursor);
        }
        remote::response_status(status, &body).map_err(ProviderError::Request)?;
        Ok(body)
    }
}

/// A provider bound to an already verified account's short-lived access token.
pub(crate) struct GoogleDrive<'a> {
    transport: Http<'a>,
}

impl<'a> GoogleDrive<'a> {
    pub fn new(token: &'a str) -> Self {
        Self {
            transport: Http { token },
        }
    }

    pub(super) fn start(&self, corpus: &Corpus) -> Result<String, ProviderError> {
        start(&self.transport, corpus)
    }

    pub(super) fn changes(
        &self,
        corpus: &Corpus,
        cursor: &str,
    ) -> Result<ChangePage, ProviderError> {
        changes(&self.transport, corpus, cursor)
    }

    pub(super) fn children(
        &self,
        corpus: &Corpus,
        parent: &str,
        page: Option<&str>,
    ) -> Result<FilePage, ProviderError> {
        children(&self.transport, corpus, parent, page)
    }

    pub(super) fn file(&self, id: &str) -> Result<File, ProviderError> {
        file(&self.transport, id)
    }
}

fn valid_token(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control)
}

fn add_corpus<'a>(
    params: &mut Vec<(&'a str, &'a str)>,
    corpus: &'a Corpus,
) -> Result<(), ProviderError> {
    if let Corpus::SharedDrive(id) = corpus {
        if !valid_id(id) {
            return Err(ProviderError::InvalidResponse);
        }
        params.push(("driveId", id));
    }
    Ok(())
}

fn start(transport: &impl Transport, corpus: &Corpus) -> Result<String, ProviderError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Response {
        start_page_token: String,
    }
    let mut params = vec![("supportsAllDrives", "true"), ("fields", "startPageToken")];
    add_corpus(&mut params, corpus)?;
    let body = transport.get(&format!("{API}/changes/startPageToken"), &params)?;
    let result: Response =
        serde_json::from_slice(&body).map_err(|_| ProviderError::InvalidResponse)?;
    if !valid_token(&result.start_page_token) {
        return Err(ProviderError::InvalidResponse);
    }
    Ok(result.start_page_token)
}

fn changes(
    transport: &impl Transport,
    corpus: &Corpus,
    cursor: &str,
) -> Result<ChangePage, ProviderError> {
    if !valid_token(cursor) {
        return Err(ProviderError::RejectedCursor);
    }
    let fields = format!("nextPageToken,newStartPageToken,changes(fileId,removed,changeType,driveId,file({FILE_FIELDS}))");
    let mut params = vec![
        ("pageToken", cursor),
        ("pageSize", "200"),
        ("spaces", "drive"),
        ("includeRemoved", "true"),
        ("includeCorpusRemovals", "true"),
        ("includeItemsFromAllDrives", "true"),
        ("supportsAllDrives", "true"),
        ("fields", fields.as_str()),
    ];
    add_corpus(&mut params, corpus)?;
    let body = transport.get(&format!("{API}/changes"), &params)?;
    let page: ChangePage =
        serde_json::from_slice(&body).map_err(|_| ProviderError::InvalidResponse)?;
    if page
        .next_page_token
        .as_deref()
        .is_some_and(|s| !valid_token(s))
        || page
            .new_start_page_token
            .as_deref()
            .is_some_and(|s| !valid_token(s))
        || page.next_page_token.is_some() == page.new_start_page_token.is_some()
    {
        return Err(ProviderError::InvalidResponse);
    }
    for change in &page.changes {
        if !matches!(change.change_type.as_deref(), Some("file" | "drive"))
            || (change.change_type.as_deref() == Some("file") && change.file_id.is_none())
            || (change.change_type.as_deref() == Some("drive") && change.drive_id.is_none())
            || change.file_id.as_deref().is_some_and(|id| !valid_id(id))
            || change.drive_id.as_deref().is_some_and(|id| !valid_id(id))
            || change.file.as_ref().is_some_and(|f| !valid_id(&f.id))
            || change
                .file
                .as_ref()
                .is_some_and(|f| change.file_id.as_deref() != Some(&f.id))
        {
            return Err(ProviderError::InvalidResponse);
        }
    }
    Ok(page)
}

fn children(
    transport: &impl Transport,
    corpus: &Corpus,
    parent: &str,
    token: Option<&str>,
) -> Result<FilePage, ProviderError> {
    if !valid_id(parent) || token.is_some_and(|s| !valid_token(s)) {
        return Err(ProviderError::InvalidResponse);
    }
    let query = format!("'{parent}' in parents and trashed = false");
    let fields = format!("nextPageToken,incompleteSearch,files({FILE_FIELDS})");
    let mut params = vec![
        ("q", query.as_str()),
        ("fields", fields.as_str()),
        ("pageSize", "200"),
        ("spaces", "drive"),
        ("supportsAllDrives", "true"),
        ("includeItemsFromAllDrives", "true"),
    ];
    if let Some(token) = token {
        params.push(("pageToken", token));
    }
    if matches!(corpus, Corpus::SharedDrive(_)) {
        params.push(("corpora", "drive"));
    } else {
        params.push(("corpora", "user"));
    }
    add_corpus(&mut params, corpus)?;
    let body = transport.get(&format!("{API}/files"), &params)?;
    let result: FilePage =
        serde_json::from_slice(&body).map_err(|_| ProviderError::InvalidResponse)?;
    if result.incomplete_search {
        return Err(ProviderError::Incomplete);
    }
    if result
        .next_page_token
        .as_deref()
        .is_some_and(|s| !valid_token(s))
        || result.files.iter().any(|f| !valid_id(&f.id))
    {
        return Err(ProviderError::InvalidResponse);
    }
    Ok(result)
}

fn file(transport: &impl Transport, id: &str) -> Result<File, ProviderError> {
    if !valid_id(id) {
        return Err(ProviderError::InvalidResponse);
    }
    let body = transport.get(
        &format!("{API}/files/{id}"),
        &[("fields", FILE_FIELDS), ("supportsAllDrives", "true")],
    )?;
    let result: File = serde_json::from_slice(&body).map_err(|_| ProviderError::InvalidResponse)?;
    if result.id != id {
        return Err(ProviderError::InvalidResponse);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    type RecordedRequest = (String, Vec<(String, String)>);

    struct Fake {
        response: &'static [u8],
        request: RefCell<Option<RecordedRequest>>,
    }

    impl Transport for Fake {
        fn get(&self, endpoint: &str, params: &[(&str, &str)]) -> Result<Vec<u8>, ProviderError> {
            *self.request.borrow_mut() = Some((
                endpoint.to_owned(),
                params
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ));
            Ok(self.response.to_vec())
        }
    }

    fn fake(response: &'static [u8]) -> Fake {
        Fake {
            response,
            request: RefCell::new(None),
        }
    }

    #[test]
    fn shared_drive_start_and_paginated_changes_use_same_corpus() {
        let corpus = Corpus::SharedDrive("shared-1".into());
        let provider = fake(br#"{"startPageToken":"cursor"}"#);
        assert_eq!(start(&provider, &corpus).unwrap(), "cursor");
        let req = provider.request.borrow();
        let (endpoint, params) = req.as_ref().unwrap();
        assert!(endpoint.ends_with("/changes/startPageToken"));
        assert!(params.contains(&("driveId".into(), "shared-1".into())));
        drop(req);
        let provider =
            fake(br#"{"nextPageToken":"next","changes":[{"changeType":"file","fileId":"file-1","removed":true}]}"#);
        let page = changes(&provider, &corpus, "cursor").unwrap();
        assert_eq!(page.next_page_token.as_deref(), Some("next"));
        assert!(page.changes[0].removed);
        let req = provider.request.borrow();
        let (endpoint, params) = req.as_ref().unwrap();
        assert!(endpoint.ends_with("/changes"));
        assert!(params.contains(&("pageToken".into(), "cursor".into())));
        assert!(params.contains(&("driveId".into(), "shared-1".into())));
        assert!(params.contains(&("includeRemoved".into(), "true".into())));
        let end = fake(br#"{"newStartPageToken":"final","changes":[]}"#);
        assert_eq!(
            changes(&end, &corpus, "next")
                .unwrap()
                .new_start_page_token
                .as_deref(),
            Some("final")
        );
    }

    #[test]
    fn pagination_and_metadata_are_validated_without_exposing_bodies() {
        let corpus = Corpus::User;
        for response in [
            br#"{"changes":[]}"#.as_slice(),
            br#"{"changes":[],"nextPageToken":"a","newStartPageToken":"b"}"#.as_slice(),
            br#"{"changes":[{"changeType":"file","fileId":"one","file":{"id":"two"}}],"newStartPageToken":"b"}"#
                .as_slice(),
        ] {
            assert!(matches!(
                changes(&fake(response), &corpus, "a"),
                Err(ProviderError::InvalidResponse)
            ));
        }
        assert!(matches!(
            children(
                &fake(br#"{"incompleteSearch":true,"files":[]}"#),
                &corpus,
                "root",
                None
            ),
            Err(ProviderError::Incomplete)
        ));
        let provider = fake(br#"{"files":[{"id":"f"}],"nextPageToken":"next"}"#);
        let page = children(&provider, &corpus, "root", None).unwrap();
        assert_eq!(page.files[0].id, "f");
        assert_eq!(page.next_page_token.as_deref(), Some("next"));
        assert!(matches!(
            file(&fake(br#"{"id":"other"}"#), "root"),
            Err(ProviderError::InvalidResponse)
        ));
    }
}
