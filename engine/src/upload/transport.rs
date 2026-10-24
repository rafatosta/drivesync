//! Create-only Google transport. Session capabilities never enter logs/errors.
use super::Intent;
use crate::{AuthError, EngineError, InventoryEntry};
use reqwest::{
    blocking::{Client, Response},
    Method,
};
use serde_json::{json, Value};
use std::{io::Read, time::Duration};
const FIELDS: &str =
    "id,name,mimeType,parents,trashed,version,size,md5Checksum,driveId,capabilities(canDownload)";
fn invalid() -> EngineError {
    EngineError::Authentication(AuthError::InvalidResponse)
}
fn network(_: impl std::fmt::Debug) -> EngineError {
    EngineError::Authentication(AuthError::Network)
}
fn client() -> Result<Client, EngineError> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .user_agent("DriveSync/0.1")
        .build()
        .map_err(network)
}
fn body(response: Response) -> Result<Vec<u8>, EngineError> {
    let status = response.status().as_u16();
    let mut data = Vec::new();
    response
        .take(1024 * 1024 + 1)
        .read_to_end(&mut data)
        .map_err(network)?;
    if data.len() > 1024 * 1024 {
        return Err(invalid());
    }
    if status == 409 {
        return Err(EngineError::RemoteChanged);
    }
    crate::remote::upload_response_status(if status == 201 { 200 } else { status }, &data)?;
    Ok(data)
}
fn metadata(op: &Intent) -> Value {
    json!({"id":op.id,"name":op.path.file_name().and_then(|s| s.to_str()),"parents":[op.parent],"mimeType":if op.directory {"application/vnd.google-apps.folder"} else {"application/octet-stream"}})
}
/// Strict HTTPS origin/path validation prevents credential exfiltration via a
/// forged persisted session or a malicious Location header. No redirects follow.
pub(crate) fn valid_session(session: &str) -> bool {
    url::Url::parse(session).is_ok_and(|u| {
        u.scheme() == "https"
            && u.host_str() == Some("www.googleapis.com")
            && u.port_or_known_default() == Some(443)
            && u.username().is_empty()
            && u.password().is_none()
            && u.fragment().is_none()
            && u.path() == "/upload/drive/v3/files"
            && u.query_pairs()
                .any(|(k, v)| k == "upload_id" && !v.is_empty())
            && session.len() <= 8192
    })
}
pub(crate) fn generate(token: &str) -> Result<String, EngineError> {
    let v: Value = serde_json::from_slice(&body(
        client()?
            .get("https://www.googleapis.com/drive/v3/files/generateIds")
            .bearer_auth(token)
            .query(&[("count", "1"), ("space", "drive"), ("type", "files")])
            .send()
            .map_err(network)?,
    )?)
    .map_err(|_| invalid())?;
    v["ids"]
        .as_array()
        .filter(|a| a.len() == 1)
        .and_then(|a| a[0].as_str())
        .filter(|s| crate::configuration::valid_id(s))
        .map(String::from)
        .ok_or_else(invalid)
}
pub(crate) fn stat(token: &str, op: &Intent) -> Result<Option<InventoryEntry>, EngineError> {
    let response = client()?
        .get(format!(
            "https://www.googleapis.com/drive/v3/files/{}",
            op.id
        ))
        .bearer_auth(token)
        .query(&[("fields", FIELDS)])
        .send()
        .map_err(network)?;
    if response.status().as_u16() == 404 {
        return Ok(None);
    }
    crate::remote::decode_upload_item(&body(response)?, &op.parent).map(Some)
}
pub(crate) fn children(
    token: &str,
    parent: &str,
    name: &str,
) -> Result<Vec<InventoryEntry>, EngineError> {
    let mut all = Vec::new();
    let mut cursor = None::<String>;
    let mut seen = std::collections::HashSet::new();
    for _ in 0..10000 {
        let q = format!(
            "'{parent}' in parents and trashed=false and name='{}'",
            name.replace('\\', "\\\\").replace('\'', "\\'")
        );
        let mut params = vec![
            ("q", q),
            ("spaces", "drive".into()),
            ("pageSize", "1000".into()),
            (
                "fields",
                format!("nextPageToken,incompleteSearch,files({FIELDS})"),
            ),
        ];
        if let Some(c) = &cursor {
            params.push(("pageToken", c.clone()));
        }
        let data = body(
            client()?
                .get("https://www.googleapis.com/drive/v3/files")
                .bearer_auth(token)
                .query(&params)
                .send()
                .map_err(network)?,
        )?;
        let v: Value = serde_json::from_slice(&data).map_err(|_| invalid())?;
        if v["incompleteSearch"].as_bool() == Some(true) {
            return Err(invalid());
        }
        for item in v["files"].as_array().ok_or_else(invalid)? {
            all.push(crate::remote::decode_upload_item(
                &serde_json::to_vec(item).map_err(|_| invalid())?,
                parent,
            )?);
            if all.len() > 100000 {
                return Err(EngineError::InventoryLimit);
            }
        }
        match v.get("nextPageToken") {
            None | Some(Value::Null) => return Ok(all),
            Some(Value::String(c))
                if !c.is_empty() && c.len() <= 4096 && seen.insert(c.clone()) =>
            {
                cursor = Some(c.clone())
            }
            _ => return Err(invalid()),
        }
    }
    Err(EngineError::InventoryLimit)
}
pub(crate) fn create(token: &str, op: &Intent) -> Result<(), EngineError> {
    body(
        client()?
            .post("https://www.googleapis.com/drive/v3/files")
            .bearer_auth(token)
            .json(&metadata(op))
            .send()
            .map_err(network)?,
    )?;
    Ok(())
}
pub(crate) fn session(token: &str, op: &Intent) -> Result<String, EngineError> {
    let response = client()?
        .post("https://www.googleapis.com/upload/drive/v3/files")
        .bearer_auth(token)
        .query(&[("uploadType", "resumable"), ("fields", FIELDS)])
        .header("X-Upload-Content-Length", op.size)
        .header("X-Upload-Content-Type", "application/octet-stream")
        .json(&metadata(op))
        .send()
        .map_err(network)?;
    if response.status().as_u16() != 200 {
        body(response)?;
        return Err(invalid());
    }
    response
        .headers()
        .get("Location")
        .and_then(|v| v.to_str().ok())
        .filter(|s| valid_session(s))
        .map(String::from)
        .ok_or_else(invalid)
}
/// Server-confirmed next byte, completed upload, or expired session.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Position {
    Offset(u64),
    Complete,
    Expired,
}
pub(crate) fn send(
    token: &str,
    session: &str,
    total: u64,
    offset: Option<u64>,
    data: Vec<u8>,
) -> Result<Position, EngineError> {
    if !valid_session(session) {
        return Err(EngineError::StateStorage);
    }
    let length = data.len() as u64;
    let range = match offset {
        Some(start) if length > 0 && start.checked_add(length).is_some_and(|end| end <= total) => {
            format!("bytes {start}-{}/{total}", start + length - 1)
        }
        Some(0) if total == 0 && length == 0 => "bytes */0".into(),
        None if length == 0 => format!("bytes */{total}"),
        _ => return Err(EngineError::InvalidPlan),
    };
    let response = client()?
        .request(Method::PUT, session)
        .bearer_auth(token)
        .header("Content-Length", length)
        .header("Content-Range", range)
        .body(data)
        .send()
        .map_err(network)?;
    position(response, total)
}
fn position(response: Response, total: u64) -> Result<Position, EngineError> {
    match response.status().as_u16() {
        200 | 201 => Ok(Position::Complete),
        404 | 410 => Ok(Position::Expired),
        308 => {
            let next = response
                .headers()
                .get("Range")
                .map(|h| {
                    h.to_str()
                        .ok()
                        .and_then(|s| s.strip_prefix("bytes=0-"))
                        .and_then(|s| s.parse::<u64>().ok())
                        .and_then(|n| n.checked_add(1))
                })
                .unwrap_or(Some(0))
                .filter(|n| *n <= total)
                .ok_or_else(invalid)?;
            Ok(Position::Offset(next))
        }
        _ => {
            body(response)?;
            Err(invalid())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn response(code: &str, headers: &str) -> Response {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let message =
            format!("HTTP/1.1 {code}\r\nContent-Length: 0\r\n{headers}Connection: close\r\n\r\n");
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            assert!(socket.read(&mut request).unwrap() > 0);
            socket.write_all(message.as_bytes()).unwrap();
        });
        let response = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
            .get(format!("http://{address}/"))
            .send()
            .unwrap();
        worker.join().unwrap();
        response
    }
    #[test]
    fn interprets_only_verified_server_offsets_and_known_session_states() {
        assert_eq!(
            position(
                response("308 Resume Incomplete", "Range: bytes=0-7\r\n"),
                10
            )
            .unwrap(),
            Position::Offset(8)
        );
        assert_eq!(
            position(response("308 Resume Incomplete", ""), 10).unwrap(),
            Position::Offset(0)
        );
        for header in [
            "Range: bytes=1-7\r\n",
            "Range: bytes=0-99\r\n",
            "Range: forged\r\n",
        ] {
            assert!(position(response("308 Resume Incomplete", header), 10).is_err());
        }
        assert_eq!(
            position(response("201 Created", ""), 10).unwrap(),
            Position::Complete
        );
        assert_eq!(
            position(response("404 Not Found", ""), 10).unwrap(),
            Position::Expired
        );
        assert!(position(
            response("302 Found", "Location: https://evil.invalid/\r\n"),
            10
        )
        .is_err());
        assert_eq!(
            position(response("409 Conflict", ""), 10),
            Err(EngineError::RemoteChanged)
        );
    }
}
