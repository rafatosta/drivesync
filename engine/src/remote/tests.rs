//! Regression tests for folder parsing and sanitized Drive errors.
use super::{
    errors::{response_status, status},
    folders::{decode_folder, decode_page},
};
use crate::{AuthError, EngineError};

#[cfg(test)]
mod folder_tests {
    use super::*;
    #[test]
    fn folders_and_pagination_require_real_folder_metadata() {
        let page = decode_page(br#"{"files":[{"id":"folder-A","name":"Projects","mimeType":"application/vnd.google-apps.folder","trashed":false}],"nextPageToken":"page-2"}"#).unwrap();
        assert_eq!(page.folders[0].id, "folder-A");
        assert_eq!(page.next_page.as_deref(), Some("page-2"));
        assert!(decode_page(br#"{"files":[],"incompleteSearch":true}"#).is_err());
        assert!(decode_folder(
            br#"{"id":"x","name":"file","mimeType":"text/plain","trashed":false}"#
        )
        .is_err());
        assert!(decode_folder(br#"{"id":"x","name":"trash","mimeType":"application/vnd.google-apps.folder","trashed":true}"#).is_err());
    }
    #[test]
    fn provider_failures_are_distinct_and_sanitized() {
        assert_eq!(status(403), Err(EngineError::RemoteForbidden));
        assert_eq!(status(404), Err(EngineError::RemoteFolderUnavailable));
        assert_eq!(status(429), Err(EngineError::RemoteRateLimited));
        assert_eq!(
            status(401),
            Err(EngineError::Authentication(
                AuthError::ReauthorizationRequired
            ))
        );
        assert!(status(503).is_err());
    }
}

#[cfg(test)]
mod permission_tests {
    use super::*;
    #[test]
    fn forbidden_reasons_distinguish_disabled_api_scope_quota_and_file_access() {
        for body in [br#"{"error":{"errors":[{"reason":"accessNotConfigured"}]}}"#.as_slice(), br#"{"error":{"details":[{"reason":"SERVICE_DISABLED","metadata":{"private":"must-not-leak"}}]}}"#] {
            assert_eq!(response_status(403, body), Err(EngineError::DriveApiDisabled));
        }
        assert_eq!(
            response_status(
                403,
                br#"{"error":{"details":[{"reason":"ACCESS_TOKEN_SCOPE_INSUFFICIENT"}]}}"#
            ),
            Err(EngineError::Authentication(AuthError::MissingPermissions))
        );
        assert_eq!(
            response_status(
                403,
                br#"{"error":{"errors":[{"reason":"insufficientPermissions"}]}}"#
            ),
            Err(EngineError::Authentication(AuthError::MissingPermissions))
        );
        assert_eq!(
            response_status(
                403,
                br#"{"error":{"errors":[{"reason":"userRateLimitExceeded"}]}}"#
            ),
            Err(EngineError::RemoteRateLimited)
        );
        for body in [br#"{"error":{"errors":[{"reason":"insufficientFilePermissions"}],"message":"private-file-name"}}"#.as_slice(), b"not JSON", br#"{"error":{"message":"SERVICE_DISABLED"}}"#] {
            assert_eq!(response_status(403, body), Err(EngineError::RemoteForbidden));
        }
        assert_eq!(
            response_status(401, b""),
            Err(EngineError::Authentication(
                AuthError::ReauthorizationRequired
            ))
        );
        assert!(response_status(200, b"{}").is_ok());
    }
}
