//! Minimal deterministic HTTP server that records requests (std only).

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use drivesync_engine::providers::google::{GoogleClientLimits, GoogleDriveClient, GoogleSession};

#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    /// Request target exactly as sent (path and raw query).
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap()
    }
    /// Decoded-insensitive raw query pairs, in sent order.
    pub fn query(&self) -> Vec<(String, String)> {
        let Some((_, q)) = self.target.split_once('?') else {
            return vec![];
        };
        q.split('&')
            .map(|p| {
                let (k, v) = p.split_once('=').unwrap_or((p, ""));
                (k.to_owned(), v.to_owned())
            })
            .collect()
    }
}

impl Recorded {
    /// Query pairs after percent-decoding, so tests do not depend on how a
    /// client chooses to escape reserved characters such as `,`.
    pub fn decoded_query(&self) -> Vec<(String, String)> {
        self.query()
            .into_iter()
            .map(|(k, v)| (percent_decode(&k), percent_decode(&v)))
            .collect()
    }
}

fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (
            bytes[i],
            bytes.get(i + 1).and_then(|b| hex(*b)),
            bytes.get(i + 2).and_then(|b| hex(*b)),
        ) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (b'+', _, _) => {
                out.push(b' ');
                i += 1;
            }
            (b, _, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// How the body is delimited on the wire.
pub enum Framing {
    /// Correct `Content-Length`.
    Exact,
    /// `Content-Length` announces this many bytes, but the connection closes
    /// after the (shorter) body: a truncated response.
    Declared(usize),
    /// No `Content-Length`: the body ends when the connection closes.
    UntilClose,
}

pub struct Reply {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
    /// Pause before answering, to exercise timeouts.
    pub delay: Duration,
    pub framing: Framing,
}

impl Reply {
    pub fn json(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: vec![("Content-Type", "application/json".into())],
            body: body.as_bytes().to_vec(),
            delay: Duration::ZERO,
            framing: Framing::Exact,
        }
    }
}

pub struct Server {
    pub base: String,
    pub requests: Arc<Mutex<Vec<Recorded>>>,
}

impl Server {
    /// Serves the given replies in order, one per connection.
    pub fn start(replies: Vec<Reply>) -> Self {
        Self::start_with(|_| replies)
    }

    /// Builds dynamic Location headers after choosing the isolated loopback port.
    pub fn start_with(make_replies: impl FnOnce(&str) -> Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let replies = make_replies(&base);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        thread::spawn(move || {
            for reply in replies {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };
                handle(stream, reply, &log);
            }
        });
        Self { base, requests }
    }

    pub fn client(&self) -> GoogleDriveClient {
        GoogleDriveClient::with_base_urls(
            format!("{}/drive/v3", self.base),
            format!("{}/upload/drive/v3", self.base),
        )
        .expect("loopback test endpoints are accepted")
    }

    pub fn client_with_limits(&self, limits: GoogleClientLimits) -> GoogleDriveClient {
        self.client()
            .with_limits(limits)
            .expect("limits are accepted")
    }

    pub fn recorded(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }
}

pub fn session() -> GoogleSession {
    GoogleSession::new("account-a", "tok-A-secret")
}

fn handle(mut stream: TcpStream, reply: Reply, log: &Mutex<Vec<Recorded>>) -> Option<()> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    let head_end = loop {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.lines();
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_owned();
    let target = first.next()?.to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect();
    let len = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = buf[head_end..].to_vec();
    while body.len() < len {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    // Record before responding: the client can observe completion as soon as
    // the response reaches it, including header-only 204 responses.
    log.lock().unwrap().push(Recorded {
        method,
        target,
        headers,
        body,
    });
    thread::sleep(reply.delay);
    let mut out = format!("HTTP/1.1 {} X\r\nConnection: close\r\n", reply.status);
    match reply.framing {
        Framing::Exact => out.push_str(&format!("Content-Length: {}\r\n", reply.body.len())),
        Framing::Declared(n) => out.push_str(&format!("Content-Length: {n}\r\n")),
        Framing::UntilClose => {}
    }
    for (k, v) in &reply.headers {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str("\r\n");
    let _ = stream.write_all(out.as_bytes());
    let _ = stream.write_all(&reply.body);
    let _ = stream.shutdown(Shutdown::Both);
    Some(())
}
