//! Binary media transport, independent from staging and local installation.
use crate::{configuration::valid_id, AuthError, EngineError, InventoryEntry};
use reqwest::blocking::{Client, Response};
use std::{io::Read, time::Duration};

/// Reads current metadata through the same strict decoder as the inventory.
pub(crate) fn metadata(
    token: &str,
    expected: &InventoryEntry,
) -> Result<InventoryEntry, EngineError> {
    if !valid_id(&expected.id) {
        return Err(EngineError::InvalidPlan);
    }
    let body=super::client::request(token,&format!("https://www.googleapis.com/drive/v3/files/{}",expected.id),&[("fields","id,name,mimeType,parents,trashed,version,size,md5Checksum,driveId,capabilities(canDownload)")])?;
    super::inventory::decode_item(&body, &expected.parent_id)
}
/// No redirects or automatic decompression. Partial/error bodies are never media.
pub(crate) fn media(token: &str, id: &str) -> Result<Response, EngineError> {
    if !valid_id(id) {
        return Err(EngineError::InvalidPlan);
    }
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .user_agent("DriveSync/0.1")
        .build()
        .map_err(network)?;
    let response = client
        .get(format!("https://www.googleapis.com/drive/v3/files/{id}"))
        .bearer_auth(token)
        .query(&[("alt", "media")])
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .send()
        .map_err(network)?;
    validate_response(response)
}
fn network(_: impl std::fmt::Debug) -> EngineError {
    EngineError::Authentication(AuthError::Network)
}
fn validate_response(mut response: Response) -> Result<Response, EngineError> {
    let status = response.status().as_u16();
    if status == 200 {
        if response
            .headers()
            .get(reqwest::header::CONTENT_ENCODING)
            .is_some_and(|v| v != "identity")
        {
            return Err(EngineError::DownloadIntegrity);
        }
        return Ok(response);
    }
    let mut body = Vec::new();
    response
        .by_ref()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut body)
        .map_err(network)?;
    super::errors::response_status(status, &body)?;
    Err(EngineError::Authentication(AuthError::InvalidResponse))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn response(code: &str, headers: &str, body: &str) -> Response {
        use std::{io::Write, net::TcpListener, thread};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let message = format!(
            "HTTP/1.1 {code}\r\nContent-Length: {}\r\n{headers}Connection: close\r\n\r\n{body}",
            body.len()
        );
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            assert!(socket.read(&mut request).unwrap() > 0);
            socket.write_all(message.as_bytes()).unwrap();
        });
        let response = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
            .get(format!("http://{address}"))
            .send()
            .unwrap();
        worker.join().unwrap();
        response
    }
    #[test]
    fn accepts_only_complete_unencoded_media_and_preserves_http_errors() {
        assert!(validate_response(response("200 OK", "", "abc")).is_ok());
        for (code, headers, body, expected) in [
            (
                "206 Partial Content",
                "",
                "abc",
                EngineError::Authentication(AuthError::InvalidResponse),
            ),
            (
                "200 OK",
                "Content-Encoding: gzip\r\n",
                "abc",
                EngineError::DownloadIntegrity,
            ),
            (
                "404 Not Found",
                "",
                "",
                EngineError::RemoteFolderUnavailable,
            ),
            (
                "403 Forbidden",
                "",
                r#"{"error":{"errors":[{"reason":"rateLimitExceeded"}]}}"#,
                EngineError::RemoteRateLimited,
            ),
            (
                "301 Moved Permanently",
                "Location: https://example.invalid/\r\n",
                "",
                EngineError::Authentication(AuthError::InvalidResponse),
            ),
        ] {
            assert_eq!(
                validate_response(response(code, headers, body)).unwrap_err(),
                expected
            );
        }
    }
}
