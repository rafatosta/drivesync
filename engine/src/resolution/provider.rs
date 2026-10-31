//! Account-scoped v2 conditional updates; discovery and reserved-ID copies use v3.
use super::{execution::Remote, Blob, Plan, RemoteVersion};
use crate::{
    auth::OAuthConfig, download::filesystem::Root, AuthError, Cancellation, DriveSyncEngine,
    EngineError, FolderConfiguration, InventoryEntry, InventoryKind, RemoteInventory,
};
use reqwest::{
    blocking::{Client, Response},
    Method,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Read, Seek, SeekFrom},
    time::Duration,
};

pub(crate) struct Provider<'a> {
    pub engine: &'a DriveSyncEngine,
    pub folder: &'a FolderConfiguration,
    pub config: &'a OAuthConfig,
    pub root: &'a Root,
    pub cancel: &'a Cancellation,
    parents: HashMap<String, String>,
}
impl<'a> Provider<'a> {
    pub fn new(
        engine: &'a DriveSyncEngine,
        folder: &'a FolderConfiguration,
        config: &'a OAuthConfig,
        root: &'a Root,
        cancel: &'a Cancellation,
        inventory: &RemoteInventory,
    ) -> Result<Self, EngineError> {
        if inventory.account_id != folder.account_id || inventory.root_id != folder.remote.id {
            return Err(EngineError::InvalidPlan);
        }
        Ok(Self {
            engine,
            folder,
            config,
            root,
            cancel,
            parents: inventory
                .entries
                .iter()
                .filter(|v| v.kind == InventoryKind::Folder)
                .map(|v| (v.id.clone(), v.parent_id.clone()))
                .collect(),
        })
    }
    fn access<T>(&self, f: impl FnOnce(&str) -> Result<T, EngineError>) -> Result<T, EngineError> {
        crate::local::check(self.cancel)?;
        self.engine
            .auth_upload_access(self.config, &self.folder.account_id, f)
    }
    fn parent(&self, parent: &str, write: bool) -> Result<(), EngineError> {
        let mut current = parent.to_owned();
        let mut seen = HashSet::new();
        while current != self.folder.remote.id {
            if !seen.insert(current.clone()) || seen.len() > 10000 {
                return Err(EngineError::RemoteChanged);
            }
            let expected = self
                .parents
                .get(&current)
                .ok_or(EngineError::RemoteChanged)?;
            let value = self.access(|token| {
                get_json(
                    token,
                    &format!("https://www.googleapis.com/drive/v3/files/{current}"),
                    "id,parents,mimeType,trashed,driveId",
                )
            })?;
            if value["id"].as_str() != Some(&current)
                || value["parents"]
                    .as_array()
                    .is_none_or(|v| v.len() != 1 || v[0].as_str() != Some(expected))
                || value["mimeType"].as_str() != Some("application/vnd.google-apps.folder")
                || value["trashed"].as_bool() != Some(false)
                || !value["driveId"].is_null()
            {
                return Err(EngineError::RemoteChanged);
            }
            current = expected.clone();
        }
        self.access(|token| crate::remote::inventory_root(token, &self.folder.remote.id))
            .map_err(crate::coordinator::remote)?;
        if write {
            self.access(|token| crate::remote::writable_root(token, parent))?;
        }
        Ok(())
    }
    fn source(&self, plan: &Plan) -> Result<(), EngineError> {
        self.engine.verify_root(self.folder, self.config)?;
        self.root.validate()?;
        if crate::download::filesystem::fingerprint_root(self.root, &plan.origin, self.cancel)?
            .as_ref()
            != Some(&plan.local_fingerprint)
        {
            return Err(EngineError::DownloadConflict);
        }
        Ok(())
    }
    fn names(&self, parent: &str, name: &str, id: &str) -> Result<(), EngineError> {
        let mut cursor = None::<String>;
        let mut seen = HashSet::new();
        for _ in 0..10000 {
            let data = self
                .access(|token| crate::remote::inventory_page(token, parent, cursor.as_deref()))?;
            let page: Value = serde_json::from_slice(&data).map_err(|_| invalid())?;
            if page["incompleteSearch"].as_bool() == Some(true) {
                return Err(invalid());
            }
            for value in page["files"].as_array().ok_or_else(invalid)? {
                let entry = crate::remote::decode_upload_item(
                    &serde_json::to_vec(value).map_err(|_| invalid())?,
                    parent,
                )?;
                if entry.id != id
                    && crate::planner::path_key(&entry.name) == crate::planner::path_key(name)
                {
                    return Err(EngineError::DownloadConflict);
                }
            }
            match page.get("nextPageToken") {
                None | Some(Value::Null) => return Ok(()),
                Some(Value::String(next))
                    if !next.is_empty() && next.len() <= 4096 && seen.insert(next.clone()) =>
                {
                    cursor = Some(next.clone())
                }
                _ => return Err(invalid()),
            }
        }
        Err(EngineError::InventoryLimit)
    }
    fn raw(&self, id: &str) -> Result<(RemoteVersion, bool), EngineError> {
        if !crate::configuration::valid_id(id) {
            return Err(EngineError::InvalidPlan);
        }
        let value=self.access(|token|get_json(token,&format!("https://www.googleapis.com/drive/v2/files/{id}"),"id,title,mimeType,version,fileSize,md5Checksum,etag,parents(id),labels(trashed),driveId,capabilities(canEdit,canDownload)"))?;
        decode(&value, id)
    }
    /// Fresh sessions are provisional; after interruption the durable intention
    /// reconciles the same ID first, then may restart from immutable snapshots.
    /// Every final publication still carries the original conditional tag.
    fn transfer(
        &mut self,
        plan: &Plan,
        mut file: File,
        blob: &Blob,
        copy: bool,
    ) -> Result<(), EngineError> {
        self.configured()?;
        self.source(plan)?;
        self.parent(&plan.remote.item.parent_id, true)?;
        let (base, metadata) = if copy {
            let id = plan.copy_id.as_deref().ok_or(EngineError::InvalidPlan)?;
            let name = plan.copy_name.as_deref().ok_or(EngineError::InvalidPlan)?;
            self.names(&plan.remote.item.parent_id, name, id)?;
            (
                "https://www.googleapis.com/upload/drive/v3/files".to_string(),
                json!({"id":id,"name":name,"parents":[plan.remote.item.parent_id],"mimeType":plan.remote.item.mime_type}),
            )
        } else {
            let (current, editable) = self.raw(&plan.remote.item.id)?;
            if current != plan.remote {
                return Err(EngineError::RemoteChanged);
            }
            if !editable {
                return Err(EngineError::RemoteForbidden);
            }
            self.names(
                &current.item.parent_id,
                &plan.desired_name,
                &current.item.id,
            )?;
            (
                format!(
                    "https://www.googleapis.com/upload/drive/v2/files/{}",
                    current.item.id
                ),
                json!({"title":plan.desired_name}),
            )
        };
        // Verify the immutable source before authorizing a provisional session.
        if !crate::download::stream::verify(&mut file, blob.size, &blob.sha256, self.cancel)? {
            return Err(EngineError::DownloadIntegrity);
        }
        file.seek(SeekFrom::Start(0))
            .map_err(crate::download::filesystem::io)?;
        let session = self.access(|token| {
            let method = if copy { Method::POST } else { Method::PUT };
            let mut request = client()?
                .request(method, &base)
                .bearer_auth(token)
                .query(&[("uploadType", "resumable")])
                .header("X-Upload-Content-Type", "application/octet-stream")
                .header("X-Upload-Content-Length", blob.size)
                .json(&metadata);
            if !copy {
                request = request.header(reqwest::header::IF_MATCH, &plan.remote.etag);
            }
            let response = request.send().map_err(network)?;
            if response.status().as_u16() != 200 {
                body(response)?;
                return Err(invalid());
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(invalid)?
                .to_owned();
            if !valid_session(
                &location,
                if copy {
                    None
                } else {
                    Some(&plan.remote.item.id)
                },
            ) {
                return Err(invalid());
            }
            Ok(location)
        })?;
        let mut offset = 0u64;
        let mut hash = Sha256::new();
        loop {
            self.configured()?;
            self.source(plan)?;
            let count = (blob.size - offset).min(8 * 1024 * 1024) as usize;
            let mut bytes = vec![0; count];
            file.read_exact(&mut bytes)
                .map_err(crate::download::filesystem::io)?;
            hash.update(&bytes);
            let end = offset + count as u64;
            let last = end == blob.size;
            if last {
                let mut extra = [0];
                if file
                    .read(&mut extra)
                    .map_err(crate::download::filesystem::io)?
                    != 0
                    || format!("{:x}", hash.clone().finalize()) != blob.sha256
                {
                    return Err(EngineError::DownloadIntegrity);
                }
                self.source(plan)?;
                self.parent(&plan.remote.item.parent_id, true)?;
            }
            self.access(|token| {
                let mut request = client()?
                    .put(&session)
                    .bearer_auth(token)
                    .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                    .header(reqwest::header::CONTENT_LENGTH, count)
                    .header(
                        reqwest::header::CONTENT_RANGE,
                        if blob.size == 0 {
                            "bytes */0".into()
                        } else {
                            format!("bytes {offset}-{}/{total}", end - 1, total = blob.size)
                        },
                    )
                    .body(bytes);
                if !copy {
                    request = request.header(reqwest::header::IF_MATCH, &plan.remote.etag);
                }
                let response = request.send().map_err(network)?;
                if !last && response.status().as_u16() == 308 {
                    if response
                        .headers()
                        .get(reqwest::header::RANGE)
                        .and_then(|v| v.to_str().ok())
                        != Some(format!("bytes=0-{}", end - 1).as_str())
                    {
                        return Err(invalid());
                    }
                    return Ok(());
                }
                if !last {
                    body(response)?;
                    return Err(invalid());
                }
                body(response).map(|_| ())
            })?;
            offset = end;
            if last {
                break;
            }
        }
        Ok(())
    }
}
impl Remote for Provider<'_> {
    fn configured(&mut self) -> Result<(), EngineError> {
        self.engine.verify_root(self.folder, self.config)?;
        self.root.validate()?;
        self.access(|token| crate::remote::inventory_root(token, &self.folder.remote.id))
            .map_err(crate::coordinator::remote)
            .map(|_| ())
    }
    fn observe(&mut self, id: &str) -> Result<RemoteVersion, EngineError> {
        for _ in 0..3 {
            let (version, _) = self.raw(id)?;
            self.parent(&version.item.parent_id, false)?;
            let current =
                self.access(|token| crate::remote::file_metadata(token, &version.item))?;
            if current == version.item {
                return Ok(version);
            }
        }
        Err(EngineError::RemoteChanged)
    }
    fn media(&mut self, version: &RemoteVersion) -> Result<Box<dyn Read>, EngineError> {
        if !version.valid() {
            return Err(EngineError::InvalidPlan);
        }
        self.parent(&version.item.parent_id, false)?;
        self.access(|token| {
            let response = client()?
                .get(format!(
                    "https://www.googleapis.com/drive/v2/files/{}",
                    version.item.id
                ))
                .query(&[("alt", "media")])
                .bearer_auth(token)
                .header(reqwest::header::IF_MATCH, &version.etag)
                .header(reqwest::header::ACCEPT_ENCODING, "identity")
                .send()
                .map_err(network)?;
            if response.status().as_u16() != 200 {
                body(response)?;
                return Err(invalid());
            }
            if response
                .headers()
                .get(reqwest::header::CONTENT_ENCODING)
                .is_some_and(|v| v != "identity")
            {
                return Err(EngineError::DownloadIntegrity);
            }
            Ok(Box::new(response) as Box<dyn Read>)
        })
    }
    fn generate(&mut self) -> Result<String, EngineError> {
        self.access(crate::upload::generate_remote_id)
    }
    fn copy(&mut self, plan: &Plan, bytes: File, blob: &Blob) -> Result<(), EngineError> {
        self.transfer(plan, bytes, blob, true)
    }
    fn update(&mut self, plan: &Plan, bytes: Option<File>, blob: &Blob) -> Result<(), EngineError> {
        if let Some(file) = bytes {
            return self.transfer(plan, file, blob, false);
        }
        self.configured()?;
        self.source(plan)?;
        self.parent(&plan.remote.item.parent_id, true)?;
        let (current, editable) = self.raw(&plan.remote.item.id)?;
        if current != plan.remote {
            return Err(EngineError::RemoteChanged);
        }
        if !editable {
            return Err(EngineError::RemoteForbidden);
        }
        self.names(
            &current.item.parent_id,
            &plan.desired_name,
            &current.item.id,
        )?;
        self.access(|token| {
            body(
                client()?
                    .patch(format!(
                        "https://www.googleapis.com/drive/v2/files/{}",
                        current.item.id
                    ))
                    .bearer_auth(token)
                    .header(reqwest::header::IF_MATCH, &current.etag)
                    .json(&json!({"title":plan.desired_name}))
                    .send()
                    .map_err(network)?,
            )
            .map(|_| ())
        })
    }
}
fn invalid() -> EngineError {
    EngineError::Authentication(AuthError::InvalidResponse)
}
fn network(_: impl std::fmt::Debug) -> EngineError {
    EngineError::Authentication(AuthError::Network)
}
fn client() -> Result<Client, EngineError> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .user_agent("DriveSync/0.1")
        .build()
        .map_err(network)
}
fn body(response: Response) -> Result<Vec<u8>, EngineError> {
    let status = response.status().as_u16();
    let mut bytes = Vec::new();
    response
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(network)?;
    if bytes.len() > 1024 * 1024 {
        return Err(invalid());
    }
    if matches!(status, 409 | 412) {
        return Err(EngineError::RemoteChanged);
    }
    crate::remote::upload_response_status(if status == 201 { 200 } else { status }, &bytes)?;
    Ok(bytes)
}
fn get_json(token: &str, url: &str, fields: &str) -> Result<Value, EngineError> {
    serde_json::from_slice(&body(
        client()?
            .get(url)
            .bearer_auth(token)
            .query(&[("fields", fields)])
            .send()
            .map_err(network)?,
    )?)
    .map_err(|_| invalid())
}
fn decode(value: &Value, id: &str) -> Result<(RemoteVersion, bool), EngineError> {
    let string = |key: &str| value[key].as_str().map(str::to_owned).ok_or_else(invalid);
    let parents = value["parents"]
        .as_array()
        .filter(|v| v.len() == 1)
        .ok_or_else(invalid)?;
    if value["id"].as_str() != Some(id)
        || value["labels"]["trashed"].as_bool() != Some(false)
        || !value["driveId"].is_null()
        || value["capabilities"]["canDownload"].as_bool() != Some(true)
    {
        return Err(EngineError::RemoteFolderUnavailable);
    }
    let mime = string("mimeType")?;
    if mime.starts_with("application/vnd.google-apps.") {
        return Err(EngineError::InvalidPlan);
    }
    let version = RemoteVersion {
        etag: string("etag")?,
        item: InventoryEntry {
            id: id.into(),
            parent_id: parents[0]["id"].as_str().ok_or_else(invalid)?.into(),
            name: string("title")?,
            mime_type: mime,
            version: Some(string("version")?.parse().map_err(|_| invalid())?),
            size: Some(string("fileSize")?.parse().map_err(|_| invalid())?),
            md5_checksum: Some(string("md5Checksum")?.to_ascii_lowercase()),
            web_view_link: None,
            kind: InventoryKind::Binary,
        },
    };
    if !version.valid() {
        return Err(invalid());
    }
    Ok((
        version,
        value["capabilities"]["canEdit"].as_bool() == Some(true),
    ))
}
/// No session URI may select an alternate origin, identity, port or fragment.
fn valid_session(session: &str, id: Option<&str>) -> bool {
    if session.len() > 8192 {
        return false;
    }
    url::Url::parse(session).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("www.googleapis.com")
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
            && url.path()
                == id
                    .map(|id| format!("/upload/drive/v2/files/{id}"))
                    .unwrap_or_else(|| "/upload/drive/v3/files".into())
            && url
                .query_pairs()
                .any(|(k, v)| k == "upload_id" && !v.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_origin_identity_and_capability_are_strict() {
        assert!(valid_session(
            "https://www.googleapis.com/upload/drive/v2/files/file?upload_id=opaque",
            Some("file")
        ));
        assert!(valid_session(
            "https://www.googleapis.com/upload/drive/v3/files?upload_id=opaque",
            None
        ));
        for uri in [
            "http://www.googleapis.com/upload/drive/v2/files/file?upload_id=x",
            "https://www.googleapis.com.evil.example/upload/drive/v2/files/file?upload_id=x",
            "https://www.googleapis.com:444/upload/drive/v2/files/file?upload_id=x",
            "https://user@www.googleapis.com/upload/drive/v2/files/file?upload_id=x",
            "https://www.googleapis.com/upload/drive/v2/files/other?upload_id=x",
            "https://www.googleapis.com/upload/drive/v2/files/file?upload_id=",
            "https://www.googleapis.com/upload/drive/v2/files/file?upload_id=x#fragment",
        ] {
            assert!(!valid_session(uri, Some("file")));
        }
    }
    #[test]
    fn metadata_does_not_infer_integrity_permissions_or_preconditions() {
        let value = json!({"id":"file","title":"report.txt","mimeType":"text/plain","version":"3","fileSize":"3","md5Checksum":"900150983cd24fb0d6963f7d28e17f72","etag":"\"opaque\"","parents":[{"id":"root"}],"labels":{"trashed":false},"capabilities":{"canDownload":true,"canEdit":false}});
        let (version, editable) = decode(&value, "file").unwrap();
        assert!(version.valid());
        assert!(!editable);
        for key in [
            "etag",
            "version",
            "fileSize",
            "md5Checksum",
            "parents",
            "labels",
            "capabilities",
        ] {
            let mut missing = value.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(decode(&missing, "file").is_err(), "{key}");
        }
        for tag in ["*", "W/\"weak\"", "\"line\r\nbreak\"", ""] {
            let mut bad = value.clone();
            bad["etag"] = tag.into();
            assert!(decode(&bad, "file").is_err());
        }
        let mut duplicate = value.clone();
        duplicate["parents"] = json!([{"id":"root"},{"id":"other"}]);
        assert!(decode(&duplicate, "file").is_err());
    }
}
