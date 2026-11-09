//! HTTP error translation (contract: INTERFACES.md §9.4).

use crate::providers::google::errors::failure_from_response;
use crate::providers::google::{GoogleApiError as E, GoogleApiFailure};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Fixed clock: Tue, 14 Nov 2023 22:13:20 GMT.
fn now() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_700_000_000)
}

fn fail(status: u16, retry_after: Option<&str>, body: &[u8]) -> GoogleApiFailure {
    failure_from_response(status, retry_after, body, now())
}

fn body(reason: &str) -> Vec<u8> {
    format!(
        r#"{{"error":{{"code":403,"message":"private file name.pdf","errors":[{{"domain":"global","reason":"{reason}","message":"private file name.pdf"}}]}}}}"#
    )
    .into_bytes()
}

fn details_body(reason: &str) -> Vec<u8> {
    format!(
        r#"{{"error":{{"code":403,"message":"private","details":[{{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"{reason}","metadata":{{"secret":"leak"}}}}]}}}}"#
    )
    .into_bytes()
}

/// ERR-U01: status and reason map to the documented category.
#[test]
fn maps_documented_status_and_reason_pairs() {
    let cases: Vec<(u16, Vec<u8>, E, bool)> = vec![
        (400, body("badRequest"), E::InvalidArgument, false),
        (401, body("authError"), E::AuthenticationRequired, false),
        (
            403,
            body("storageQuotaExceeded"),
            E::StorageQuotaExceeded,
            false,
        ),
        (
            403,
            body("fileNotDownloadable"),
            E::FileNotDownloadable,
            false,
        ),
        (
            403,
            body("insufficientFilePermissions"),
            E::PermissionDenied,
            false,
        ),
        (403, body("domainPolicy"), E::PermissionDenied, false),
        (
            403,
            details_body("ACCESS_TOKEN_SCOPE_INSUFFICIENT"),
            E::InsufficientScope,
            false,
        ),
        (404, body("notFound"), E::NotFoundOrInaccessible, false),
        (500, Vec::new(), E::ProviderUnavailable, true),
        (502, Vec::new(), E::ProviderUnavailable, true),
        (503, Vec::new(), E::ProviderUnavailable, true),
        (504, Vec::new(), E::ProviderUnavailable, true),
    ];
    for (status, payload, kind, retryable) in cases {
        let failure = fail(status, None, &payload);
        assert_eq!(failure.kind, kind, "status {status}");
        assert_eq!(failure.http_status, Some(status));
        assert_eq!(failure.retryable, retryable, "status {status}");
    }
}

/// ERR-U02: all documented rate-limit reasons are retryable, on 403 and 429.
#[test]
fn rate_limit_reasons_are_retryable() {
    for (status, reason) in [
        (403, "rateLimitExceeded"),
        (403, "userRateLimitExceeded"),
        (403, "sharingRateLimitExceeded"),
        (429, "rateLimitExceeded"),
    ] {
        let failure = fail(status, None, &body(reason));
        assert_eq!(failure.kind, E::RateLimited, "{status} {reason}");
        assert!(failure.retryable);
        assert_eq!(failure.retry_after_seconds, None);
    }
}

/// ERR-U03: `Retry-After` in seconds is represented once, in the failure.
#[test]
fn retry_after_seconds_is_preserved_once() {
    let failure = fail(429, Some("120"), &body("rateLimitExceeded"));
    assert_eq!(failure.kind, E::RateLimited);
    assert_eq!(failure.retry_after_seconds, Some(120));
    assert_eq!(fail(429, Some(" 7 "), &[]).retry_after_seconds, Some(7));
    assert_eq!(fail(429, Some("0"), &[]).retry_after_seconds, Some(0));
}

/// ERR-U12: `Retry-After` as an HTTP-date is converted relative to the clock.
#[test]
fn retry_after_http_date_is_converted_to_seconds() {
    let in_two_minutes = fail(503, Some("Tue, 14 Nov 2023 22:15:20 GMT"), &[]);
    assert_eq!(in_two_minutes.retry_after_seconds, Some(120));
    let obsolete_rfc850 = fail(503, Some("Tuesday, 14-Nov-23 22:15:20 GMT"), &[]);
    assert_eq!(obsolete_rfc850.retry_after_seconds, Some(120));
    let past = fail(429, Some("Mon, 13 Nov 2023 22:13:20 GMT"), &[]);
    assert_eq!(past.retry_after_seconds, Some(0), "a past date means now");
}

/// ERR-U13: `Retry-After` is kept for any failing status, once, and never
/// changes the category.
#[test]
fn retry_after_on_server_errors_does_not_change_category() {
    let failure = fail(503, Some("30"), &[]);
    assert_eq!(failure.kind, E::ProviderUnavailable);
    assert_eq!(failure.retry_after_seconds, Some(30));
    assert!(failure.retryable);
}

/// ERR-U04: an unparsable `Retry-After` is dropped, never guessed.
#[test]
fn invalid_retry_after_is_ignored() {
    for raw in [
        "abc",
        "-5",
        "",
        "1.5",
        "99999999999999999999999",
        "Tue, 99 Nov 2023 22:15:20 GMT",
        "30 seconds",
    ] {
        let failure = fail(429, Some(raw), &[]);
        assert_eq!(failure.retry_after_seconds, None, "{raw:?}");
    }
}

/// ERR-U05: one status with different reasons yields different categories.
#[test]
fn reason_takes_precedence_over_status_alone() {
    let quota = fail(403, None, &body("storageQuotaExceeded"));
    let denied = fail(403, None, &body("insufficientFilePermissions"));
    let limited = fail(403, None, &body("userRateLimitExceeded"));
    assert_ne!(quota.kind, denied.kind);
    assert_ne!(denied.kind, limited.kind);
}

/// ERR-U06: 403 without a parsable body is still a permission failure.
#[test]
fn forbidden_without_usable_body_is_permission_denied_not_transient() {
    for payload in [&b""[..], b"not json", b"{}", br#"{"error":{}}"#] {
        let failure = fail(403, None, payload);
        assert_eq!(failure.kind, E::PermissionDenied);
        assert!(!failure.retryable);
    }
}

/// ERR-U07: provider reason is a safe code and the body is never copied.
#[test]
fn failure_never_contains_provider_message_or_metadata() {
    for payload in [
        body("insufficientFilePermissions"),
        details_body("SOME_REASON"),
    ] {
        let failure = fail(403, None, &payload);
        let rendered = format!("{failure:?}");
        for secret in ["private file name", "leak", "pdf"] {
            assert!(!rendered.contains(secret), "{rendered}");
        }
    }
}

/// ERR-U08: hostile reason text is not propagated as a "safe code".
#[test]
fn unsafe_reason_text_is_not_propagated() {
    let hostile = "bad reason with spaces, token=abc <b>";
    let failure = fail(403, None, &body(hostile));
    assert_eq!(failure.provider_reason, None);
    assert_eq!(failure.kind, E::PermissionDenied);
    let long = "a".repeat(65);
    assert_eq!(fail(403, None, &body(&long)).provider_reason, None);
    let max = "a".repeat(64);
    assert_eq!(
        fail(403, None, &body(&max)).provider_reason.as_deref(),
        Some(max.as_str())
    );
}

/// ERR-U09: the reason code is preserved for diagnostics when it is safe.
#[test]
fn known_reason_code_is_preserved() {
    let failure = fail(403, None, &body("storageQuotaExceeded"));
    assert_eq!(
        failure.provider_reason.as_deref(),
        Some("storageQuotaExceeded")
    );
    let from_details =
        fail(403, None, &details_body("ACCESS_TOKEN_SCOPE_INSUFFICIENT"));
    assert_eq!(
        from_details.provider_reason.as_deref(),
        Some("ACCESS_TOKEN_SCOPE_INSUFFICIENT")
    );
}

/// ERR-U10: unexpected statuses keep their status and never look like success.
#[test]
fn unexpected_statuses_are_failures_with_status_preserved() {
    for (status, kind) in [
        (300u16, E::InvalidResponse),
        (301, E::InvalidResponse),
        (302, E::InvalidResponse),
        (304, E::InvalidResponse),
        (307, E::InvalidResponse),
        (308, E::InvalidResponse),
        (204, E::InvalidResponse),
        (418, E::InvalidResponse),
        (499, E::InvalidResponse),
        (505, E::ProviderUnavailable),
        (599, E::ProviderUnavailable),
    ] {
        let failure = fail(status, None, b"");
        assert_eq!(failure.http_status, Some(status));
        assert_eq!(failure.kind, kind, "status {status}");
    }
}

/// ERR-U11: the real 403 scope shape (`errors[]` plus `ErrorInfo` details).
#[test]
fn scope_error_with_both_reason_shapes() {
    let payload = br#"{"error":{"code":403,"message":"m","errors":[{"reason":"insufficientPermissions"}],
        "details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"ACCESS_TOKEN_SCOPE_INSUFFICIENT"}]}}"#;
    let failure = fail(403, None, payload);
    assert_eq!(failure.kind, E::InsufficientScope);
    assert_eq!(
        failure.provider_reason.as_deref(),
        Some("insufficientPermissions")
    );
    let plain = fail(403, None, &body("insufficientPermissions"));
    assert_eq!(plain.kind, E::PermissionDenied);
}

/// ERR-U14: a bare 429 (no body) is a rate limit; `dailyLimitExceeded` is quota.
#[test]
fn bare_429_is_rate_limited_and_daily_limit_is_quota() {
    let bare = fail(429, None, &[]);
    assert_eq!(bare.kind, E::RateLimited);
    assert!(bare.retryable);
    let daily = fail(403, None, &body("dailyLimitExceeded"));
    assert_eq!(daily.kind, E::QuotaExceeded);
    assert!(!daily.retryable);
}

/// ERR-U15: `Retry-After` text is never copied beyond the parsed number.
#[test]
fn retry_after_header_text_is_not_exposed() {
    let failure = fail(429, Some("secret-text"), &[]);
    assert!(!format!("{failure:?}").contains("secret-text"));
}
