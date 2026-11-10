//! Single-request resumable operations and metadata-only conveniences.
//! No queue, file reader, retry or automatic recovery query is created here.
use super::client::{decode, invalid_argument, query_for};
use super::*;
use reqwest::{Method, Url};

/// A sensitive server-issued URI, revalidated before every credentialed use.
#[derive(Clone)]
pub struct GoogleUploadSession {
    pub session_uri: String,
    pub total_bytes: Option<u64>,
}
impl std::fmt::Debug for GoogleUploadSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleUploadSession")
            .finish_non_exhaustive()
    }
}
/// Caller-owned bytes at an explicit offset. Debug never displays their content.
pub struct GoogleUploadChunk {
    pub start: u64,
    pub bytes: Vec<u8>,
    pub total_bytes: Option<u64>,
}
impl std::fmt::Debug for GoogleUploadChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleUploadChunk")
            .field("start", &self.start)
            .finish_non_exhaustive()
    }
}
/// Only a final server response confirms completion. The public contract keeps
/// GoogleFile inline; no allocation/indirection is imposed on its consumers.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum GoogleUploadProgress {
    Incomplete {
        received_range: Option<GoogleByteRange>,
    },
    Complete {
        file: GoogleFile,
    },
}

impl GoogleDriveClient {
    /// Begins a resumable create; metadata alone does not confirm file content.
    pub async fn start_resumable_create(
        &self,
        session: &GoogleSession,
        request: GoogleFileWrite,
        options: FileCreateOptions,
        total_bytes: Option<u64>,
    ) -> GoogleResult<GoogleUploadSession> {
        let query = query_for(
            &options,
            &[
                "supportsAllDrives",
                "ignoreDefaultVisibility",
                "useContentAsIndexableText",
                "keepRevisionForever",
                "fields",
            ],
        )?;
        self.start_upload(session, None, request, query, total_bytes)
            .await
    }
    /// Begins replacement content for the same remote file identity.
    pub async fn start_resumable_update(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        request: GoogleFileWrite,
        options: FileUpdateOptions,
        total_bytes: Option<u64>,
    ) -> GoogleResult<GoogleUploadSession> {
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
        self.start_upload(session, Some(file_id), request, query, total_bytes)
            .await
    }
    /// Sends one block, validating totals and non-final 256 KiB alignment.
    /// A lost response leaves effects unknown: this method never repeats it.
    pub async fn upload_chunk(
        &self,
        session: &GoogleSession,
        upload: &GoogleUploadSession,
        chunk: GoogleUploadChunk,
    ) -> GoogleResult<GoogleUploadProgress> {
        let url = self.upload_url(&upload.session_uri)?;
        if upload
            .total_bytes
            .zip(chunk.total_bytes)
            .is_some_and(|(a, b)| a != b)
        {
            return Err(invalid_argument());
        }
        let total = chunk.total_bytes.or(upload.total_bytes);
        let end = chunk
            .start
            .checked_add(chunk.bytes.len() as u64)
            .ok_or_else(invalid_argument)?;
        if total.is_some_and(|total| end > total)
            || (chunk.bytes.is_empty() && !(chunk.start == 0 && total == Some(0)))
            || (total != Some(end) && !chunk.bytes.len().is_multiple_of(256 * 1024))
        {
            return Err(invalid_argument());
        }
        let content_range = if chunk.bytes.is_empty() {
            "bytes */0".into()
        } else {
            format!(
                "bytes {}-{}/{}",
                chunk.start,
                end - 1,
                total.map(|v| v.to_string()).unwrap_or_else(|| "*".into())
            )
        };
        let headers = [
            ("Content-Range", content_range),
            ("Content-Type", "application/octet-stream".into()),
        ];
        let response = self
            .send(session, Method::PUT, url, Some(chunk.bytes), &headers)
            .await?;
        self.upload_result(response, total, Some(end)).await
    }
    /// Asks for the server-confirmed offset without replaying any content.
    pub async fn query_upload_progress(
        &self,
        session: &GoogleSession,
        upload: &GoogleUploadSession,
    ) -> GoogleResult<GoogleUploadProgress> {
        let url = self.upload_url(&upload.session_uri)?;
        let headers = [
            (
                "Content-Range",
                format!(
                    "bytes */{}",
                    upload
                        .total_bytes
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "*".into())
                ),
            ),
            ("Content-Type", "application/octet-stream".into()),
        ];
        let response = self
            .send(session, Method::PUT, url, Some(Vec::new()), &headers)
            .await?;
        self.upload_result(response, upload.total_bytes, None).await
    }
    /// Moves a resource to trash using only the metadata endpoint.
    pub async fn trash_file(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
    ) -> GoogleResult<GoogleFile> {
        self.files_update(
            session,
            file_id,
            GoogleFileWrite {
                trashed: GooglePatch::Set(true),
                ..Default::default()
            },
            FileUpdateOptions {
                supports_all_drives: Some(true),
                ..Default::default()
            },
        )
        .await
    }
    /// Renames metadata without replacing remote bytes.
    pub async fn rename_file(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        name: &str,
    ) -> GoogleResult<GoogleFile> {
        self.files_update(
            session,
            file_id,
            GoogleFileWrite {
                name: GooglePatch::Set(name.into()),
                ..Default::default()
            },
            FileUpdateOptions {
                supports_all_drives: Some(true),
                ..Default::default()
            },
        )
        .await
    }
    /// Moves via parent query parameters, without copying or reuploading bytes.
    pub async fn move_file(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        add_parent: &FileId,
        remove_parents: &[FileId],
    ) -> GoogleResult<GoogleFile> {
        self.files_update(
            session,
            file_id,
            GoogleFileWrite::default(),
            FileUpdateOptions {
                add_parents: Some(vec![add_parent.clone()]),
                remove_parents: if remove_parents.is_empty() {
                    None
                } else {
                    Some(remove_parents.into())
                },
                supports_all_drives: Some(true),
                ..Default::default()
            },
        )
        .await
    }

    async fn start_upload(
        &self,
        session: &GoogleSession,
        file_id: Option<&FileId>,
        request: GoogleFileWrite,
        mut query: Vec<(String, String)>,
        total_bytes: Option<u64>,
    ) -> GoogleResult<GoogleUploadSession> {
        let mut url = self.upload_base.clone();
        {
            let mut path = url.path_segments_mut().map_err(|_| invalid_argument())?;
            path.pop_if_empty().push("files");
            if let Some(id) = file_id {
                if matches!(id.0.as_str(), "" | "." | "..") {
                    return Err(invalid_argument());
                }
                path.push(&id.0);
            }
        }
        query.push(("uploadType".into(), "resumable".into()));
        url.query_pairs_mut().extend_pairs(query);
        let body = serde_json::to_vec(&request).map_err(|_| invalid_argument())?;
        let mut headers = vec![("X-Upload-Content-Type", "application/octet-stream".into())];
        if let Some(total) = total_bytes {
            headers.push(("X-Upload-Content-Length", total.to_string()));
        }
        let response = self
            .send(
                session,
                if file_id.is_some() {
                    Method::PATCH
                } else {
                    Method::POST
                },
                url,
                Some(body),
                &headers,
            )
            .await?;
        if response.status().as_u16() != 200 {
            return match self.response_body(response, 200).await {
                Err(error) => Err(error),
                Ok(_) => Err(invalid_response()),
            };
        }
        let uri = response
            .headers()
            .get("Location")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(invalid_response)?;
        let uri = self
            .upload_url(uri)
            .map_err(|_| invalid_response())?
            .to_string();
        Ok(GoogleUploadSession {
            session_uri: uri,
            total_bytes,
        })
    }

    /// Session URIs are opaque data, never an authorization to choose any host.
    fn upload_url(&self, raw: &str) -> GoogleResult<Url> {
        let url = Url::parse(raw).map_err(|_| invalid_argument())?;
        let prefix = format!("{}/files", self.upload_base.path().trim_end_matches('/'));
        if url.origin() != self.upload_base.origin()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || !(url.path() == prefix || url.path().starts_with(&format!("{prefix}/")))
        {
            return Err(invalid_argument());
        }
        Ok(url)
    }

    async fn upload_result(
        &self,
        response: reqwest::Response,
        total: Option<u64>,
        sent_end: Option<u64>,
    ) -> GoogleResult<GoogleUploadProgress> {
        let status = response.status().as_u16();
        if status == 308 {
            let confirmed = match response.headers().get("Range") {
                None => None,
                Some(value) => {
                    let value = value.to_str().map_err(|_| invalid_response())?;
                    let (start, end) = value
                        .strip_prefix("bytes=")
                        .and_then(|v| v.split_once('-'))
                        .ok_or_else(invalid_response)?;
                    let start: u64 = start.parse().map_err(|_| invalid_response())?;
                    let end: u64 = end.parse().map_err(|_| invalid_response())?;
                    if start != 0
                        || end == u64::MAX
                        || total.is_some_and(|n| end >= n)
                        || sent_end.is_some_and(|n| end >= n)
                    {
                        return Err(invalid_response());
                    }
                    Some(GoogleByteRange {
                        start,
                        end_inclusive: Some(end),
                    })
                }
            };
            return Ok(GoogleUploadProgress::Incomplete {
                received_range: confirmed,
            });
        }
        if status == 404 || status == 410 {
            return Err(GoogleApiFailure::new(
                GoogleApiError::ExpiredUploadSession,
                Some(status),
            ));
        }
        if status == 200 || status == 201 {
            if total.zip(sent_end).is_some_and(|(total, end)| total != end) {
                return Err(invalid_response());
            }
            let body = self.response_body(response, status).await?;
            let file: GoogleFile = decode(&body).map_err(|mut failure| {
                failure.http_status = Some(status);
                failure
            })?;
            if let (Some(total), Some(size)) = (total, file.size.as_deref()) {
                if size.parse::<u64>().ok() != Some(total) {
                    return Err(invalid_response());
                }
            }
            return Ok(GoogleUploadProgress::Complete { file });
        }
        match self.response_body(response, 200).await {
            Err(error) => Err(error),
            Ok(_) => Err(invalid_response()),
        }
    }
}
fn invalid_response() -> GoogleApiFailure {
    GoogleApiFailure::new(GoogleApiError::InvalidResponse, None)
}
