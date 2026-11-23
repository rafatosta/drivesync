//! Bounded HTTP validators for conditional writes, without retry or policy.
use super::{
    client::{decode, invalid_argument, query_for},
    *,
};
use reqwest::{Method, Url};
/// Metadata with the opaque HTTP validator of the same response.
/// A missing/weak validator never grants conditional mutation authority.
#[derive(Debug)]
pub struct GoogleFileSnapshot {
    pub file: GoogleFile,
    pub etag: Option<String>,
}
pub(super) fn strong_etag(value: &str) -> GoogleResult<()> {
    if value.len() < 2
        || value.len() > 4096
        || !value.starts_with('"')
        || !value.ends_with('"')
        || value[1..value.len() - 1]
            .bytes()
            .any(|b| b < 0x21 || b == b'"' || b == 0x7f)
    {
        return Err(invalid_argument());
    }
    Ok(())
}
impl GoogleDriveClient {
    fn file_url(&self, id: &FileId, query: Vec<(String, String)>) -> GoogleResult<Url> {
        if matches!(id.0.as_str(), "" | "." | "..") {
            return Err(invalid_argument());
        }
        let mut url = self.drive_base.clone();
        url.path_segments_mut()
            .map_err(|_| invalid_argument())?
            .pop_if_empty()
            .push("files")
            .push(&id.0);
        url.query_pairs_mut().extend_pairs(query);
        Ok(url)
    }
    /// Reads metadata and its response ETag together; no validator is fabricated.
    pub async fn files_get_snapshot(
        &self,
        session: &GoogleSession,
        id: &FileId,
        options: FileGetOptions,
    ) -> GoogleResult<GoogleFileSnapshot> {
        let mut options = options;
        options.fields = Some(FieldMask(match options.fields {
            Some(mask) => format!("{},version", mask.0),
            None => "*".into(),
        }));
        let url = self.file_url(id, super::file_get_query(&options))?;
        let response = self.send(session, Method::GET, url, None, &[]).await?;
        let mut etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .filter(|v| strong_etag(v).is_ok())
            .map(str::to_owned);
        let bytes = self.response_body(response, 200).await?;
        let file = super::parse_file(&bytes)?;
        // Drive v3 metadata may omit HTTP validators. Read the official v2
        // validator for the same identity and version; never derive one from a
        // version counter or accept a validator for a different snapshot.
        if self.drive_base.host_str() == Some("www.googleapis.com") {
            let mut validator_url = self.drive_base.clone();
            validator_url.set_path("/drive/v2/files/");
            validator_url
                .path_segments_mut()
                .map_err(|_| invalid_argument())?
                .pop_if_empty()
                .push(&id.0);
            validator_url.set_query(Some("fields=id,etag,version&supportsAllDrives=true"));
            let response = self
                .send(session, Method::GET, validator_url, None, &[])
                .await?;
            let body = self.response_body(response, 200).await?;
            let validator: serde_json::Value = decode(&body)?;
            if validator.get("id").and_then(|v| v.as_str()) != Some(id.0.as_str())
                || validator.get("version").and_then(|v| v.as_str()) != file.version.as_deref()
                || file.version.is_none()
            {
                return Err(GoogleApiFailure::new(
                    GoogleApiError::PreconditionFailed,
                    None,
                ));
            }
            etag = validator
                .get("etag")
                .and_then(|v| v.as_str())
                .filter(|v| strong_etag(v).is_ok())
                .map(str::to_owned);
        }
        Ok(GoogleFileSnapshot { file, etag })
    }
    /// PATCH with an explicit strong If-Match validator. HTTP 412 remains a
    /// precondition failure; this call never retries or falls back unconditionally.
    pub async fn files_update_conditional(
        &self,
        session: &GoogleSession,
        id: &FileId,
        request: GoogleFileWrite,
        options: FileUpdateOptions,
        etag: &str,
    ) -> GoogleResult<GoogleFile> {
        strong_etag(etag)?;
        request.validate_update()?;
        let query = query_for(
            &options,
            &[
                "addParents",
                "removeParents",
                "supportsAllDrives",
                "keepRevisionForever",
                "useContentAsIndexableText",
                "fields",
            ],
        )?;
        let mut url = self.file_url(
            id,
            query.into_iter().filter(|(k, _)| k != "fields").collect(),
        )?;
        let path = url.path().replace("/drive/v3/", "/drive/v2/");
        url.set_path(&path);
        url.query_pairs_mut().append_pair("fields", "*");
        let bytes = conditional_body(&request)?;
        let response = self
            .send(
                session,
                Method::PATCH,
                url,
                Some(bytes),
                &[("If-Match", etag.into())],
            )
            .await?;
        conditional_file(&self.response_body(response, 200).await?)
    }
}

/// Translate only the conditional mutation subset used by Operator. Unsupported
/// v3 fields must never be silently dropped when talking to the v2 endpoint.
pub(super) fn conditional_body(request: &GoogleFileWrite) -> GoogleResult<Vec<u8>> {
    let value = serde_json::to_value(request).map_err(|_| invalid_argument())?;
    let mut out = serde_json::Map::new();
    for (key, value) in value.as_object().ok_or_else(invalid_argument)? {
        match key.as_str() {
            "name" => {
                out.insert("title".into(), value.clone());
            }
            "trashed" => {
                out.insert("labels".into(), serde_json::json!({"trashed": value}));
            }
            "appProperties" | "properties" if value.is_object() => {
                let visibility = if key == "appProperties" {
                    "PRIVATE"
                } else {
                    "PUBLIC"
                };
                let properties: Vec<_> = value.as_object().unwrap().iter().map(|(key,value)| serde_json::json!({"key":key,"value":value,"visibility":visibility})).collect();
                out.entry("properties")
                    .or_insert_with(|| serde_json::json!([]))
                    .as_array_mut()
                    .unwrap()
                    .extend(properties);
            }
            "mimeType" | "description" => {
                out.insert(key.clone(), value.clone());
            }
            _ => {
                return Err(GoogleApiFailure::new(
                    GoogleApiError::UnsupportedOperation,
                    None,
                ))
            }
        }
    }
    serde_json::to_vec(&out).map_err(|_| invalid_argument())
}
pub(super) fn conditional_file(bytes: &[u8]) -> GoogleResult<GoogleFile> {
    let mut file: serde_json::Value = decode(bytes)?;
    if let Some(object) = file.as_object_mut() {
        for (old, new) in [
            ("title", "name"),
            ("fileSize", "size"),
            ("createdDate", "createdTime"),
            ("modifiedDate", "modifiedTime"),
        ] {
            if let Some(value) = object.remove(old) {
                object.insert(new.into(), value);
            }
        }
        if object.get("properties").is_some_and(|p| p.is_array()) {
            let properties = object.remove("properties").unwrap();
            let mut private = serde_json::Map::new();
            let mut public = serde_json::Map::new();
            for property in properties.as_array().unwrap() {
                let key = property
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| GoogleApiFailure::new(GoogleApiError::InvalidResponse, None))?;
                let value = property
                    .get("value")
                    .filter(|v| v.is_string())
                    .ok_or_else(|| GoogleApiFailure::new(GoogleApiError::InvalidResponse, None))?;
                let target = match property
                    .get("visibility")
                    .and_then(|v| v.as_str())
                    .unwrap_or("PRIVATE")
                {
                    "PRIVATE" => &mut private,
                    "PUBLIC" => &mut public,
                    _ => return Err(GoogleApiFailure::new(GoogleApiError::InvalidResponse, None)),
                };
                if target.insert(key.into(), value.clone()).is_some() {
                    return Err(GoogleApiFailure::new(GoogleApiError::InvalidResponse, None));
                }
            }
            object.insert("appProperties".into(), serde_json::Value::Object(private));
            object.insert("properties".into(), serde_json::Value::Object(public));
        }
        if let Some(labels) = object.remove("labels") {
            if let Some(value) = labels.get("trashed") {
                object.insert("trashed".into(), value.clone());
            }
        }
        if let Some(parents) = object.get_mut("parents").and_then(|v| v.as_array_mut()) {
            for parent in parents {
                if let Some(id) = parent.get("id").cloned() {
                    *parent = id;
                }
            }
        }
    }
    serde_json::from_value(file)
        .map_err(|_| GoogleApiFailure::new(GoogleApiError::InvalidResponse, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conditional_private_execution_properties_keep_visibility_in_both_directions() {
        let request = GoogleFileWrite {
            app_properties: GooglePatch::Set(std::collections::BTreeMap::from([(
                "drivesyncExecution".into(),
                "attempt".into(),
            )])),
            ..Default::default()
        };
        let mut request = request;
        request.properties = GooglePatch::Set(std::collections::BTreeMap::from([(
            "public".into(),
            "preserved".into(),
        )]));
        let body: serde_json::Value =
            serde_json::from_slice(&conditional_body(&request).unwrap()).unwrap();
        assert_eq!(
            body["properties"][0],
            serde_json::json!({"key":"drivesyncExecution","value":"attempt","visibility":"PRIVATE"})
        );
        let file=conditional_file(br#"{"id":"file","title":"name","properties":[{"key":"drivesyncExecution","value":"attempt","visibility":"PRIVATE"},{"key":"public","value":"value","visibility":"PUBLIC"}]}"#).unwrap();
        assert_eq!(
            file.app_properties.unwrap()["drivesyncExecution"],
            "attempt"
        );
        assert_eq!(file.properties.unwrap()["public"], "value");
    }
}
