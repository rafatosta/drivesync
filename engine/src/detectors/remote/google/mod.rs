//! Google evidence collection, initial traversal and crash-safe page replay through Provider.
use super::*;
mod api;
mod metadata;
use crate::{
    models::*,
    providers::google::{GoogleChangeList, GoogleFile, GoogleFileList},
};
use metadata::{adapt, filtered, is_folder, location};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::Arc,
};

pub(super) enum Request {
    Start,
    Get(String),
    List {
        parent: String,
        token: Option<String>,
    },
    Changes(String),
}
pub(super) enum Response {
    Start(String),
    File(Box<GoogleFile>),
    List(GoogleFileList),
    Changes(GoogleChangeList),
}
/// Provider seam: typed requests only; no HTTP endpoints are built by the detector.
pub(super) trait Api: Send + Sync {
    fn call(&self, request: Request, cancellation: &Cancellation) -> RemoteResult<Response>;
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Scan {
    folder: String,
    token: Option<String>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Progress {
    cursor: Option<String>,
    scan: VecDeque<Scan>,
    scanned: BTreeSet<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    files: Vec<File>,
    next: Progress,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    version: u32,
    epoch: String,
    sequence: u64,
    progress: Progress,
    pending: Option<Pending>,
    #[serde(default)]
    warning: Option<RemoteError>,
}
impl State {
    fn fresh() -> RemoteResult<Self> {
        use std::io::Read;
        let mut seed = [0u8; 24];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut seed))
            .map_err(|_| RemoteFailure::new(RemoteError::Internal))?;
        Ok(Self {
            version: 2,
            epoch: format!("{:x}", Sha256::digest(seed)),
            sequence: 0,
            progress: Progress::default(),
            pending: None,
            warning: None,
        })
    }
}
pub(super) struct GoogleRemoteDetector {
    pub(super) scope: RemoteDetectorScope,
    pub(super) api: Arc<dyn Api>,
    pub(super) backend: Arc<dyn RemoteBackend>,
}
fn incomplete() -> RemoteFailure {
    RemoteFailure::new(RemoteError::PageIncomplete)
}
fn valid(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control)
}
fn fingerprint(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))[..12].to_owned()
}
impl GoogleRemoteDetector {
    fn save(&self, old: &mut Option<Vec<u8>>, state: &State) -> RemoteResult<()> {
        let value = serde_json::to_vec(state).map_err(|_| incomplete())?;
        self.backend.save(old.as_deref(), &value)?;
        *old = Some(value);
        Ok(())
    }
    fn flush(
        &self,
        old: &mut Option<Vec<u8>>,
        state: &mut State,
        cancel: &Cancellation,
    ) -> RemoteResult<u64> {
        let Some(pending) = &state.pending else {
            return Ok(0);
        };
        let mut count = 0;
        for file in &pending.files {
            cancel.check()?;
            file.validate().map_err(|_| incomplete())?;
            if file.sync_id != self.scope.sync_id {
                return Err(incomplete());
            }
            if self.backend.emit(file)? {
                count += 1;
            }
        }
        state.progress = state.pending.take().unwrap().next;
        self.save(old, state)?;
        Ok(count)
    }
    fn stage(
        &self,
        old: &mut Option<Vec<u8>>,
        state: &mut State,
        files: Vec<File>,
        next: Progress,
        cancel: &Cancellation,
    ) -> RemoteResult<u64> {
        state.pending = Some(Pending { files, next });
        self.save(old, state)?;
        self.flush(old, state, cancel)
    }
    /// Reports collection failures, independently of resource change interpretation.
    fn warning_event(
        &self,
        state: &State,
        kind: FileEventKind,
        reason: Option<ObservationUncertainty>,
    ) -> RemoteResult<File> {
        let event = File {
            id: crate::models::FileId(format!("remote:{}:{}:0", state.epoch, state.sequence)),
            sync_id: self.scope.sync_id.clone(),
            event: FileEvent {
                remote: None,
                kind,
                uncertainty: reason,
                previous: None,
                current: None,
            },
            observed_at: now(),
            coverage: if kind == FileEventKind::ScopeUnavailable {
                ObservationCoverage::Unavailable
            } else {
                ObservationCoverage::Partial
            },
            scope: FileScope::Remote(RemoteFileRef {
                account_id: self.scope.account_id.clone(),
                file_id: RemoteFileId(self.scope.remote_root_id.clone()),
            }),
            data: None,
        };
        event.validate().map_err(|_| incomplete())?;
        Ok(event)
    }
    /// Maps one provider observation without consulting history or interpreting an action.
    fn observation(
        &self,
        state: &State,
        index: usize,
        id: &str,
        raw: Option<&GoogleFile>,
        evidence: RemoteObservation,
    ) -> RemoteResult<File> {
        let file = File {
            id: crate::models::FileId(format!("remote:{}:{}:{index}", state.epoch, state.sequence)),
            sync_id: self.scope.sync_id.clone(),
            event: FileEvent {
                kind: FileEventKind::Observed,
                remote: Some(evidence),
                uncertainty: None,
                previous: None,
                current: raw.and_then(|file| location(file, &self.scope.account_id)),
            },
            observed_at: now(),
            coverage: ObservationCoverage::Partial,
            scope: FileScope::Remote(RemoteFileRef {
                account_id: self.scope.account_id.clone(),
                file_id: RemoteFileId(id.to_owned()),
            }),
            data: raw
                .map(|file| adapt(file, &self.scope.account_id))
                .transpose()?
                .map(FileData::Remote),
        };
        file.validate().map_err(|_| incomplete())?;
        Ok(file)
    }
    fn validate_checkpoint(&self) -> RemoteResult<()> {
        if let Some(bytes) = self.backend.load()? {
            let state: State = serde_json::from_slice(&bytes)
                .map_err(|_| RemoteFailure::new(RemoteError::CheckpointUnavailable))?;
            if state.version != 2
                || state.epoch.len() != 64
                || !state.epoch.bytes().all(|byte| byte.is_ascii_hexdigit())
                || state
                    .progress
                    .cursor
                    .as_deref()
                    .is_some_and(|token| !valid(token))
            {
                return Err(RemoteFailure::new(RemoteError::CheckpointUnavailable));
            }
            if let Some(pending) = state.pending {
                for file in pending.files {
                    file.validate()
                        .map_err(|_| RemoteFailure::new(RemoteError::CheckpointUnavailable))?;
                    if file.sync_id != self.scope.sync_id {
                        return Err(RemoteFailure::new(RemoteError::CheckpointUnavailable));
                    }
                }
            }
        }
        Ok(())
    }
    #[cfg(test)]
    pub fn poll(
        &self,
        mode: RemotePollMode,
        budget: u32,
        cancel: &Cancellation,
    ) -> RemoteResult<RemotePollResult> {
        self.observe_round(mode, budget, cancel, &|_, _| {})
    }
    fn observe_round(
        &self,
        mode: RemotePollMode,
        budget: u32,
        cancel: &Cancellation,
        progress: &dyn Fn(u64, u64),
    ) -> RemoteResult<RemotePollResult> {
        let counters = std::cell::Cell::new((0, 0));
        let report = |observed, emitted| {
            counters.set((observed, emitted));
            progress(observed, emitted);
        };
        let result = self.poll_round(mode, budget, cancel, &report);
        if let Err(error) = &result {
            if matches!(
                error.kind,
                RemoteError::PermissionDenied
                    | RemoteError::AuthorizationRequired
                    | RemoteError::CursorExpired
                    | RemoteError::PageIncomplete
            ) {
                let warnings = self.record_failure(error.kind, cancel)?;
                let (observed, emitted) = counters.get();
                progress(observed, emitted + warnings);
            }
        }
        result
    }
    fn record_failure(&self, error: RemoteError, cancel: &Cancellation) -> RemoteResult<u64> {
        cancel.check()?;
        let mut old = self.backend.load()?;
        let mut state = match &old {
            Some(bytes) => serde_json::from_slice::<State>(bytes)
                .map_err(|_| RemoteFailure::new(RemoteError::CheckpointUnavailable))?,
            None => State::fresh()?,
        };
        if state.pending.is_some() || state.warning == Some(error) {
            return Ok(0);
        }
        state.sequence += 1;
        state.warning = Some(error);
        let inaccessible = matches!(
            error,
            RemoteError::PermissionDenied | RemoteError::AuthorizationRequired
        );
        let event = self.warning_event(
            &state,
            if inaccessible {
                FileEventKind::ScopeUnavailable
            } else {
                FileEventKind::RecheckRequired
            },
            if inaccessible {
                None
            } else {
                Some(ObservationUncertainty::CoverageIncomplete)
            },
        )?;
        let next = state.progress.clone();
        self.stage(&mut old, &mut state, vec![event], next, cancel)
    }
    /// Page preparation is durable before first emission; replay cannot regenerate IDs/times.
    fn poll_round(
        &self,
        mode: RemotePollMode,
        budget: u32,
        cancel: &Cancellation,
        progress: &dyn Fn(u64, u64),
    ) -> RemoteResult<RemotePollResult> {
        cancel.check()?;
        let mut old = self.backend.load()?;
        let mut state = match &old {
            Some(bytes) => serde_json::from_slice::<State>(bytes)
                .map_err(|_| RemoteFailure::new(RemoteError::CheckpointUnavailable))?,
            None => State::fresh()?,
        };
        if state.version != 2 {
            return Err(RemoteFailure::new(RemoteError::CheckpointUnavailable));
        }
        let mut emitted = self.flush(&mut old, &mut state, cancel)?;
        let mut observed = 0;
        progress(observed, emitted);
        if mode == RemotePollMode::Recovery && state.progress.cursor.is_some() {
            state.sequence += 1;
            let event = self.warning_event(
                &state,
                FileEventKind::RecheckRequired,
                Some(ObservationUncertainty::CoverageIncomplete),
            )?;
            // Recovery never assumes that unseen entries were deleted.
            emitted += self.stage(
                &mut old,
                &mut state,
                vec![event],
                Progress::default(),
                cancel,
            )?;
        }
        let mut pages = 0;
        let mut completed = false;
        while pages < budget {
            cancel.check()?;
            let mut next = state.progress.clone();
            let mut files = Vec::new();
            state.sequence += 1;
            if next.cursor.is_none() {
                let Response::Start(cursor) = self.api.call(Request::Start, cancel)? else {
                    return Err(incomplete());
                };
                if !valid(&cursor) {
                    return Err(incomplete());
                }
                let Response::File(root) = self
                    .api
                    .call(Request::Get(self.scope.remote_root_id.clone()), cancel)?
                else {
                    return Err(incomplete());
                };
                if root.id.as_ref().map(|id| id.0.as_str()) != Some(&self.scope.remote_root_id)
                    || root.mime_type.as_ref().map(|mime| mime.0.as_str())
                        != Some("application/vnd.google-apps.folder")
                    || root.trashed != Some(false)
                {
                    return Err(RemoteFailure::new(RemoteError::PermissionDenied));
                }
                let root = filtered(&root);
                files.push(self.observation(
                    &state,
                    0,
                    &self.scope.remote_root_id,
                    Some(&root),
                    capture(),
                )?);
                next.cursor = Some(cursor);
                next.scanned.insert(self.scope.remote_root_id.clone());
                next.scan.push_back(Scan {
                    folder: self.scope.remote_root_id.clone(),
                    token: None,
                });
                observed += 1;
                progress(observed, emitted);
            } else if let Some(scan) = next.scan.front().cloned() {
                let Response::List(page) = self.api.call(
                    Request::List {
                        parent: scan.folder.clone(),
                        token: scan.token.clone(),
                    },
                    cancel,
                )?
                else {
                    return Err(incomplete());
                };
                if page.incomplete_search == Some(true)
                    || page.files.len() > 200
                    || page
                        .next_page_token
                        .as_ref()
                        .is_some_and(|token| !valid(token) || Some(token) == scan.token.as_ref())
                {
                    return Err(incomplete());
                }
                next.scan.pop_front();
                if let Some(token) = page.next_page_token {
                    next.scan.push_front(Scan {
                        folder: scan.folder.clone(),
                        token: Some(token),
                    });
                }
                for raw in page.files {
                    observed += 1;
                    progress(observed, emitted);
                    let raw = filtered(&raw);
                    let id = raw
                        .id
                        .as_ref()
                        .filter(|id| valid(&id.0))
                        .map(|id| id.0.clone())
                        .ok_or_else(incomplete)?;
                    files.push(self.observation(
                        &state,
                        files.len(),
                        &id,
                        Some(&raw),
                        capture(),
                    )?);
                    if is_folder(&raw) && next.scanned.insert(id.clone()) {
                        next.scan.push_back(Scan {
                            folder: id.clone(),
                            token: None,
                        });
                    }
                }
                if next.scan.is_empty() {
                    next.scanned.clear();
                }
            } else {
                let cursor = next.cursor.clone().unwrap();
                let Response::Changes(page) =
                    self.api.call(Request::Changes(cursor.clone()), cancel)?
                else {
                    return Err(incomplete());
                };
                if page.changes.len() > 200
                    || page.next_page_token.is_some() == page.new_start_page_token.is_some()
                {
                    return Err(incomplete());
                }
                completed = page.next_page_token.is_none();
                let token = page
                    .next_page_token
                    .or(page.new_start_page_token)
                    .ok_or_else(incomplete)?;
                if !valid(&token) || (!completed && token == cursor) {
                    return Err(incomplete());
                }
                for change in page.changes {
                    cancel.check()?;
                    observed += 1;
                    progress(observed, emitted);
                    let id = change
                        .file_id
                        .as_ref()
                        .filter(|id| valid(&id.0))
                        .map(|id| id.0.clone())
                        .ok_or_else(incomplete)?;
                    let raw = change.file.as_ref().map(filtered);
                    if raw.as_ref().is_some_and(|file| {
                        file.id.as_ref().map(|id| id.0.as_str()) != Some(id.as_str())
                    }) {
                        return Err(incomplete());
                    }
                    files.push(self.observation(
                        &state,
                        files.len(),
                        &id,
                        raw.as_ref(),
                        RemoteObservation {
                            source: RemoteObservationSource::Changes,
                            removed: change.removed,
                            time: change.time,
                            change_type: change.change_type,
                            drive_id: change.drive_id.map(|id| id.0),
                        },
                    )?);
                }
                next.cursor = Some(token);
            }
            if next.scanned.len() > 100_000 || next.scan.len() > 100_000 {
                return Err(incomplete());
            }
            state.warning = None;
            emitted += self.stage(&mut old, &mut state, files, next, cancel)?;
            progress(observed, emitted);
            pages += 1;
            if completed {
                break;
            }
        }
        let has_more = !completed || !state.progress.scan.is_empty();
        Ok(RemotePollResult {
            coverage: if has_more {
                RemoteCoverage::Partial
            } else {
                RemoteCoverage::Complete
            },
            observed,
            emitted,
            has_more,
            checkpoint_confirmed: state.progress.cursor.as_deref().map(fingerprint),
        })
    }
}
fn capture() -> RemoteObservation {
    RemoteObservation {
        source: RemoteObservationSource::Capture,
        removed: None,
        time: None,
        change_type: None,
        drive_id: None,
    }
}
#[cfg(test)]
mod tests;

/// Creates Google's implementation after the common facade has selected this provider.
pub(super) fn create(
    connector: crate::Engine,
    session: RemoteSessionContext,
    scope: RemoteDetectorScope,
    backend: Arc<dyn RemoteBackend>,
) -> RemoteResult<Box<dyn RemoteObserver>> {
    if session.provider.0 != "google" || session.account_id != scope.account_id {
        return Err(RemoteFailure::new(RemoteError::InvalidScope));
    }
    Ok(Box::new(GoogleRemoteDetector {
        scope,
        api: Arc::new(api::Access::new(connector, session)),
        backend,
    }))
}
impl RemoteObserver for GoogleRemoteDetector {
    fn validate_progress(&self) -> RemoteResult<()> {
        self.validate_checkpoint()
    }
    fn poll_once(
        &self,
        request: RemotePollRequest,
        cancel: &Cancellation,
        progress: &dyn Fn(u64, u64),
    ) -> RemoteResult<RemotePollResult> {
        let budget = request
            .page_budget
            .ok_or_else(|| RemoteFailure::new(RemoteError::InvalidScope))?;
        self.observe_round(request.mode, budget, cancel, progress)
    }
}
