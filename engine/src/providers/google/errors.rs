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
    RateLimited { retry_after_seconds: Option<u64> },
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

/// Translates an HTTP error response into a sanitized failure.
///
/// `retry_after` is the raw `Retry-After` header value, if any. Only reason
/// codes are read from `body`; messages are never copied.
// RED: behaviour pending. Signature is internal and may change during GREEN.
pub(crate) fn failure_from_response(
    _status: u16,
    _retry_after: Option<&str>,
    _body: &[u8],
) -> GoogleApiFailure {
    unimplemented!("HTTP error translation (RED)")
}
