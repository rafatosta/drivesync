//! Bounded read-only traversal of a configured Drive tree; no file transfers.
use crate::{configuration::valid_id, AccountId, AuthError, Cancellation, EngineError};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

/// Why an observed item is excluded from the initial binary-download scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkipReason {
    UnsafeName,
    SharedDrive,
    GoogleNativeOrShortcut,
    DownloadNotAllowed,
    MissingIntegrityMetadata,
}
/// Classification is observational, never proof that local installation is safe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InventoryKind {
    Folder,
    Binary,
    Skipped(SkipReason),
}

/// One remote identity. Duplicate names remain separate entries by ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryEntry {
    pub id: String,
    pub parent_id: String,
    pub name: String,
    pub mime_type: String,
    pub version: Option<u64>,
    pub size: Option<u64>,
    pub md5_checksum: Option<String>,
    pub kind: InventoryKind,
}
/// Successful observations scoped to one account/root, not a consistent snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteInventory {
    pub account_id: AccountId,
    pub root_id: String,
    pub entries: Vec<InventoryEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    id: String,
    name: String,
    mime_type: String,
    parents: Vec<String>,
    trashed: bool,
    version: Option<String>,
    size: Option<String>,
    md5_checksum: Option<String>,
    drive_id: Option<String>,
    capabilities: Option<Capabilities>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Capabilities {
    can_download: Option<bool>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    files: Vec<Item>,
    next_page_token: Option<String>,
    #[serde(default)]
    incomplete_search: bool,
}

fn invalid() -> EngineError {
    EngineError::Authentication(AuthError::InvalidResponse)
}
fn cancelled(cancel: &Cancellation) -> Result<(), EngineError> {
    if cancel.is_cancelled() {
        Err(EngineError::Authentication(AuthError::Cancelled))
    } else {
        Ok(())
    }
}
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 255
        && !name.contains('/')
        && !name.contains('\\')
        && !name.chars().any(char::is_control)
}
impl Item {
    fn decode(self, parent: &str) -> Result<InventoryEntry, EngineError> {
        if !valid_id(&self.id)
            || self.name.is_empty()
            || self.name.len() > 4096
            || self.mime_type.is_empty()
            || self.mime_type.len() > 255
            || self.mime_type.chars().any(char::is_control)
            || self.trashed
            || self.parents != [parent]
        {
            return Err(invalid());
        }
        let version = self
            .version
            .map(|v| v.parse::<u64>().map_err(|_| invalid()))
            .transpose()?;
        if version == Some(0) {
            return Err(invalid());
        }
        let size = self
            .size
            .map(|v| v.parse::<u64>().map_err(|_| invalid()))
            .transpose()?;
        if self
            .md5_checksum
            .as_ref()
            .is_some_and(|h| h.len() != 32 || !h.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(invalid());
        }
        let kind = if !safe_name(&self.name) {
            InventoryKind::Skipped(SkipReason::UnsafeName)
        } else if self.drive_id.is_some() {
            InventoryKind::Skipped(SkipReason::SharedDrive)
        } else if self.mime_type == super::models::MIME {
            InventoryKind::Folder
        } else if self.mime_type.starts_with("application/vnd.google-apps.") {
            InventoryKind::Skipped(SkipReason::GoogleNativeOrShortcut)
        } else if self.capabilities.and_then(|c| c.can_download) != Some(true) {
            InventoryKind::Skipped(SkipReason::DownloadNotAllowed)
        } else if version.is_none() || size.is_none() || self.md5_checksum.is_none() {
            InventoryKind::Skipped(SkipReason::MissingIntegrityMetadata)
        } else {
            InventoryKind::Binary
        };
        Ok(InventoryEntry {
            id: self.id,
            parent_id: parent.into(),
            name: self.name,
            mime_type: self.mime_type,
            version,
            size,
            md5_checksum: self.md5_checksum,
            kind,
        })
    }
}

/// Walks provider pages through an injectable read-only transport.
/// Failures/cancellation discard the partial in-memory result. Limits bound work.
pub(crate) fn scan(
    account: &AccountId,
    root: &str,
    cancel: &Cancellation,
    mut fetch: impl FnMut(&str, Option<&str>) -> Result<Vec<u8>, EngineError>,
) -> Result<RemoteInventory, EngineError> {
    scan_bounded(
        account,
        root,
        cancel,
        &mut fetch,
        100_000,
        10_000,
        &mut |_, _, _| {},
    )
}
/// Reports completed pages and the next folder before waiting on its request.
/// Partial counts never escape as a successful inventory on failure.
pub(crate) fn scan_with_progress(
    account: &AccountId,
    root: &str,
    cancel: &Cancellation,
    mut fetch: impl FnMut(&str, Option<&str>) -> Result<Vec<u8>, EngineError>,
    mut progress: impl FnMut(usize, usize, &std::path::Path),
) -> Result<RemoteInventory, EngineError> {
    scan_bounded(
        account,
        root,
        cancel,
        &mut fetch,
        100_000,
        10_000,
        &mut progress,
    )
}
fn scan_bounded(
    account: &AccountId,
    root: &str,
    cancel: &Cancellation,
    fetch: &mut impl FnMut(&str, Option<&str>) -> Result<Vec<u8>, EngineError>,
    max_items: usize,
    max_pages: usize,
    progress: &mut impl FnMut(usize, usize, &std::path::Path),
) -> Result<RemoteInventory, EngineError> {
    if !valid_id(root) {
        return Err(EngineError::RemoteFolderUnavailable);
    }
    let mut seen = HashSet::from([root.to_string()]);
    let mut queue = VecDeque::from([(root.to_string(), std::path::PathBuf::new())]);
    let mut entries = Vec::new();
    let mut pages = 0;
    let mut files = 0;
    let mut folders = 0;
    while let Some((parent, path)) = queue.pop_front() {
        let mut token: Option<String> = None;
        let mut tokens = HashSet::new();
        loop {
            cancelled(cancel)?;
            pages += 1;
            if pages > max_pages {
                return Err(EngineError::InventoryLimit);
            }
            progress(files, folders, &path);
            cancelled(cancel)?;
            let body = fetch(&parent, token.as_deref())?;
            cancelled(cancel)?;
            let page: Page = serde_json::from_slice(&body).map_err(|_| invalid())?;
            if page.incomplete_search {
                return Err(invalid());
            }
            for raw in page.files {
                cancelled(cancel)?;
                let entry = raw.decode(&parent)?;
                if !seen.insert(entry.id.clone()) {
                    return Err(invalid());
                }
                if entries.len() >= max_items {
                    return Err(EngineError::InventoryLimit);
                }
                if entry.mime_type == super::models::MIME {
                    folders += 1;
                } else {
                    files += 1;
                }
                if entry.kind == InventoryKind::Folder {
                    queue.push_back((entry.id.clone(), path.join(&entry.name)));
                }
                entries.push(entry);
            }
            progress(files, folders, &path);
            token = page.next_page_token;
            match &token {
                None => break,
                Some(t)
                    if t.is_empty()
                        || t.len() > 4096
                        || t.chars().any(char::is_control)
                        || !tokens.insert(t.clone()) =>
                {
                    return Err(invalid())
                }
                _ => {}
            }
        }
    }
    cancelled(cancel)?;
    Ok(RemoteInventory {
        account_id: account.clone(),
        root_id: root.into(),
        entries,
    })
}

/// Requests all child metadata, not the folder-only picker query.
pub(crate) fn page(
    token: &str,
    parent: &str,
    continuation: Option<&str>,
) -> Result<Vec<u8>, EngineError> {
    if !valid_id(parent)
        || continuation
            .is_some_and(|t| t.is_empty() || t.len() > 4096 || t.chars().any(char::is_control))
    {
        return Err(invalid());
    }
    let query = format!("'{parent}' in parents and trashed = false");
    let mut params = vec![("q",query.as_str()),("fields","nextPageToken,incompleteSearch,files(id,name,mimeType,parents,trashed,version,size,md5Checksum,driveId,capabilities(canDownload))"),("pageSize","200"),("spaces","drive"),("corpora","user")];
    if let Some(next) = continuation {
        params.push(("pageToken", next));
    }
    super::client::request(token, "https://www.googleapis.com/drive/v3/files", &params)
}

/// Resolves the root alias and rejects shared-drive roots in this initial scope.
pub(crate) fn root(token: &str, configured: &str) -> Result<String, EngineError> {
    if !valid_id(configured) {
        return Err(EngineError::RemoteFolderUnavailable);
    }
    let body = super::client::request(
        token,
        &format!("https://www.googleapis.com/drive/v3/files/{configured}"),
        &[("fields", "id,mimeType,trashed,driveId")],
    )?;
    decode_root(&body, configured)
}
fn decode_root(body: &[u8], configured: &str) -> Result<String, EngineError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Root {
        id: String,
        mime_type: String,
        trashed: bool,
        drive_id: Option<String>,
    }
    let root: Root = serde_json::from_slice(body).map_err(|_| invalid())?;
    if !valid_id(&root.id)
        || (configured != "root" && root.id != configured)
        || root.trashed
        || root.mime_type != super::models::MIME
        || root.drive_id.is_some()
    {
        return Err(EngineError::RemoteFolderUnavailable);
    }
    Ok(root.id)
}

#[cfg(test)]
mod tests;

/// Decodes single-item metadata for download revalidation without relaxing inventory rules.
pub(super) fn decode_item(body: &[u8], parent: &str) -> Result<InventoryEntry, EngineError> {
    serde_json::from_slice::<Item>(body)
        .map_err(|_| invalid())?
        .decode(parent)
}
