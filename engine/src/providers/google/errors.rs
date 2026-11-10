//! Public error model of the Google provider.

/// Result of any Google provider call.
pub type GoogleResult<T> = Result<T, GoogleApiFailure>;

/// Stable error categories, independent of provider message text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleApiError {
    InvalidGrant,
    InsufficientScope,
    StorageQuotaExceeded,
    FileNotDownloadable,
    InvalidArgument,
    AuthenticationRequired,
    ReauthorizationRequired,
    PermissionDenied,
    NotFoundOrInaccessible,
    PreconditionFailed,
    RateLimited,
    QuotaExceeded,
    ProviderUnavailable,
    Network,
    Timeout,
    Cancelled,
    InvalidResponse,
    UnsupportedOperation,
    UnsupportedMimeType,
    ExpiredCursor,
    ExpiredUploadSession,
    IncompleteTransfer,
}

/// Sanitized failure of a provider call.
///
/// Never contains credentials, raw response bodies or provider messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleApiFailure {
    pub kind: GoogleApiError,
    pub http_status: Option<u16>,
    /// Safe provider reason code only (for example `rateLimitExceeded`).
    pub provider_reason: Option<String>,
    pub retry_after_seconds: Option<u64>,
    pub retryable: bool,
}

impl GoogleApiFailure {
    /// Constructs a failure without retaining transport or response text.
    pub(super) fn new(kind: GoogleApiError, http_status: Option<u16>) -> Self {
        let retryable = matches!(
            kind,
            GoogleApiError::Network
                | GoogleApiError::Timeout
                | GoogleApiError::RateLimited
                | GoogleApiError::ProviderUnavailable
        );
        Self {
            kind,
            http_status,
            provider_reason: None,
            retry_after_seconds: None,
            retryable,
        }
    }
}

/// Translates a bounded error body without retaining messages or metadata.
/// The supplied clock makes HTTP-date conversion deterministic in tests.
pub(crate) fn failure_from_response(
    status: u16,
    retry_after: Option<&str>,
    body: &[u8],
    now: std::time::SystemTime,
) -> GoogleApiFailure {
    use GoogleApiError as E;
    let json: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();
    let error = &json["error"];
    let reasons: Vec<&str> = error["errors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["reason"].as_str())
        .collect();
    let details: Vec<&str> = error["details"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry["@type"] == "type.googleapis.com/google.rpc.ErrorInfo")
        .filter_map(|entry| entry["reason"].as_str())
        .collect();
    let has = |reason| reasons.contains(&reason) || details.contains(&reason);
    let kind = match status {
        400 => E::InvalidArgument,
        401 => E::AuthenticationRequired,
        403 if has("storageQuotaExceeded") => E::StorageQuotaExceeded,
        403 if has("fileNotDownloadable") => E::FileNotDownloadable,
        403 if has("ACCESS_TOKEN_SCOPE_INSUFFICIENT") => E::InsufficientScope,
        403 if has("rateLimitExceeded")
            || has("userRateLimitExceeded")
            || has("sharingRateLimitExceeded") =>
        {
            E::RateLimited
        }
        403 if has("dailyLimitExceeded") => E::QuotaExceeded,
        403 => E::PermissionDenied,
        404 => E::NotFoundOrInaccessible,
        409 | 412 => E::PreconditionFailed,
        429 => E::RateLimited,
        500..=599 => E::ProviderUnavailable,
        _ => E::InvalidResponse,
    };
    let mut failure = GoogleApiFailure::new(kind, Some(status));
    failure.provider_reason = reasons
        .first()
        .or_else(|| details.first())
        .filter(|code| {
            !code.is_empty()
                && code.len() <= 64
                && code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
        .map(|code| (*code).to_owned());
    failure.retry_after_seconds = retry_after.and_then(|raw| {
        let raw = raw.trim();
        if !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()) {
            raw.parse().ok()
        } else {
            httpdate::parse_http_date(raw)
                .ok()
                .map(|date| date.duration_since(now).unwrap_or_default().as_secs())
        }
    });
    failure
}
