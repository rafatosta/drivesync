//! Bounded blocking HTTP requests to the Google Drive API.
//!
//! Redirects are disabled. Provider responses are capped in size, and HTTP
//! status errors are mapped to sanitized engine errors before returning bytes.
use super::errors::response_status;
use crate::{AuthError, EngineError};
use reqwest::blocking::Client;
use std::{io::Read, time::Duration};

/// Maximum response size for folder metadata (one mebibyte).
const MAX_BODY: u64 = 1024 * 1024;

/// Runs a read-only authenticated request and returns a bounded JSON body.
///
/// # Errors
///
/// Returns a typed [`EngineError`] for network errors, excessive response
/// size, authorization failures, unavailable providers or other HTTP errors.
/// Raw provider responses are not included in returned errors.
pub(super) fn request(token: &str, endpoint: &str, params: &[(&str, &str)]) -> Result<Vec<u8>, EngineError> {
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .user_agent("DriveSync/0.1")
        .build()
        .map_err(|_| EngineError::Authentication(AuthError::Network))?;
    let response = client
        .get(endpoint)
        .bearer_auth(token)
        .query(params)
        .send()
        .map_err(|_| EngineError::Authentication(AuthError::Network))?;
    let code = response.status().as_u16();
    let mut body = Vec::new();
    response
        .take(MAX_BODY + 1)
        .read_to_end(&mut body)
        .map_err(|_| EngineError::Authentication(AuthError::Network))?;
    if body.len() as u64 > MAX_BODY {
        return Err(EngineError::Authentication(AuthError::InvalidResponse));
    }
    response_status(code, &body)?;
    Ok(body)
}
