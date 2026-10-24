//! Converts Drive HTTP errors into safe application error variants.
//!
//! Only allowlisted provider reason codes are inspected; raw error messages,
//! file names, URLs and other response data must not be surfaced.
use super::models::ErrorEnvelope;
use crate::{AuthError, EngineError};

/// Checks the response status, including Drive-specific HTTP 403 reasons.
///
/// # Errors
///
/// Returns a sanitized [`EngineError`] unless the response succeeded.
pub(crate) fn response_status(code: u16, body: &[u8]) -> Result<(), EngineError> {
    if code != 403 {
        return status(code);
    }
    Err(forbidden_reason(body))
}
fn forbidden_reason(body: &[u8]) -> EngineError {
    // Read only allowlisted reason codes. Never show provider messages, links,
    // metadata or raw response bodies, which can include private identifiers.
    let Ok(envelope) = serde_json::from_slice::<ErrorEnvelope>(body) else {
        return EngineError::RemoteForbidden;
    };
    let reasons: Vec<_> = envelope
        .error
        .errors
        .iter()
        .chain(envelope.error.details.iter())
        .filter_map(|r| r.reason.as_deref())
        .collect();
    if reasons.iter().any(|r| {
        matches!(
            *r,
            "SERVICE_DISABLED" | "accessNotConfigured" | "serviceDisabled"
        )
    }) {
        EngineError::DriveApiDisabled
    } else if reasons.iter().any(|r| {
        matches!(
            *r,
            "ACCESS_TOKEN_SCOPE_INSUFFICIENT" | "insufficientPermissions"
        )
    }) {
        EngineError::Authentication(AuthError::MissingPermissions)
    } else if reasons.iter().any(|r| {
        matches!(
            *r,
            "RATE_LIMIT_EXCEEDED"
                | "QUOTA_EXCEEDED"
                | "rateLimitExceeded"
                | "userRateLimitExceeded"
                | "dailyLimitExceeded"
                | "quotaExceeded"
        )
    }) {
        EngineError::RemoteRateLimited
    } else {
        EngineError::RemoteForbidden
    }
}
pub(super) fn status(code: u16) -> Result<(), EngineError> {
    Err(match code {
        200 => return Ok(()),
        401 => EngineError::Authentication(AuthError::ReauthorizationRequired),
        403 => EngineError::RemoteForbidden,
        404 => EngineError::RemoteFolderUnavailable,
        429 => EngineError::RemoteRateLimited,
        500..=599 => EngineError::Authentication(AuthError::ProviderUnavailable),
        _ => EngineError::Authentication(AuthError::InvalidResponse),
    })
}

/// Only cursor-specific rejections request a reconciliation. A generic 400 may
/// indicate a programming error and must not silently reset observation state.
pub(super) fn change_response_status(code: u16, body: &[u8]) -> Result<(), EngineError> {
    let rejected_parameter = code == 400
        && serde_json::from_slice::<serde_json::Value>(body)
            .ok()
            .and_then(|value| value.get("error")?.get("errors")?.as_array().cloned())
            .is_some_and(|errors| {
                errors.iter().any(|error| {
                    error.get("location").and_then(|v| v.as_str()) == Some("pageToken")
                        && error.get("locationType").and_then(|v| v.as_str()) == Some("parameter")
                        && matches!(
                            error.get("reason").and_then(|v| v.as_str()),
                            Some("invalid" | "invalidParameter" | "badRequest")
                        )
                })
            });
    if code == 410 || rejected_parameter {
        Err(EngineError::ChangeCursorRejected)
    } else {
        response_status(code, body)
    }
}
#[cfg(test)]
mod change_tests {
    use super::*;
    #[test]
    fn only_cursor_rejections_trigger_reconciliation() {
        assert_eq!(
            change_response_status(410, b""),
            Err(EngineError::ChangeCursorRejected)
        );
        let body=br#"{"error":{"errors":[{"reason":"invalid","locationType":"parameter","location":"pageToken"}]}}"#;
        assert_eq!(
            change_response_status(400, body),
            Err(EngineError::ChangeCursorRejected)
        );
        assert_eq!(
            response_status(410, b""),
            Err(EngineError::Authentication(
                crate::AuthError::InvalidResponse
            ))
        );
        for body in [br#"{}"#.as_slice(),br#"{"error":{"errors":[{"reason":"invalid","locationType":"parameter","location":"fields"}]}}"#] {
            assert_eq!(change_response_status(400,body),Err(EngineError::Authentication(crate::AuthError::InvalidResponse)));
        }
        assert_eq!(
            change_response_status(429, b""),
            Err(EngineError::RemoteRateLimited)
        );
    }
}
