//! Bounded asynchronous HTTP transport for Google Drive metadata.

use std::time::{Duration, SystemTime};

use reqwest::{header, redirect::Policy, Client, Url};

use super::errors::failure_from_response;
use super::{
    file_get_query, parse_file, FileGetOptions, FileId, GoogleApiError as E, GoogleApiFailure,
    GoogleFile, GoogleResult, GoogleSession,
};

const UPLOAD_BASE: &str = "https://www.googleapis.com/upload/drive/v3";
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
    pub(super) drive_base: Url,
    pub(super) upload_base: Url,
    pub(super) http: GoogleResult<Client>,
    pub(super) limits: GoogleClientLimits,
    #[cfg(feature = "test-endpoints")]
    pub(super) oauth_base: Option<Url>,
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
            upload_base: Url::parse(UPLOAD_BASE).expect("constant upload URL is valid"),
            #[cfg(feature = "test-endpoints")]
            oauth_base: None,
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
        let upload_base = validate(upload_base.into())?;
        let mut client = Self::from_base(drive_base);
        client.upload_base = upload_base;
        Ok(client)
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
        let body = self
            .rest(
                session,
                reqwest::Method::GET,
                &["files", &file_id.0],
                "",
                file_get_query(&options),
                None,
            )
            .await?;
        parse_file(&body).map_err(|mut failure| {
            failure.http_status = Some(200);
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
pub(super) fn transport_failure(error: reqwest::Error) -> GoogleApiFailure {
    GoogleApiFailure::new(
        if error.is_timeout() {
            E::Timeout
        } else {
            E::Network
        },
        None,
    )
}

/// Creates a sanitized local input failure.
pub(super) fn invalid_argument() -> GoogleApiFailure {
    GoogleApiFailure::new(E::InvalidArgument, None)
}

/// Converts endpoint-specific options, omitting unsupported shared fields.
pub(super) fn query_for<T: serde::Serialize>(
    options: &T,
    allowed: &[&str],
) -> GoogleResult<Vec<(String, String)>> {
    let value = serde_json::to_value(options).map_err(|_| invalid_argument())?;
    let mut query = Vec::new();
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            if value.is_null() || !allowed.contains(&key.as_str()) {
                continue;
            }
            let text = match value {
                serde_json::Value::String(value) => value.clone(),
                serde_json::Value::Array(values) => values
                    .iter()
                    .map(|v| v.as_str().ok_or_else(invalid_argument))
                    .collect::<GoogleResult<Vec<_>>>()?
                    .join(","),
                serde_json::Value::Bool(_) | serde_json::Value::Number(_) => value.to_string(),
                _ => return Err(invalid_argument()),
            };
            query.push((key.clone(), text));
        }
    }
    Ok(query)
}

/// Decodes only complete JSON objects, without exposing parser diagnostics.
pub(super) fn decode<T: serde::de::DeserializeOwned>(body: &[u8]) -> GoogleResult<T> {
    if body.iter().copied().find(|b| !b.is_ascii_whitespace()) != Some(b'{') {
        return Err(GoogleApiFailure::new(E::InvalidResponse, None));
    }
    serde_json::from_slice(body).map_err(|_| GoogleApiFailure::new(E::InvalidResponse, None))
}

impl GoogleDriveClient {
    /// Executes one bounded REST call. The method determines its expected status;
    /// mutations are never retried and a lost response leaves effects uncertain.
    pub(super) async fn rest(
        &self,
        session: &GoogleSession,
        method: reqwest::Method,
        segments: &[&str],
        suffix: &str,
        query: Vec<(String, String)>,
        body: Option<Vec<u8>>,
    ) -> GoogleResult<Vec<u8>> {
        let expected = if method == reqwest::Method::DELETE {
            204
        } else {
            200
        };
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
        if !suffix.is_empty() {
            url.set_path(&format!("{}{suffix}", url.path()));
        }
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        let response = self.send(session, method, url, body, &[]).await?;
        self.response_body(response, expected).await
    }

    /// Adds credentials to this request only; no URL or raw error is propagated.
    pub(super) async fn send(
        &self,
        session: &GoogleSession,
        method: reqwest::Method,
        url: Url,
        body: Option<Vec<u8>>,
        extra: &[(&str, String)],
    ) -> GoogleResult<reqwest::Response> {
        if session.access_token.is_empty() {
            return Err(GoogleApiFailure::new(E::AuthenticationRequired, None));
        }
        let mut authorization =
            header::HeaderValue::from_str(&format!("Bearer {}", session.access_token))
                .map_err(|_| GoogleApiFailure::new(E::AuthenticationRequired, None))?;
        authorization.set_sensitive(true);
        let mut request = self
            .http
            .as_ref()
            .map_err(Clone::clone)?
            .request(method, url)
            .header(header::AUTHORIZATION, authorization)
            .timeout(self.limits.request_timeout);
        if let Some(body) = body {
            request = request.body(body);
            if !extra
                .iter()
                .any(|(key, _)| key.eq_ignore_ascii_case("content-type"))
            {
                request = request.header(header::CONTENT_TYPE, "application/json");
            }
        }
        for (key, value) in extra {
            request = request.header(*key, value);
        }
        request.send().await.map_err(transport_failure)
    }

    /// Bounded response reading also applies to errors and replies without length.
    pub(super) async fn response_body(
        &self,
        mut response: reqwest::Response,
        expected: u16,
    ) -> GoogleResult<Vec<u8>> {
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let now = SystemTime::now();
        let failure = || failure_from_response(status, retry_after.as_deref(), &[], now);
        if status != expected && !(400..=599).contains(&status) {
            return Err(failure());
        }
        let limit = self.limits.max_metadata_response_bytes;
        if response.content_length().is_some_and(|n| n > limit as u64) {
            return Err(failure());
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport_failure)? {
            if chunk.len() > limit - body.len() {
                return Err(failure());
            }
            body.extend_from_slice(&chunk);
        }
        if status != expected {
            return Err(failure_from_response(
                status,
                retry_after.as_deref(),
                &body,
                now,
            ));
        }
        Ok(body)
    }
}

/// A metadata parser failure follows a received HTTP 200 response.
pub(super) fn decode_http<T: serde::de::DeserializeOwned>(body: &[u8]) -> GoogleResult<T> {
    decode(body).map_err(|mut error| {
        error.http_status = Some(200);
        error
    })
}

/// Only changes endpoints interpret 410 as an expired change cursor.
pub(super) fn cursor_failure(mut failure: GoogleApiFailure) -> GoogleApiFailure {
    if failure.http_status == Some(410) {
        failure.kind = E::ExpiredCursor;
        failure.retryable = false;
    }
    failure
}
