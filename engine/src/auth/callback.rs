use super::AuthError;
use oauth2::CsrfToken;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use url::Url;

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub(super) fn check(&self) -> Result<(), AuthError> {
        if self.is_cancelled() {
            Err(AuthError::Cancelled)
        } else {
            Ok(())
        }
    }
}
pub(super) struct Listener {
    socket: TcpListener,
    authority: String,
}
impl Listener {
    pub fn bind() -> Result<Self, AuthError> {
        let socket = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .map_err(|_| AuthError::CallbackUnavailable)?;
        socket
            .set_nonblocking(true)
            .map_err(|_| AuthError::CallbackUnavailable)?;
        let authority = socket
            .local_addr()
            .map_err(|_| AuthError::CallbackUnavailable)?
            .to_string();
        Ok(Self { socket, authority })
    }
    pub fn redirect_uri(&self) -> String {
        format!("http://{}/oauth/callback", self.authority)
    }
    pub fn receive(
        self,
        state: &CsrfToken,
        cancel: &Cancellation,
        timeout: Duration,
    ) -> Result<String, AuthError> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            cancel.check()?;
            match self.socket.accept() {
                Ok((mut stream, peer)) => {
                    if !peer.ip().is_loopback() {
                        continue;
                    }
                    let result =
                        read_callback(&mut stream, &self.authority, state, cancel, deadline);
                    // The browser response contains no code, identity, or token. It does not
                    // claim success: token exchange and secure persistence still follow.
                    let (status, message) = match &result {
                        Ok(_) => (
                            "200 OK",
                            "Authorization received. Return to DriveSync to finish connecting.",
                        ),
                        Err(AuthError::Denied) => {
                            ("200 OK", "Authorization declined. Return to DriveSync.")
                        }
                        _ => (
                            "400 Bad Request",
                            "Invalid authorization response. Return to DriveSync.",
                        ),
                    };
                    let response = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n{message}", message.len());
                    let _ = stream.write_all(response.as_bytes());
                    match result {
                        Err(AuthError::InvalidCallback) => continue,
                        other => return other,
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50))
                }
                Err(_) => return Err(AuthError::CallbackUnavailable),
            }
        }
        Err(AuthError::TimedOut)
    }
}
fn read_callback(
    stream: &mut TcpStream,
    authority: &str,
    state: &CsrfToken,
    cancel: &Cancellation,
    deadline: Instant,
) -> Result<String, AuthError> {
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .map_err(|_| AuthError::InvalidCallback)?;
    stream
        .set_write_timeout(Some(Duration::from_millis(250)))
        .map_err(|_| AuthError::InvalidCallback)?;
    let mut reader = BufReader::new(stream.take(8193));
    let mut headers = String::new();
    loop {
        cancel.check()?;
        if Instant::now() >= deadline {
            return Err(AuthError::TimedOut);
        }
        let mut line = String::new();
        let size = reader
            .read_line(&mut line)
            .map_err(|_| AuthError::InvalidCallback)?;
        if size == 0 || headers.len() + size > 8192 {
            return Err(AuthError::InvalidCallback);
        }
        let end = line == "\r\n";
        headers.push_str(&line);
        if end {
            break;
        }
    }
    parse_callback(&headers, authority, state)
}
pub(super) fn parse_callback(
    headers: &str,
    authority: &str,
    state: &CsrfToken,
) -> Result<String, AuthError> {
    let mut lines = headers.split("\r\n");
    let request: Vec<_> = lines
        .next()
        .ok_or(AuthError::InvalidCallback)?
        .split(' ')
        .collect();
    if request.len() != 3
        || request[0] != "GET"
        || request[2] != "HTTP/1.1"
        || !request[1].starts_with("/oauth/callback?")
    {
        return Err(AuthError::InvalidCallback);
    }
    let mut hosts = Vec::new();
    for line in lines.take_while(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or(AuthError::InvalidCallback)?;
        if name.eq_ignore_ascii_case("host") {
            hosts.push(value.trim());
        }
    }
    if hosts != [authority] {
        return Err(AuthError::InvalidCallback);
    }
    let url = Url::parse(&format!("http://{authority}{}", request[1]))
        .map_err(|_| AuthError::InvalidCallback)?;
    if url.path() != "/oauth/callback" || url.fragment().is_some() {
        return Err(AuthError::InvalidCallback);
    }
    let mut params = std::collections::HashMap::new();
    for (key, value) in url.query_pairs() {
        if params
            .insert(key.into_owned(), value.into_owned())
            .is_some()
        {
            return Err(AuthError::InvalidCallback);
        }
    }
    let supplied = CsrfToken::new(params.remove("state").ok_or(AuthError::InvalidCallback)?);
    // oauth2's timing-resistant-secret-traits hashes before comparison.
    if supplied != *state {
        return Err(AuthError::InvalidCallback);
    }
    if let Some(error) = params.remove("error") {
        if params.contains_key("code") {
            return Err(AuthError::InvalidCallback);
        }
        return Err(match error.as_str() {
            "access_denied" => AuthError::Denied,
            "temporarily_unavailable" | "server_error" => AuthError::ProviderUnavailable,
            _ => AuthError::InvalidResponse,
        });
    }
    params
        .remove("code")
        .filter(|s| !s.is_empty())
        .ok_or(AuthError::InvalidCallback)
}
