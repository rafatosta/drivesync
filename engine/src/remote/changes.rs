//! Account-level change-feed transport with strict, bounded decoding.
use crate::{configuration::valid_id, AuthError, EngineError, InventoryEntry};
use serde::Deserialize;

#[derive(Clone, Debug)]
pub(crate) struct Change {
    pub id: String,
    pub removed: bool,
    /// Unavailable includes trash and lost access; it never means local deletion.
    pub item: Option<InventoryEntry>,
}
pub(crate) struct ChangePage {
    pub changes: Vec<Change>,
    pub next: Option<String>,
    pub terminal: Option<String>,
}
fn invalid() -> EngineError {
    EngineError::Authentication(AuthError::InvalidResponse)
}
pub(crate) fn valid_token(token: &str) -> bool {
    !token.is_empty() && token.len() <= 4096 && !token.chars().any(char::is_control)
}
pub(crate) fn change_start(token: &str) -> Result<String, EngineError> {
    let bytes = super::client::request(
        token,
        "https://www.googleapis.com/drive/v3/changes/startPageToken",
        &[("fields", "startPageToken")],
    )?;
    decode_start(&bytes)
}
fn decode_start(bytes: &[u8]) -> Result<String, EngineError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Start {
        start_page_token: String,
    }
    let start: Start = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if !valid_token(&start.start_page_token) {
        return Err(invalid());
    }
    Ok(start.start_page_token)
}
pub(crate) fn change_page(token: &str, page: &str) -> Result<ChangePage, EngineError> {
    if !valid_token(page) {
        return Err(invalid());
    }
    let bytes = super::client::change_request(token, "https://www.googleapis.com/drive/v3/changes", &[
        ("pageToken", page), ("pageSize", "200"), ("spaces", "drive"),
        ("includeRemoved", "true"), ("includeItemsFromAllDrives", "false"),
        ("fields", "nextPageToken,newStartPageToken,changes(fileId,removed,file(id,name,mimeType,parents,trashed,version,size,md5Checksum,driveId,capabilities(canDownload)))")])?;
    decode_page(&bytes)
}
fn decode_page(bytes: &[u8]) -> Result<ChangePage, EngineError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Page {
        #[serde(default)]
        changes: Vec<Raw>,
        next_page_token: Option<String>,
        new_start_page_token: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        file_id: String,
        #[serde(default)]
        removed: bool,
        file: Option<serde_json::Value>,
    }
    let page: Page = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if page.next_page_token.is_some() == page.new_start_page_token.is_some()
        || page
            .next_page_token
            .iter()
            .chain(&page.new_start_page_token)
            .any(|t| !valid_token(t))
        || page.changes.len() > 200
    {
        return Err(invalid());
    }
    let mut changes = Vec::new();
    for raw in page.changes {
        if !valid_id(&raw.file_id) {
            return Err(invalid());
        }
        let mut removed = raw.removed;
        let item = if raw.removed {
            None
        } else {
            let file = raw.file.ok_or_else(invalid)?;
            if file.get("id").and_then(|v| v.as_str()) != Some(&raw.file_id) {
                return Err(invalid());
            }
            let trashed = file
                .get("trashed")
                .and_then(|v| v.as_bool())
                .ok_or_else(invalid)?;
            if file.get("parents").is_some_and(|v| !v.is_array()) {
                return Err(invalid());
            }
            let parents = file.get("parents").and_then(|v| v.as_array());
            removed = trashed || file.get("driveId").is_some_and(|v| !v.is_null());
            if trashed
                || file.get("driveId").is_some_and(|v| !v.is_null())
                || parents.is_none_or(|p| p.is_empty())
            {
                None
            } else {
                let parents = parents.unwrap();
                if parents.len() != 1 {
                    return Err(invalid());
                }
                let parent = parents[0]
                    .as_str()
                    .filter(|id| valid_id(id))
                    .ok_or_else(invalid)?;
                Some(super::inventory::decode_item(
                    &serde_json::to_vec(&file).map_err(|_| invalid())?,
                    parent,
                )?)
            }
        };
        changes.push(Change {
            id: raw.file_id,
            removed,
            item,
        });
    }
    Ok(ChangePage {
        changes,
        next: page.next_page_token,
        terminal: page.new_start_page_token,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_envelopes_and_does_not_infer_removal_from_missing_file() {
        for data in [
            r#"{}"#,
            r#"{"changes":[]}"#,
            r#"{"changes":[],"nextPageToken":"n","newStartPageToken":"t"}"#,
            r#"{"changes":[{"fileId":"f","removed":false}],"newStartPageToken":"t"}"#,
            r#"{"changes":[],"nextPageToken":""}"#,
        ] {
            assert!(decode_page(data.as_bytes()).is_err());
        }
        let page =
            decode_page(br#"{"changes":[{"fileId":"f","removed":true}],"newStartPageToken":"t"}"#)
                .unwrap();
        assert!(page.changes[0].item.is_none());
        assert!(decode_page(br#"{"changes":[],"nextPageToken":"n"}"#)
            .unwrap()
            .changes
            .is_empty());
        assert_eq!(decode_start(br#"{"startPageToken":"s"}"#).unwrap(), "s");
        assert!(decode_start(br#"{"startPageToken":""}"#).is_err());
        assert!(decode_page(br#"{"newStartPageToken":"t"}"#)
            .unwrap()
            .changes
            .is_empty());
        let page=decode_page(br#"{"changes":[{"fileId":"root","file":{"id":"root","trashed":false}}],"newStartPageToken":"t"}"#).unwrap();
        assert!(!page.changes[0].removed);
        let page=decode_page(br#"{"changes":[{"fileId":"f","removed":false,"file":{"id":"f","name":"f","mimeType":"text/plain","parents":["root"],"trashed":false,"version":"2","size":"3","md5Checksum":"900150983cd24fb0d6963f7d28e17f72","capabilities":{"canDownload":true}}}],"newStartPageToken":"t"}"#).unwrap();
        assert_eq!(page.changes[0].item.as_ref().unwrap().version, Some(2));
        for file in [
            r#"{"id":"other","trashed":false}"#,
            r#"{"id":"f","trashed":false,"parents":true}"#,
        ] {
            let body = format!(
                r#"{{"changes":[{{"fileId":"f","removed":false,"file":{file}}}],"newStartPageToken":"t"}}"#
            );
            assert!(decode_page(body.as_bytes()).is_err());
        }
    }
}
