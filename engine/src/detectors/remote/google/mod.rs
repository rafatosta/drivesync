//! Google interpretation, scoped traversal and crash-safe page replay through Provider.
use super::*;
mod api;
mod metadata;
mod timestamp;
use crate::{
    models::*,
    providers::google::{GoogleChangeList, GoogleFile, GoogleFileList},
};
use metadata::{adapt, classify, filtered, is_folder, location};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
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
    started_at: Option<i64>,
    cursor: Option<String>,
    scan: VecDeque<Scan>,
    scanned: BTreeSet<String>,
    known: BTreeMap<String, GoogleFile>,
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
            version: 1,
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
    fn event(
        &self,
        state: &State,
        index: usize,
        raw: Option<&GoogleFile>,
        previous: Option<&GoogleFile>,
        kind: FileEventKind,
        reason: Option<ObservationUncertainty>,
    ) -> RemoteResult<File> {
        let id = format!("remote:{}:{}:{index}", state.epoch, state.sequence);
        let reference = raw.and_then(|f| f.id.as_ref()).map(|id| RemoteFileRef {
            account_id: self.scope.account_id.clone(),
            file_id: RemoteFileId(id.0.clone()),
        });
        let data = raw
            .map(|raw| adapt(raw, &self.scope.account_id))
            .transpose()?
            .map(FileData::Remote);
        let event = File {
            id: crate::models::FileId(id),
            sync_id: self.scope.sync_id.clone(),
            event: FileEvent {
                remote: None,
                kind,
                uncertainty: reason,
                previous: previous.and_then(|file| location(file, &self.scope.account_id)),
                current: raw.and_then(|file| location(file, &self.scope.account_id)),
            },
            observed_at: now(),
            coverage: if kind == FileEventKind::ScopeUnavailable {
                ObservationCoverage::Unavailable
            } else {
                ObservationCoverage::Partial
            },
            scope: FileScope::Remote(reference.unwrap_or_else(|| RemoteFileRef {
                account_id: self.scope.account_id.clone(),
                file_id: RemoteFileId(self.scope.remote_root_id.clone()),
            })),
            data,
        };
        event.validate().map_err(|_| incomplete())?;
        Ok(event)
    }
    fn validate_checkpoint(&self) -> RemoteResult<()> {
        if let Some(bytes) = self.backend.load()? {
            let state: State = serde_json::from_slice(&bytes)
                .map_err(|_| RemoteFailure::new(RemoteError::CheckpointUnavailable))?;
            if state.version != 1
                || state.epoch.len() != 64
                || !state.epoch.bytes().all(|byte| byte.is_ascii_hexdigit())
                || state.progress.known.len() > 100_000
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
        let event = self.event(
            &state,
            0,
            None,
            None,
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
        if state.version != 1 {
            return Err(RemoteFailure::new(RemoteError::CheckpointUnavailable));
        }
        let mut emitted = self.flush(&mut old, &mut state, cancel)?;
        let mut observed = 0;
        progress(observed, emitted);
        if mode == RemotePollMode::Recovery && state.progress.cursor.is_some() {
            state.sequence += 1;
            let event = self.event(
                &state,
                0,
                None,
                None,
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
                next.started_at = Some(now().unix_millis);
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
                files.push(self.event(
                    &state,
                    0,
                    Some(&root),
                    None,
                    FileEventKind::RecheckRequired,
                    Some(ObservationUncertainty::CaptureStarted),
                )?);
                next.known.insert(self.scope.remote_root_id.clone(), root);
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
                    let Some(id) = raw
                        .id
                        .as_ref()
                        .filter(|id| valid(&id.0))
                        .map(|id| id.0.clone())
                    else {
                        files.push(self.event(
                            &state,
                            files.len(),
                            None,
                            None,
                            FileEventKind::RecheckRequired,
                            Some(ObservationUncertainty::ObservationUnstable),
                        )?);
                        continue;
                    };
                    if !raw
                        .parents
                        .as_ref()
                        .is_some_and(|parents| parents.iter().any(|p| p.0 == scan.folder))
                        || raw.trashed != Some(false)
                    {
                        return Err(incomplete());
                    }
                    if next.known.get(&id) != Some(&raw) {
                        let previous = next.known.get(&id);
                        let (kind, reason) = classify(previous, &raw, true, next.started_at);
                        files.push(self.event(
                            &state,
                            files.len(),
                            Some(&raw),
                            previous,
                            kind,
                            reason,
                        )?);
                    }
                    if is_folder(&raw) && next.scanned.insert(id.clone()) {
                        next.scan.push_back(Scan {
                            folder: id.clone(),
                            token: None,
                        });
                    }
                    next.known.insert(id, raw);
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
                    let Some(id) = change
                        .file_id
                        .as_ref()
                        .filter(|id| valid(&id.0))
                        .map(|id| id.0.clone())
                    else {
                        files.push(self.event(
                            &state,
                            files.len(),
                            None,
                            None,
                            FileEventKind::RecheckRequired,
                            Some(ObservationUncertainty::ObservationUnstable),
                        )?);
                        continue;
                    };
                    if change.removed != Some(false) || change.file.is_none() {
                        // Removed is also loss of access. Unknown removals cannot be scoped safely.
                        files.push(self.event(
                            &state,
                            files.len(),
                            None,
                            None,
                            FileEventKind::RecheckRequired,
                            Some(ObservationUncertainty::ObservationUnstable),
                        )?);
                        continue;
                    }
                    let raw = filtered(change.file.as_ref().unwrap());
                    if raw.id.as_ref().map(|f| &f.0) != Some(&id) {
                        files.push(self.event(
                            &state,
                            files.len(),
                            None,
                            None,
                            FileEventKind::RecheckRequired,
                            Some(ObservationUncertainty::ObservationUnstable),
                        )?);
                        continue;
                    }
                    if id == self.scope.remote_root_id && raw.trashed != Some(false) {
                        return Err(RemoteFailure::new(RemoteError::PermissionDenied));
                    }
                    let previous = next.known.get(&id).cloned();
                    let inside = match self.belongs(&raw, &next.known, cancel) {
                        Ok(inside) => inside,
                        Err(error)
                            if matches!(
                                error.kind,
                                RemoteError::PageIncomplete | RemoteError::PermissionDenied
                            ) =>
                        {
                            files.push(self.event(
                                &state,
                                files.len(),
                                None,
                                None,
                                FileEventKind::RecheckRequired,
                                Some(ObservationUncertainty::CoverageIncomplete),
                            )?);
                            continue;
                        }
                        Err(error) => return Err(error),
                    };
                    let was_inside = previous
                        .as_ref()
                        .map(|file| self.belongs(file, &next.known, cancel))
                        .transpose()?
                        .unwrap_or(false);
                    if !inside && !was_inside {
                        continue;
                    }
                    if previous.as_ref() != Some(&raw) {
                        let (kind, reason) =
                            classify(previous.as_ref(), &raw, false, next.started_at);
                        files.push(self.event(
                            &state,
                            files.len(),
                            Some(&raw),
                            previous.as_ref(),
                            kind,
                            reason,
                        )?);
                        if is_folder(&raw) && inside && !was_inside {
                            next.scanned.clear();
                            next.scanned.insert(self.scope.remote_root_id.clone());
                            next.scanned.insert(id.clone());
                            next.scan.push_back(Scan {
                                folder: id.clone(),
                                token: None,
                            });
                        }
                        next.known.insert(id, raw);
                    }
                }
                next.cursor = Some(token);
            }
            if next.known.len() > 100_000 || next.scan.len() > 100_000 {
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
    fn belongs(
        &self,
        file: &GoogleFile,
        known: &BTreeMap<String, GoogleFile>,
        cancel: &Cancellation,
    ) -> RemoteResult<bool> {
        let mut current = file.clone();
        let mut visited = BTreeSet::new();
        for _ in 0..64 {
            cancel.check()?;
            let id = current.id.as_ref().ok_or_else(incomplete)?;
            if id.0 == self.scope.remote_root_id {
                return Ok(true);
            }
            if !visited.insert(id.0.clone()) {
                return Err(incomplete());
            }
            let Some(parents) = &current.parents else {
                return Err(incomplete());
            };
            if parents.is_empty() {
                return Ok(false);
            }
            if parents.iter().any(|id| id.0 == self.scope.remote_root_id) {
                return Ok(true);
            }
            if parents.len() != 1 {
                return Err(incomplete());
            }
            let parent = &parents[0].0;
            if let Some(file) = known.get(parent) {
                current = file.clone();
            } else {
                let Response::File(file) = self.api.call(Request::Get(parent.clone()), cancel)?
                else {
                    return Err(incomplete());
                };
                if file.id.as_ref().map(|id| &id.0) != Some(parent) {
                    return Err(incomplete());
                }
                current = *file;
            }
        }
        Err(incomplete())
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
