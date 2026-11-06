//! Bounded blocking HTTP requests to the Google Drive API.
//!
//! Redirects are disabled. Provider responses are capped in size, and HTTP
//! status errors are mapped to sanitized engine errors before returning bytes.
use super::errors::response_status;
use crate::{AuthError, EngineError};
use reqwest::blocking::Client;
use std::{io::Read, time::Duration};

thread_local! {
    static SYNC_CANCEL: std::cell::RefCell<Option<crate::Cancellation>> = const { std::cell::RefCell::new(None) };
}

pub(crate) fn with_cancellation<T>(cancel: crate::Cancellation, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<crate::Cancellation>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SYNC_CANCEL.replace(self.0.take());
        }
    }
    let _restore = Restore(SYNC_CANCEL.replace(Some(cancel)));
    run()
}

pub(crate) fn check_cancelled() -> Result<(), EngineError> {
    if SYNC_CANCEL.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(crate::Cancellation::is_cancelled)
    }) {
        return Err(EngineError::Authentication(AuthError::Cancelled));
    }
    Ok(())
}

/// Maximum response size for folder metadata (one mebibyte).
const MAX_BODY: u64 = 1024 * 1024;

/// Runs a read-only authenticated request and returns a bounded JSON body.
///
/// # Errors
///
/// Returns a typed [`EngineError`] for network errors, excessive response
/// size, authorization failures, unavailable providers or other HTTP errors.
/// Raw provider responses are not included in returned errors.
pub(crate) fn request(
    token: &str,
    endpoint: &str,
    params: &[(&str, &str)],
) -> Result<Vec<u8>, EngineError> {
    let (code, body) = request_status(token, endpoint, params)?;
    response_status(code, &body)?;
    Ok(body)
}

/// Performs the same bounded request while leaving status interpretation to
/// the caller. Response bodies must not be exposed in errors or logs.
pub(crate) fn request_status(
    token: &str,
    endpoint: &str,
    params: &[(&str, &str)],
) -> Result<(u16, Vec<u8>), EngineError> {
    let response = get(token, endpoint, params, Duration::from_secs(30))?;
    let code = response.status().as_u16();
    let mut body = Vec::new();
    response
        .take(MAX_BODY + 1)
        .read_to_end(&mut body)
        .map_err(|_| EngineError::Authentication(AuthError::Network))?;
    if body.len() as u64 > MAX_BODY {
        return Err(EngineError::Authentication(AuthError::InvalidResponse));
    }
    Ok((code, body))
}

/// Opens a streaming response using the same authenticated transport as metadata.
pub(crate) fn get(
    token: &str,
    endpoint: &str,
    params: &[(&str, &str)],
    timeout: Duration,
) -> Result<reqwest::blocking::Response, EngineError> {
    get_with_range(token, endpoint, params, timeout, None)
}

pub(crate) fn get_with_range(
    token: &str,
    endpoint: &str,
    params: &[(&str, &str)],
    timeout: Duration,
    offset: Option<u64>,
) -> Result<reqwest::blocking::Response, EngineError> {
    check_cancelled()?;
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(timeout)
        .user_agent("DriveSync/0.1")
        .build()
        .map_err(|_| EngineError::Authentication(AuthError::Network))?;
    let mut request = client.get(endpoint).bearer_auth(token).query(params);
    if let Some(offset) = offset {
        request = request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
    }
    let response = request.send();
    check_cancelled()?;
    response.map_err(|_| EngineError::Authentication(AuthError::Network))
}
