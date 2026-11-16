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
        let url = self.file_url(id, super::file_get_query(&options))?;
        let response = self.send(session, Method::GET, url, None, &[]).await?;
        let etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .filter(|v| strong_etag(v).is_ok())
            .map(str::to_owned);
        let bytes = self.response_body(response, 200).await?;
        Ok(GoogleFileSnapshot {
            file: super::parse_file(&bytes)?,
            etag,
        })
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
        let url = self.file_url(id, query)?;
        let bytes = serde_json::to_vec(&request).map_err(|_| invalid_argument())?;
        let response = self
            .send(
                session,
                Method::PATCH,
                url,
                Some(bytes),
                &[("If-Match", etag.into())],
            )
            .await?;
        decode(&self.response_body(response, 200).await?)
    }
}
