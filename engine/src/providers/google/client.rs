//! Bounded asynchronous HTTP transport for Google Drive metadata.

use std::time::{Duration, SystemTime};

use reqwest::{header, redirect::Policy, Client, Url};

use super::errors::failure_from_response;
use super::{
    file_get_query, parse_file, FileGetOptions, FileId, GoogleApiError as E, GoogleApiFailure,
    GoogleFile, GoogleResult, GoogleSession,
};

const DRIVE_BASE: &str = "https://www.googleapis.com/drive/v3";

/// Technical metadata bounds. Production uses a 30-second total timeout and
/// an 8 MiB response limit; only the test feature permits overrides.
#[derive(Debug, Clone, Copy)]
pub struct GoogleClientLimits {
    pub request_timeout: Duration,
    pub max_metadata_response_bytes: usize,
}

impl Default for GoogleClientLimits {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            max_metadata_response_bytes: 8 * 1024 * 1024,
        }
    }
}

/// Independent Drive client with no stored credentials, retries or scheduler.
/// Clones share the connection pool; authentication is attached per request.
#[derive(Debug, Clone)]
pub struct GoogleDriveClient {
    drive_base: Url,
    http: GoogleResult<Client>,
    limits: GoogleClientLimits,
}

impl GoogleDriveClient {
    /// Creates a client for Google's official endpoint. Transport initialization
    /// errors are sanitized and returned by the first operation.
    pub fn new() -> Self {
        Self::from_base(Url::parse(DRIVE_BASE).expect("constant Google URL is valid"))
    }

    fn from_base(drive_base: Url) -> Self {
        Self {
            drive_base,
            // Disable proxy discovery as well: tokens must go only to the selected endpoint.
            http: Client::builder()
                .redirect(Policy::none())
                .no_proxy()
                .build()
                .map_err(|_| GoogleApiFailure::new(E::Network, None)),
            limits: GoogleClientLimits::default(),
        }
    }

    /// Accepts only HTTP URLs with literal loopback IPs and no user info,
    /// query or fragment. Both bases are validated; upload is not implemented.
    #[cfg(feature = "test-endpoints")]
    pub fn with_base_urls(
        drive_base: impl Into<String>,
        upload_base: impl Into<String>,
    ) -> GoogleResult<Self> {
        fn validate(raw: String) -> GoogleResult<Url> {
            let invalid = || GoogleApiFailure::new(E::InvalidArgument, None);
            let url = Url::parse(&raw).map_err(|_| invalid())?;
            let loopback = url
                .host_str()
                .and_then(|host| {
                    host.trim_start_matches('[')
                        .trim_end_matches(']')
                        .parse::<std::net::IpAddr>()
                        .ok()
                })
                .is_some_and(|ip| ip.is_loopback());
            if url.scheme() != "http"
                || !loopback
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(invalid());
            }
            Ok(url)
        }
        let drive_base = validate(drive_base.into())?;
        validate(upload_base.into())?;
        Ok(Self::from_base(drive_base))
    }

    /// Overrides positive request limits for tests only.
    #[cfg(feature = "test-endpoints")]
    pub fn with_limits(mut self, limits: GoogleClientLimits) -> GoogleResult<Self> {
        if limits.request_timeout.is_zero() || limits.max_metadata_response_bytes == 0 {
            return Err(GoogleApiFailure::new(E::InvalidArgument, None));
        }
        self.limits = limits;
        Ok(self)
    }

    /// Reads metadata using `GET /files/{fileId}` on a Tokio runtime.
    /// IDs are encoded as one segment; empty and dot IDs are rejected locally.
    /// Only HTTP 200 with a complete, bounded JSON object succeeds. Redirects
    /// are rejected. No retry is performed. Dropping the future cancels waiting;
    /// it does not establish whether Google received the request.
    pub async fn files_get(
        &self,
        session: &GoogleSession,
        file_id: &FileId,
        options: FileGetOptions,
    ) -> GoogleResult<GoogleFile> {
        if matches!(file_id.0.as_str(), "" | "." | "..") {
            return Err(GoogleApiFailure::new(E::InvalidArgument, None));
        }
        let mut authorization =
            header::HeaderValue::from_str(&format!("Bearer {}", session.access_token))
                .map_err(|_| GoogleApiFailure::new(E::AuthenticationRequired, None))?;
        if session.access_token.is_empty() {
            return Err(GoogleApiFailure::new(E::AuthenticationRequired, None));
        }
        authorization.set_sensitive(true);
        let mut url = self.drive_base.clone();
        url.path_segments_mut()
            .map_err(|_| GoogleApiFailure::new(E::InvalidArgument, None))?
            .pop_if_empty()
            .push("files")
            .push(&file_id.0);
        let query = file_get_query(&options);
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        let mut response = self
            .http
            .as_ref()
            .map_err(Clone::clone)?
            .get(url)
            .header(header::AUTHORIZATION, authorization)
            .timeout(self.limits.request_timeout)
            .send()
            .await
            .map_err(transport_failure)?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let now = SystemTime::now();
        if status != 200 && !(400..=599).contains(&status) {
            return Err(failure_from_response(
                status,
                retry_after.as_deref(),
                &[],
                now,
            ));
        }
        let limit = self.limits.max_metadata_response_bytes;
        let oversized = || {
            if status == 200 {
                GoogleApiFailure::new(E::InvalidResponse, Some(status))
            } else {
                failure_from_response(status, retry_after.as_deref(), &[], now)
            }
        };
        if response
            .content_length()
            .is_some_and(|size| size > limit as u64)
        {
            return Err(oversized());
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport_failure)? {
            if chunk.len() > limit - body.len() {
                return Err(oversized());
            }
            body.extend_from_slice(&chunk);
        }
        if status != 200 {
            return Err(failure_from_response(
                status,
                retry_after.as_deref(),
                &body,
                now,
            ));
        }
        parse_file(&body).map_err(|mut failure| {
            failure.http_status = Some(status);
            failure
        })
    }
}

impl Default for GoogleDriveClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Discards reqwest's URL and source chain, which can contain private data.
fn transport_failure(error: reqwest::Error) -> GoogleApiFailure {
    GoogleApiFailure::new(
        if error.is_timeout() {
            E::Timeout
        } else {
            E::Network
        },
        None,
    )
}
