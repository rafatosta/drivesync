//! Loopback consent context and provider-delegated exchange. No Google HTTP here.
use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Notify,
};
const LIFETIME: Duration = Duration::from_secs(300);
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Pending,
    Finishing,
    Committing,
    Cancelled,
    Expired,
}
pub(super) struct Attempt {
    _ownership: tokio::sync::OwnedRwLockReadGuard<()>,
    listener: tokio::sync::Mutex<Option<TcpListener>>,
    phase: Mutex<Phase>,
    cancelled: Notify,
    state: String,
    verifier: String,
    redirect: String,
    account_id: Option<AccountId>,
    deadline: Instant,
}
fn failure(kind: EngineErrorKind) -> EngineError {
    EngineError::new(kind)
}
fn random() -> EngineResult<String> {
    let mut bytes = [0; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|_| failure(EngineErrorKind::Internal))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn provider_error(error: GoogleApiFailure) -> EngineError {
    let kind = match error.kind {
        GoogleApiError::InvalidGrant
        | GoogleApiError::AuthenticationRequired
        | GoogleApiError::PermissionDenied
        | GoogleApiError::ReauthorizationRequired => EngineErrorKind::AuthorizationDenied,
        _ => EngineErrorKind::ProviderUnavailable,
    };
    let mut value = failure(kind);
    value.retryable = error.retryable;
    value
}
impl Engine {
    /// Prepares a loopback attempt and browser URL. Does not open a browser,
    /// exchange codes or persist accounts. The attempt expires after five minutes.
    pub async fn begin_google_sign_in(
        &self,
        request: BeginGoogleSignInRequest,
    ) -> EngineResult<BeginGoogleSignInResult> {
        let owner = self.clone();
        self.runtime()
            .spawn(async move {
                let ownership = owner.inner.ownership.gate.clone().read_owned().await;
                let config = owner
                    .inner
                    .oauth
                    .as_ref()
                    .ok_or_else(|| failure(EngineErrorKind::AuthorizationUnavailable))?;
                if let Some(id) = &request.account_id {
                    owner.inner.accounts.get(GetAccountRequest {
                        account_id: id.clone(),
                    })?;
                }
                let mut redirect = config
                    .redirect_uris
                    .iter()
                    .filter_map(|value| reqwest::Url::parse(value).ok())
                    .find(|url| {
                        url.scheme() == "http"
                            && matches!(
                                url.host_str(),
                                Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
                            )
                            && url.query().is_none()
                            && url.fragment().is_none()
                            && url.username().is_empty()
                            && url.password().is_none()
                    })
                    .ok_or_else(|| failure(EngineErrorKind::AuthorizationUnavailable))?;
                let address = if matches!(redirect.host_str(), Some("[::1]" | "::1")) {
                    "[::1]:0"
                } else {
                    "127.0.0.1:0"
                };
                let listener = TcpListener::bind(address)
                    .await
                    .map_err(|_| failure(EngineErrorKind::Internal))?;
                redirect
                    .set_port(Some(
                        listener
                            .local_addr()
                            .map_err(|_| failure(EngineErrorKind::Internal))?
                            .port(),
                    ))
                    .map_err(|_| failure(EngineErrorKind::InvalidConfiguration))?;
                let state = random()?;
                let verifier = random()?;
                let url = owner
                    .inner
                    .client
                    .authorization_request(
                        config,
                        GoogleAuthorizationRequest {
                            redirect_uri: redirect.to_string(),
                            state: state.clone(),
                            code_challenge: URL_SAFE_NO_PAD
                                .encode(Sha256::digest(verifier.as_bytes())),
                            code_challenge_method: "S256".into(),
                            scopes: config.scopes.clone(),
                            access_type: Some("offline".into()),
                            prompt: Some("consent select_account".into()),
                            ..Default::default()
                        },
                    )
                    .map_err(provider_error)?;
                let id = GoogleSignInId(random()?);
                let attempt = Arc::new(Attempt {
                    _ownership: ownership,
                    listener: tokio::sync::Mutex::new(Some(listener)),
                    phase: Mutex::new(Phase::Pending),
                    cancelled: Notify::new(),
                    state,
                    verifier,
                    redirect: redirect.to_string(),
                    account_id: request.account_id,
                    deadline: Instant::now() + LIFETIME,
                });
                owner
                    .inner
                    .attempts
                    .lock()
                    .map_err(|_| failure(EngineErrorKind::Internal))?
                    .insert(id.clone(), attempt.clone());
                let weak = Arc::downgrade(&owner.inner);
                let expiring = Arc::downgrade(&attempt);
                let expires = id.clone();
                owner.runtime().spawn(async move {
                    tokio::time::sleep(LIFETIME).await;
                    if let (Some(inner), Some(attempt)) = (weak.upgrade(), expiring.upgrade()) {
                        if attempt.expire() {
                            inner
                                .attempts
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .remove(&expires);
                        }
                    }
                });
                Ok(BeginGoogleSignInResult {
                    connection_id: id,
                    authorization_url: url.url,
                })
            })
            .await
            .map_err(|_| failure(EngineErrorKind::Internal))?
    }
    /// Consumes one attempt. Dropping this future does not promise cancellation;
    /// callers must explicitly cancel when closing their connection view.
    pub async fn finish_google_sign_in(
        &self,
        request: FinishGoogleSignInRequest,
    ) -> EngineResult<FinishGoogleSignInResult> {
        let owner = self.clone();
        self.runtime()
            .spawn(async move {
                let attempt = owner
                    .inner
                    .attempts
                    .lock()
                    .map_err(|_| failure(EngineErrorKind::Internal))?
                    .get(&request.connection_id)
                    .cloned()
                    .ok_or_else(|| failure(EngineErrorKind::AttemptNotFound))?;
                {
                    let mut phase = attempt
                        .phase
                        .lock()
                        .map_err(|_| failure(EngineErrorKind::Internal))?;
                    match *phase {
                        Phase::Pending => *phase = Phase::Finishing,
                        Phase::Cancelled => return Err(failure(EngineErrorKind::Cancelled)),
                        Phase::Expired => return Err(failure(EngineErrorKind::TimedOut)),
                        _ => return Err(failure(EngineErrorKind::AlreadyInProgress)),
                    }
                }
                let outcome = owner.finish_attempt(&attempt).await;
                owner
                    .inner
                    .attempts
                    .lock()
                    .map_err(|_| failure(EngineErrorKind::Internal))?
                    .remove(&request.connection_id);
                outcome
            })
            .await
            .map_err(|_| failure(EngineErrorKind::Internal))?
    }
    async fn finish_attempt(&self, attempt: &Attempt) -> EngineResult<FinishGoogleSignInResult> {
        let exchange = async {
            let listener = attempt
                .listener
                .lock()
                .await
                .take()
                .ok_or_else(|| failure(EngineErrorKind::AlreadyInProgress))?;
            let code = callback(&listener, &attempt.redirect, &attempt.state).await?;
            let config = self
                .inner
                .oauth
                .as_ref()
                .ok_or_else(|| failure(EngineErrorKind::AuthorizationUnavailable))?;
            self.inner
                .client
                .exchange_authorization_code(config, &code, &attempt.redirect, &attempt.verifier)
                .await
                .map_err(provider_error)
        };
        let notified = attempt.cancelled.notified();
        if *attempt
            .phase
            .lock()
            .map_err(|_| failure(EngineErrorKind::Internal))?
            == Phase::Cancelled
        {
            return Err(failure(EngineErrorKind::Cancelled));
        }
        let tokens = tokio::select! {
            biased;
            _ = notified => return Err(failure(if *attempt.phase.lock().map_err(|_| failure(EngineErrorKind::Internal))? == Phase::Expired { EngineErrorKind::TimedOut } else { EngineErrorKind::Cancelled })),
            _ = tokio::time::sleep_until(attempt.deadline.into()) => return Err(failure(EngineErrorKind::TimedOut)),
            result = exchange => result?,
        };
        {
            let mut phase = attempt
                .phase
                .lock()
                .map_err(|_| failure(EngineErrorKind::Internal))?;
            match *phase {
                Phase::Cancelled => return Err(failure(EngineErrorKind::Cancelled)),
                Phase::Expired => return Err(failure(EngineErrorKind::TimedOut)),
                _ => *phase = Phase::Committing,
            }
        }
        let authorization = ProviderAuthorizationHandle::google(tokens);
        if let Some(id) = &attempt.account_id {
            let result = self
                .inner
                .accounts
                .update_authorization(UpdateAuthorizationRequest {
                    account_id: id.clone(),
                    authorization,
                })
                .await?;
            Ok(FinishGoogleSignInResult {
                account: result.account,
                created: false,
            })
        } else {
            let result = self
                .inner
                .accounts
                .add(AddAccountRequest {
                    provider: crate::models::ProviderId("google".into()),
                    request_id: None,
                    authorization,
                })
                .await?;
            Ok(FinishGoogleSignInResult {
                account: result.account,
                created: result.created,
            })
        }
    }
    /// Cancels a pending attempt. Once secure persistence starts, returns false
    /// and lets the original finish call report its eventual durable result.
    pub fn cancel_google_sign_in(
        &self,
        request: CancelGoogleSignInRequest,
    ) -> CancelGoogleSignInResult {
        let mut entries = self
            .inner
            .attempts
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let Some(attempt) = entries.get(&request.connection_id) else {
            return CancelGoogleSignInResult { cancelled: false };
        };
        let mut phase = attempt.phase.lock().unwrap_or_else(|e| e.into_inner());
        if *phase == Phase::Committing {
            return CancelGoogleSignInResult { cancelled: false };
        }
        *phase = Phase::Cancelled;
        attempt.cancelled.notify_one();
        if let Ok(mut listener) = attempt.listener.try_lock() {
            listener.take();
        }
        drop(phase);
        entries.remove(&request.connection_id);
        CancelGoogleSignInResult { cancelled: true }
    }
}
/// A bounded, GET-only HTTP callback parser. Invalid traffic receives an error
/// but does not consume the OAuth attempt or reach a provider operation.
async fn callback(listener: &TcpListener, redirect: &str, state: &str) -> EngineResult<String> {
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|_| failure(EngineErrorKind::InvalidCallback))?;
        let read = async {
            let mut bytes = Vec::new();
            let mut buffer = [0; 1024];
            while bytes.len() <= 8192 && !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                let size = stream.read(&mut buffer).await.ok()?;
                if size == 0 {
                    return None;
                }
                bytes.extend_from_slice(&buffer[..size]);
            }
            if bytes.len() > 8192 {
                return None;
            }
            let text = std::str::from_utf8(&bytes).ok()?;
            let mut words = text.lines().next()?.split_whitespace();
            if words.next()? != "GET" {
                return None;
            }
            let target = words.next()?;
            if words.next()? != "HTTP/1.1"
                || words.next().is_some()
                || !target.starts_with('/')
                || target.starts_with("//")
            {
                return None;
            }
            let base = reqwest::Url::parse(redirect).ok()?;
            let url = base.join(target).ok()?;
            if url.origin() != base.origin()
                || url.path() != base.path()
                || url.fragment().is_some()
            {
                return None;
            }
            let values: Vec<_> = url.query_pairs().collect();
            let one = |key: &str| {
                let mut matches = values.iter().filter(|(k, _)| k == key);
                let value = matches.next()?.1.to_string();
                if matches.next().is_some() {
                    None
                } else {
                    Some(value)
                }
            };
            if one("state")?.as_str() != state {
                return None;
            }
            let codes = values.iter().filter(|(k, _)| k == "code").count();
            let errors = values.iter().filter(|(k, _)| k == "error").count();
            if codes == 1 && errors == 0 {
                let code = one("code")?;
                if code.is_empty() {
                    None
                } else {
                    Some(Ok(code))
                }
            } else if errors == 1 && codes == 0 {
                Some(Err(failure(EngineErrorKind::AuthorizationDenied)))
            } else {
                None
            }
        };
        let result = tokio::time::timeout(Duration::from_secs(5), read)
            .await
            .ok()
            .flatten();
        let response = if result.is_some() {
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 32\r\nConnection: close\r\n\r\nYou may now return to DriveSync."
        } else {
            "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        };
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            stream.write_all(response.as_bytes()),
        )
        .await;
        if let Some(result) = result {
            return result;
        }
    }
}

impl Attempt {
    /// Expire the same production state transition used by deterministic tests.
    pub(super) fn expire(&self) -> bool {
        let mut phase = self.phase.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(*phase, Phase::Committing | Phase::Cancelled) {
            return false;
        }
        *phase = Phase::Expired;
        self.cancelled.notify_one();
        if let Ok(mut listener) = self.listener.try_lock() {
            listener.take();
        }
        true
    }
    #[cfg(test)]
    pub(super) fn finishing(&self) -> bool {
        *self.phase.lock().unwrap() == Phase::Finishing
    }
}
