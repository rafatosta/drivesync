//! Incremental media and export transport, with explicit partial-read errors.
use super::client::invalid_argument;
use super::*;
use md5::Md5;
use sha2::{Digest, Sha256};
use std::time::Duration;

/// Inclusive HTTP byte range. A missing end means the remaining bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleByteRange {
    pub start: u64,
    pub end_inclusive: Option<u64>,
}

/// Incremental HTTP body with a per-read idle timeout. A read failure is sticky:
/// later reads cannot convert an incomplete response into successful EOF.
/// Debug never prints the URL.
pub struct GoogleByteStream {
    response: Option<reqwest::Response>,
    expected: Option<u64>,
    read_timeout: Duration,
    received: u64,
    failure: Option<GoogleApiFailure>,
}
impl std::fmt::Debug for GoogleByteStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleByteStream")
            .field("received", &self.received)
            .finish_non_exhaustive()
    }
}
impl GoogleByteStream {
    /// Reads at most one transport chunk. None means HTTP EOF; a known length
    /// must match. Without a length this does not prove whole-file integrity.
    pub async fn next_chunk(&mut self) -> GoogleResult<Option<Vec<u8>>> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let Some(response) = self.response.as_mut() else {
            return Ok(None);
        };
        let next = tokio::time::timeout(self.read_timeout, response.chunk()).await;
        let failure = match next {
            Ok(Ok(Some(bytes))) => {
                if let Some(total) = self.received.checked_add(bytes.len() as u64) {
                    if self.expected.is_none_or(|expected| total <= expected) {
                        self.received = total;
                        return Ok(Some(bytes.to_vec()));
                    }
                }
                GoogleApiError::IncompleteTransfer
            }
            Ok(Ok(None))
                if self
                    .expected
                    .is_none_or(|expected| expected == self.received) =>
            {
                self.response = None;
                return Ok(None);
            }
            Ok(Ok(None)) => GoogleApiError::IncompleteTransfer,
            Ok(Err(error)) if error.is_timeout() => GoogleApiError::Timeout,
            Ok(Err(_)) => GoogleApiError::IncompleteTransfer,
            Err(_) => GoogleApiError::Timeout,
        };
        self.response = None;
        let failure = GoogleApiFailure::new(failure, None);
        self.failure = Some(failure.clone());
        Err(failure)
    }
}

/// A binary response and technical HTTP metadata, not synchronization state.
#[derive(Debug)]
pub struct GoogleDownloadStream {
    pub body: GoogleByteStream,
    pub http_status: u16,
    pub content_length: Option<u64>,
    pub content_range: Option<String>,
    pub etag: Option<String>,
    pub mime_type: Option<MimeType>,
}
impl GoogleDriveClient {
    /// Recover SHA-256 evidence for a binary revision that only exposes an MD5.
    /// MD5 checks transport integrity while SHA-256 remains the replica identity evidence.
    pub(crate) async fn fill_content_sha256(
        &self,
        session: &GoogleSession,
        file: &mut GoogleFile,
        cached_sha256: Option<&str>,
    ) -> GoogleResult<()> {
        let (Some(id), Some(_version), Some(expected), Some(size)) = (
            file.id.as_ref(),
            file.version
                .as_ref()
                .and_then(|value| value.parse::<u64>().ok()),
            file.md5_checksum.as_deref().filter(|value| {
                value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
            }),
            file.size
                .as_ref()
                .and_then(|value| value.parse::<u64>().ok()),
        ) else {
            return Ok(());
        };
        if file.sha256_checksum.is_some()
            || file.trashed.is_none()
            || file
                .mime_type
                .as_ref()
                .is_none_or(|mime| mime.0.starts_with("application/vnd.google-apps."))
            || !file
                .capabilities
                .as_ref()
                .is_some_and(|caps| caps.can_download == Some(true))
        {
            return Ok(());
        }
        if let Some(cached) = cached_sha256 {
            file.sha256_checksum = Some(cached.to_owned());
            return Ok(());
        }

        let _sample = crate::performance::Sample::begin("google_sha256_media_and_verification");
        let mut media = self.get_file_bytes(session, id, None).await?;
        let mut received = 0u64;
        let mut md5 = Md5::new();
        let mut sha256 = Sha256::new();
        let incomplete = || GoogleApiFailure::new(GoogleApiError::IncompleteTransfer, None);
        while let Some(bytes) = media.body.next_chunk().await? {
            received = received
                .checked_add(bytes.len() as u64)
                .filter(|count| *count <= size)
                .ok_or_else(incomplete)?;
            md5.update(&bytes);
            sha256.update(&bytes);
        }
        if received != size || !format!("{:x}", md5.finalize()).eq_ignore_ascii_case(expected) {
            return Err(incomplete());
        }
        let after = self
            .files_get(
                session,
                id,
                FileGetOptions {
                    fields: Some(FieldMask(
                        "id,version,size,md5Checksum,headRevisionId,trashed".into(),
                    )),
                    supports_all_drives: Some(true),
                    ..Default::default()
                },
            )
            .await?;
        if after.id != file.id
            || after.version != file.version
            || after.head_revision_id != file.head_revision_id
            || after.size != file.size
            || after.md5_checksum != file.md5_checksum
            || after.trashed != file.trashed
        {
            return Err(incomplete());
        }
        file.sha256_checksum = Some(format!("{:x}", sha256.finalize()));
        Ok(())
    }

    /// Reads blob bytes using alt=media. A range requires a consistent 206 reply.
    pub async fn get_file_bytes(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        range: Option<GoogleByteRange>,
    ) -> GoogleResult<GoogleDownloadStream> {
        self.download(session, &["files", &file_id.0], &[("alt", "media")], range)
            .await
    }
    /// Reads one blob revision; this does not export Workspace revisions.
    pub async fn get_revision_bytes(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        revision_id: &RevisionId,
        range: Option<GoogleByteRange>,
    ) -> GoogleResult<GoogleDownloadStream> {
        self.download(
            session,
            &["files", &file_id.0, "revisions", &revision_id.0],
            &[("alt", "media")],
            range,
        )
        .await
    }
    /// Exports Workspace content in the requested MIME type, as a stream.
    pub async fn export_file_bytes(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        mime_type: &MimeType,
    ) -> GoogleResult<GoogleDownloadStream> {
        if mime_type.0.is_empty() {
            return Err(invalid_argument());
        }
        self.download(
            session,
            &["files", &file_id.0, "export"],
            &[("mimeType", &mime_type.0)],
            None,
        )
        .await
    }
    /// Official files.export operation; exposes the same incremental body.
    pub async fn files_export(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        mime_type: &MimeType,
    ) -> GoogleResult<GoogleByteStream> {
        Ok(self
            .export_file_bytes(session, file_id, mime_type)
            .await?
            .body)
    }

    async fn download(
        &self,
        session: &GoogleSession,
        segments: &[&str],
        query: &[(&str, &str)],
        range: Option<GoogleByteRange>,
    ) -> GoogleResult<GoogleDownloadStream> {
        let mut headers = Vec::new();
        if let Some(range) = &range {
            if range.end_inclusive.is_some_and(|end| end < range.start) {
                return Err(invalid_argument());
            }
            headers.push((
                "Range",
                format!(
                    "bytes={}-{}",
                    range.start,
                    range
                        .end_inclusive
                        .map(|v| v.to_string())
                        .unwrap_or_default()
                ),
            ));
        }
        let mut url = self.drive_base.clone();
        {
            let mut path = url.path_segments_mut().map_err(|_| invalid_argument())?;
            path.pop_if_empty();
            for segment in segments {
                if matches!(*segment, "" | "." | "..") {
                    return Err(invalid_argument());
                }
                path.push(segment);
            }
        }
        url.query_pairs_mut().extend_pairs(query);
        let response = self
            .send_stream(session, reqwest::Method::GET, url, None, &headers)
            .await?;
        let status = response.status().as_u16();
        let invalid = || GoogleApiFailure::new(GoogleApiError::InvalidResponse, Some(status));
        let expected_status = if range.is_some() { 206 } else { 200 };
        if status != expected_status {
            return match self.response_body(response, expected_status).await {
                Err(error) => Err(error),
                Ok(_) => Err(invalid()),
            };
        }
        let header = |name| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let content_range = header("content-range");
        let content_length = response.content_length();
        let mut expected = content_length;
        if let Some(range) = range {
            let (start, end, total) = content_range
                .as_deref()
                .and_then(parse_content_range)
                .ok_or_else(invalid)?;
            if start != range.start
                || range.end_inclusive.is_some_and(|requested| {
                    end > requested || (end < requested && total != end.checked_add(1))
                })
            {
                return Err(invalid());
            }
            let length = end
                .checked_sub(start)
                .and_then(|n| n.checked_add(1))
                .ok_or_else(invalid)?;
            if content_length.is_some_and(|length_header| length_header != length) {
                return Err(invalid());
            }
            expected = Some(length);
        }
        let etag = header("etag");
        let mime_type = header("content-type").map(MimeType);
        Ok(GoogleDownloadStream {
            body: GoogleByteStream {
                response: Some(response),
                expected,
                read_timeout: self.limits.request_timeout,
                received: 0,
                failure: None,
            },
            http_status: status,
            content_length,
            content_range,
            etag,
            mime_type,
        })
    }
}

/// Parses the response byte interval, rejecting overflow and contradictory totals.
fn parse_content_range(value: &str) -> Option<(u64, u64, Option<u64>)> {
    let (range, total) = value.strip_prefix("bytes ")?.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let (start, end) = (start.parse().ok()?, end.parse().ok()?);
    let total = if total == "*" {
        None
    } else {
        Some(total.parse().ok()?)
    };
    if end < start || total.is_some_and(|total| end >= total) {
        return None;
    }
    Some((start, end, total))
}
