//! Google read-only observation with incremental file evidence and scoped recovery.
mod normalize;
use super::super::progress::ScanProgress;
use super::{interfaces::*, models::*};
use crate::detectors::observation::{ObservationCoverage, RemoteObservation};
use crate::{models::*, providers::google::*};
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
const FIELDS: &str =
    "id,name,parents,mimeType,size,sha256Checksum,md5Checksum,headRevisionId,version,modifiedTime,trashed,capabilities";
const FOLDER: &str = "application/vnd.google-apps.folder";
const AUDIT_INTERVAL: Duration = Duration::from_secs(30);
const MAX_ITEMS: usize = 100_000;
/// Opaque authorized adapter bound to the account that acquired this session through Accounts.
pub(crate) struct GoogleObserverSession {
    pub account_id: AccountId,
    pub provider_id: ProviderId,
    pub session: GoogleSession,
}
impl AuthorizedSession for GoogleObserverSession {
    fn account_id(&self) -> &AccountId {
        &self.account_id
    }
    fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
/// Factory registration for the Google specialist; no implicit fallback to this provider.
pub(crate) struct GoogleObserverProvider {
    pub client: GoogleDriveClient,
}
impl RemoteObserverProvider for GoogleObserverProvider {
    fn create(
        &self,
        config: RemoteObserverConfig,
    ) -> Result<Box<dyn RemoteObserver>, RemoteObserverError> {
        if config.provider_id.0 != "google" {
            return Err(RemoteObserverError::UnsupportedProvider);
        }
        Ok(Box::new(GoogleObserver {
            config,
            client: self.client.clone(),
            state: Mutex::new(State::default()),
        }))
    }
}
struct GoogleObserver {
    config: RemoteObserverConfig,
    client: GoogleDriveClient,
    state: Mutex<State>,
}
struct State {
    item_limit: usize,
    last_full: Option<Instant>,
    known: BTreeMap<RemoteId, RemoteObservation>,
    cursor: Option<String>,
    capture: Option<Capture>,
    feed_token: Option<String>,
    feed_files: Option<BTreeMap<RemoteId, GoogleFile>>,
    feed_seen: BTreeSet<String>,
    has_snapshot: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            item_limit: MAX_ITEMS,
            last_full: None,
            has_snapshot: false,
            known: BTreeMap::new(),
            capture: None,
            cursor: None,
            feed_token: None,
            feed_files: Some(BTreeMap::new()),
            feed_seen: BTreeSet::new(),
        }
    }
}
struct Capture {
    checkpoint: String,
    pages_read: usize,
    folders: VecDeque<RemoteId>,
    page: Option<String>,
    pages: BTreeSet<(RemoteId, String)>,
    visited: BTreeSet<RemoteId>,
    items: BTreeMap<RemoteId, RemoteObservation>,
}
impl RemoteObserver for GoogleObserver {
    fn observe<'a>(
        &'a self,
        request: RemoteObserveRequest,
        session: &'a dyn AuthorizedSession,
        cancel: &'a CancellationToken,
    ) -> RemoteObserverFuture<'a> {
        Box::pin(async move {
            if request.scope.account_id != self.config.account_id
                || request.scope.provider_id != self.config.provider_id
                || request.scope.sync_root_id != self.config.sync_root_id
                || request.scope.remote_root_id != self.config.remote_root_id
                || session.account_id() != &self.config.account_id
                || session.provider_id() != &self.config.provider_id
            {
                return Err(RemoteObserverError::AuthorizationRequired);
            }
            let session = session
                .as_any()
                .downcast_ref::<GoogleObserverSession>()
                .ok_or(RemoteObserverError::AuthorizationRequired)?;
            if session.session.account_id != self.config.account_id.0 {
                return Err(RemoteObserverError::AuthorizationRequired);
            }
            let work = self.round(request, &session.session);
            tokio::pin!(work);
            tokio::select! {biased;_=cancelled(cancel)=>Err(RemoteObserverError::Cancelled),result=&mut work=>result}
        })
    }
}
pub(crate) async fn cancelled(token: &CancellationToken) {
    while !token.is_cancelled() {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
impl GoogleObserver {
    async fn root(&self, session: &GoogleSession) -> Result<(), RemoteObserverError> {
        let file = self
            .client
            .files_get(
                session,
                &FileId(self.config.remote_root_id.0.clone()),
                FileGetOptions {
                    fields: Some(FieldMask(FIELDS.into())),
                    supports_all_drives: Some(true),
                    ..Default::default()
                },
            )
            .await
            .map_err(failure)?;
        if file.id.as_ref().map(|id| id.0.as_str()) != Some(self.config.remote_root_id.0.as_str())
            || file.mime_type.as_ref().map(|m| m.0.as_str()) != Some(FOLDER)
            || file.trashed != Some(false)
        {
            return Err(RemoteObserverError::IncompleteCoverage);
        }
        if !file
            .capabilities
            .as_ref()
            .is_some_and(|c| c.can_list_children == Some(true))
        {
            return Err(RemoteObserverError::PermissionDenied);
        }
        Ok(())
    }
    async fn begin(&self, session: &GoogleSession) -> Result<Capture, RemoteObserverError> {
        let _sample = crate::performance::Sample::begin("google_full_capture_start");
        // The checkpoint precedes listing so changes during capture cannot fall into a gap.
        let checkpoint = self
            .client
            .changes_get_start_page_token(
                session,
                ChangeScopeOptions {
                    supports_all_drives: Some(true),
                    ..Default::default()
                },
            )
            .await
            .map_err(failure)?
            .start_page_token;
        if checkpoint.is_empty() {
            return Err(RemoteObserverError::InvalidCursor);
        }
        self.root(session).await?;
        Ok(Capture {
            checkpoint,
            pages_read: 0,
            folders: VecDeque::from([self.config.remote_root_id.clone()]),
            page: None,
            pages: BTreeSet::new(),
            visited: BTreeSet::from([self.config.remote_root_id.clone()]),
            items: BTreeMap::new(),
        })
    }
    async fn round(
        &self,
        request: RemoteObserveRequest,
        session: &GoogleSession,
    ) -> Result<RemoteObserveResult, RemoteObserverError> {
        let mut state = self.state.lock().await;
        if request.known_remote_ids.len() > state.item_limit {
            state.item_limit = state.item_limit.saturating_mul(2).min(1_600_000);
            return Err(RemoteObserverError::IncompleteCoverage);
        }
        let budget = request.page_budget.unwrap_or(16).min(64);
        if budget == 0 {
            return Err(RemoteObserverError::IncompleteCoverage);
        }
        if matches!(
            request.mode,
            RemoteObserveMode::Initial | RemoteObserveMode::Recovery
        ) {
            state.capture = None;
            state.cursor = None;
            state.feed_token = None;
            state.feed_files = Some(BTreeMap::new());
            state.feed_seen.clear();
        }
        if state.capture.is_none() && state.cursor.is_none() {
            state.capture = Some(self.begin(session).await?);
        }
        let item_limit = state.item_limit;
        let mut pages = 0;
        let mut last_read_parent = None;
        if state.capture.is_none() {
            self.root(session).await?;
            let mut updated = BTreeSet::new();
            while pages < budget {
                if let Some(progress) = &request.progress {
                    progress.report(|| ScanProgress {
                        path: None,
                        items: state.known.len() as u64,
                        pages,
                    });
                }
                let _sample = crate::performance::Sample::begin("google_changes_page");
                let token = state
                    .feed_token
                    .clone()
                    .or_else(|| state.cursor.clone())
                    .ok_or(RemoteObserverError::InvalidCursor)?;
                let response=self.client.changes_list(session,ChangeListOptions {page_token:token.clone(),page_size:Some(100),include_removed:Some(true),include_corpus_removals:Some(true),include_items_from_all_drives:Some(true),supports_all_drives:Some(true),
                    fields:Some(FieldMask(format!("changes(fileId,removed,changeType,file({FIELDS})),nextPageToken,newStartPageToken"))),..Default::default()}).await;
                let response = match response {
                    Ok(response) => response,
                    Err(error) => {
                        if error.kind == GoogleApiError::ExpiredCursor {
                            state.cursor = None;
                            state.feed_token = None;
                            state.feed_seen.clear();
                        }
                        return Err(failure(error));
                    }
                };
                pages += 1;
                for change in response.changes {
                    let Some(files) = &mut state.feed_files else {
                        break;
                    };
                    let accepted = match (
                        change.change_type.as_deref(),
                        change.removed,
                        change.file_id,
                        change.file,
                    ) {
                        (Some("file"), Some(false), Some(id), Some(file))
                            if !id.0.is_empty()
                                && file.id.as_ref() == Some(&id)
                                && files.len() < item_limit =>
                        {
                            files.insert(RemoteId(id.0), file).is_none()
                        }
                        _ => false,
                    };
                    if !accepted {
                        state.feed_files = None;
                    }
                }
                if let Some(next) = response.next_page_token {
                    if state.feed_seen.len() >= 100_000
                        || next.is_empty()
                        || next == token
                        || !state.feed_seen.insert(next.clone())
                        || response.new_start_page_token.is_some()
                    {
                        return Err(RemoteObserverError::InvalidCursor);
                    }
                    state.feed_token = Some(next);
                } else {
                    let next = response
                        .new_start_page_token
                        .filter(|s| !s.is_empty())
                        .ok_or(RemoteObserverError::InvalidCursor)?;
                    let files = state.feed_files.take();
                    if state
                        .last_full
                        .is_none_or(|last| last.elapsed() >= AUDIT_INTERVAL)
                        || files.as_ref().is_none_or(|files| {
                            state.known.len().saturating_add(
                                files
                                    .keys()
                                    .filter(|id| !state.known.contains_key(*id))
                                    .count(),
                            ) > item_limit
                                || !files
                                    .iter()
                                    .all(|(id, file)| self.leaf_change(id, file, &state.known))
                        })
                    {
                        // Missing or structural change evidence requires a scoped listing.
                        state.capture = Some(self.begin(session).await?);
                    } else {
                        for (id, mut file) in files.unwrap_or_default() {
                            let previous = state.known.get(&id);
                            self.fill_sha256(session, &mut file, previous).await;
                            let mut value = normalize::observation(
                                file,
                                &self.config,
                                ObservationCoverage::Complete,
                            )?;
                            value.event.as_mut().unwrap().kind = observation_kind(previous, &value);
                            state.known.insert(id.clone(), value);
                            updated.insert(id);
                        }
                        state.cursor = Some(next);
                    }
                    state.feed_token = None;
                    state.feed_files = Some(BTreeMap::new());
                    state.feed_seen.clear();
                    break;
                }
            }
            if state.capture.is_none() {
                let mut observations: Vec<RemoteObservation> = if state.feed_token.is_none() {
                    state
                        .known
                        .values()
                        .cloned()
                        .map(|mut value| {
                            // A drained change feed proves unchanged known metadata up to this checkpoint.
                            value.observed_at = super::super::scanner::now();
                            value.page_token = state.cursor.clone();
                            value.event.as_mut().unwrap().observed_at = value.observed_at;
                            if value
                                .remote_id
                                .as_ref()
                                .is_none_or(|id| !updated.contains(id))
                            {
                                value.event.as_mut().unwrap().kind = ObservationKind::Snapshot;
                            }
                            value
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                if state.feed_token.is_none() {
                    for id in &request.known_remote_ids {
                        if !state.known.contains_key(id) && id != &self.config.remote_root_id {
                            observations.push(self.probe(session, id.clone(), &state.known).await);
                        }
                    }
                }
                if let Some(progress) = &request.progress {
                    progress.publish(true, || ScanProgress {
                        path: None,
                        items: state.known.len() as u64,
                        pages,
                    });
                }
                return Ok(self.result(
                    observations,
                    if state.feed_token.is_some() {
                        ObservationCoverage::Partial
                    } else {
                        ObservationCoverage::Complete
                    },
                    state.feed_token.clone().or(state.cursor.clone()),
                    state.feed_token.is_some(),
                ));
            }
        }
        while pages < budget {
            let State { capture, known, .. } = &mut *state;
            let capture = capture
                .as_mut()
                .ok_or(RemoteObserverError::IncompleteCoverage)?;
            let Some(parent) = capture.folders.front().cloned() else {
                break;
            };
            last_read_parent = Some(parent.clone());
            if capture.pages_read >= 100_000 {
                return Err(RemoteObserverError::IncompleteCoverage);
            }
            if let Some(progress) = &request.progress {
                progress.report(|| ScanProgress {
                    path: remote_path(&parent, &self.config.remote_root_id, &capture.items),
                    items: capture.items.len() as u64,
                    pages: capture.pages_read as u64,
                });
            }
            let _sample = crate::performance::Sample::begin("google_full_capture_page");
            let quote = parent.0.replace('\\', "\\\\").replace('\'', "\\'");
            let response = self
                .client
                .files_list(
                    session,
                    FileListOptions {
                        q: Some(format!("'{quote}' in parents and trashed = false")),
                        page_token: capture.page.clone(),
                        page_size: Some(100),
                        supports_all_drives: Some(true),
                        include_items_from_all_drives: Some(true),
                        fields: Some(FieldMask(format!(
                            "files({FIELDS}),nextPageToken,incompleteSearch"
                        ))),
                        ..Default::default()
                    },
                )
                .await
                .map_err(failure)?;
            pages += 1;
            capture.pages_read += 1;
            if response.incomplete_search != Some(false) {
                state.capture = None;
                state.cursor = None;
                return Err(RemoteObserverError::IncompleteCoverage);
            }
            // Validate a page completely before advancing its continuation or staged identities.
            let mut items = Vec::new();
            let mut ids = BTreeSet::new();
            for file in response.files {
                let id = file
                    .id
                    .as_ref()
                    .map(|id| RemoteId(id.0.clone()))
                    .filter(|id| !id.0.is_empty())
                    .ok_or(RemoteObserverError::IncompleteCoverage)?;
                if id == self.config.remote_root_id
                    || !ids.insert(id.clone())
                    || capture.items.contains_key(&id)
                    || !file
                        .parents
                        .as_ref()
                        .is_some_and(|parents| parents.len() == 1 && parents[0].0 == parent.0)
                    || file.trashed != Some(false)
                {
                    return Err(RemoteObserverError::IncompleteCoverage);
                }
                if file.mime_type.as_ref().map(|m| m.0.as_str()) == Some(FOLDER)
                    && capture.visited.contains(&id)
                {
                    return Err(RemoteObserverError::IncompleteCoverage);
                }
                if file.mime_type.as_ref().map(|m| m.0.as_str()) == Some(FOLDER)
                    && !file
                        .capabilities
                        .as_ref()
                        .is_some_and(|c| c.can_list_children == Some(true))
                {
                    return Err(RemoteObserverError::IncompleteCoverage);
                }
                items.push((id, file));
            }
            if capture.items.len() + items.len() > item_limit {
                state.item_limit = state.item_limit.saturating_mul(2).min(1_600_000);
                state.capture = None;
                state.cursor = None;
                return Err(RemoteObserverError::IncompleteCoverage);
            }
            if let Some(next) = &response.next_page_token {
                if next.is_empty()
                    || capture.page.as_ref() == Some(next)
                    || !capture.pages.insert((parent.clone(), next.clone()))
                {
                    return Err(RemoteObserverError::InvalidCursor);
                }
            }
            for (id, mut file) in items {
                self.fill_sha256(session, &mut file, known.get(&id)).await;
                if file.mime_type.as_ref().map(|m| m.0.as_str()) == Some(FOLDER) {
                    capture.visited.insert(id.clone());
                    capture.folders.push_back(id.clone());
                }
                capture.items.insert(
                    id,
                    normalize::observation(file, &self.config, ObservationCoverage::Partial)?,
                );
            }
            if let Some(next) = response.next_page_token {
                capture.page = Some(next);
            } else {
                capture.folders.pop_front();
                capture.page = None;
            }
            if capture.folders.is_empty() {
                break;
            }
        }
        let capture = state
            .capture
            .as_ref()
            .ok_or(RemoteObserverError::IncompleteCoverage)?;
        if let Some(progress) = &request.progress {
            let path = last_read_parent.as_ref().and_then(|parent| {
                remote_path(parent, &self.config.remote_root_id, &capture.items)
            });
            progress.publish(true, || ScanProgress {
                path,
                items: capture.items.len() as u64,
                pages: capture.pages_read as u64,
            });
        }
        if !capture.folders.is_empty() {
            return Ok(self.result(
                Vec::new(),
                ObservationCoverage::Partial,
                Some(capture.checkpoint.clone()),
                true,
            ));
        }
        // A missing listing entry does not distinguish trash, loss of access or a move.
        self.root(session).await?;
        let capture = state.capture.take().unwrap();
        let ids: BTreeSet<_> = state
            .known
            .keys()
            .cloned()
            .chain(request.known_remote_ids)
            .collect();
        if ids.len() > state.item_limit {
            state.item_limit = state.item_limit.saturating_mul(2).min(1_600_000);
            return Err(RemoteObserverError::IncompleteCoverage);
        }
        let mut observations = Vec::new();
        for id in ids {
            if !capture.items.contains_key(&id) && id != self.config.remote_root_id {
                observations.push(self.probe(session, id, &capture.items).await);
            }
        }
        let mut next = capture.items;
        for (id, value) in next.iter_mut() {
            value.coverage = ObservationCoverage::Complete;
            value.page_token = Some(capture.checkpoint.clone());
            value.event.as_mut().unwrap().kind = if !state.has_snapshot {
                ObservationKind::Snapshot
            } else {
                observation_kind(state.known.get(id), value)
            };
        }
        // Publish a complete initial/recovery view, so every item receives the final coverage.
        observations.extend(next.values().cloned());
        state.known = next;
        state.has_snapshot = true;
        state.last_full = Some(Instant::now());
        state.cursor = Some(capture.checkpoint);
        Ok(self.result(
            observations,
            ObservationCoverage::Complete,
            state.cursor.clone(),
            false,
        ))
    }
    /// Fast path for a leaf with a known parent and no change to its observed ancestry.
    fn leaf_change(
        &self,
        id: &RemoteId,
        file: &GoogleFile,
        known: &BTreeMap<RemoteId, RemoteObservation>,
    ) -> bool {
        if id == &self.config.remote_root_id
            || file.trashed != Some(false)
            || file.name.is_none()
            || file.mime_type.as_ref().is_none_or(|mime| mime.0 == FOLDER)
        {
            return false;
        }
        let Some([parent]) = file.parents.as_deref() else {
            return false;
        };
        let parent = RemoteId(parent.0.clone());
        (parent == self.config.remote_root_id
            || known.get(&parent).is_some_and(
                |folder| matches!(&folder.mime_type, Evidence::Known(mime) if mime == FOLDER),
            ))
            && known.get(id).is_none_or(|previous| {
                previous.parents == Evidence::Known(vec![parent])
                    && matches!((&previous.mime_type, &file.mime_type),
                        (Evidence::Known(previous), Some(current)) if *previous == current.0)
            })
    }

    async fn fill_sha256(
        &self,
        session: &GoogleSession,
        file: &mut GoogleFile,
        previous: Option<&RemoteObservation>,
    ) {
        if file
            .name
            .as_deref()
            .is_some_and(|name| crate::projections::libreoffice_lock_name(name.as_ref()))
        {
            return;
        }
        let cached = previous.and_then(|old| {
            let (Evidence::Known(version), Evidence::Known(size), Evidence::Known(checksum)) =
                (&old.version, &old.size, &old.checksum)
            else {
                return None;
            };
            (file.version.as_deref()?.parse::<u64>().ok()? == *version
                && file.size.as_deref()?.parse::<u64>().ok()? == *size
                && checksum.algorithm == "sha256")
                .then_some(checksum.value.as_str())
        });
        // Metadata remains available for diagnostics even when media verification fails.
        let _ = self.client.fill_content_sha256(session, file, cached).await;
    }

    /// A failed specific read is never evidence of deletion. Ancestry must reach an
    /// observed external root before an item can be classified as outside scope.
    async fn probe(
        &self,
        session: &GoogleSession,
        id: RemoteId,
        captured: &BTreeMap<RemoteId, RemoteObservation>,
    ) -> RemoteObservation {
        let unavailable = |permission: bool| {
            let mut value = normalize::missing(id.clone(), &self.config);
            value.state = if permission {
                FileState::Unavailable
            } else {
                FileState::Unknown
            };
            if permission {
                value.capabilities = Evidence::PermissionDenied;
            }
            value
        };
        let get = |id: RemoteId| async move {
            self.client
                .files_get(
                    session,
                    &FileId(id.0),
                    FileGetOptions {
                        fields: Some(FieldMask(FIELDS.into())),
                        supports_all_drives: Some(true),
                        ..Default::default()
                    },
                )
                .await
        };
        let file = match get(id.clone()).await {
            Ok(file) if file.id.as_ref().is_some_and(|value| value.0 == id.0) => file,
            Ok(_) => return unavailable(false),
            Err(error) => {
                let mut value = unavailable(error.kind == GoogleApiError::PermissionDenied);
                if error.kind != GoogleApiError::NotFoundOrInaccessible {
                    value.state = FileState::Unavailable;
                }
                return value;
            }
        };
        let mut value =
            match normalize::observation(file, &self.config, ObservationCoverage::Complete) {
                Ok(value) => value,
                Err(_) => return unavailable(false),
            };
        if value.trashed == Evidence::Known(true) {
            value.state = FileState::Absent;
            return value;
        }
        if value.trashed != Evidence::Known(false) {
            return unavailable(false);
        }
        let mut parents = value.parents.clone();
        let mut visited = BTreeSet::from([id.clone()]);
        for _ in 0..128 {
            let Evidence::Known(ids) = parents else {
                return unavailable(false);
            };
            if ids.is_empty() {
                value.state = FileState::OutsideScope;
                return value;
            }
            if ids.len() != 1
                || ids[0] == self.config.remote_root_id
                || captured.contains_key(&ids[0])
                || !visited.insert(ids[0].clone())
            {
                return unavailable(false);
            }
            let parent = match get(ids[0].clone()).await {
                Ok(parent)
                    if parent.id.as_ref().is_some_and(|value| value.0 == ids[0].0)
                        && parent.trashed == Some(false) =>
                {
                    parent
                }
                Err(error) => return unavailable(error.kind == GoogleApiError::PermissionDenied),
                _ => return unavailable(false),
            };
            parents = parent
                .parents
                .map(|ids| Evidence::Known(ids.into_iter().map(|id| RemoteId(id.0)).collect()))
                .unwrap_or(Evidence::NotCollected);
        }
        unavailable(false)
    }
    fn result(
        &self,
        observations: Vec<RemoteObservation>,
        coverage: ObservationCoverage,
        cursor: Option<String>,
        has_more: bool,
    ) -> RemoteObserveResult {
        RemoteObserveResult {
            observations: observations
                .into_iter()
                .filter(|item| {
                    !matches!(&item.name, Evidence::Known(name) if crate::projections::libreoffice_lock_name(name.as_ref()))
                })
                .collect(),
            coverage,
            continuation: cursor.map(|value| ProviderCursor {
                provider_id: self.config.provider_id.clone(),
                value,
            }),
            has_more,
        }
    }
}
fn observation_kind(
    previous: Option<&RemoteObservation>,
    current: &RemoteObservation,
) -> ObservationKind {
    match previous {
        None => ObservationKind::Created,
        Some(old) if old.parents != current.parents => ObservationKind::Moved,
        Some(old) if old.name != current.name => ObservationKind::Renamed,
        Some(old) if old.checksum != current.checksum || old.size != current.size => {
            ObservationKind::Modified
        }
        Some(old) if old.version != current.version || old.capabilities != current.capabilities => {
            ObservationKind::MetadataChanged
        }
        _ => ObservationKind::Snapshot,
    }
}

fn failure(error: GoogleApiFailure) -> RemoteObserverError {
    match error.kind {
        GoogleApiError::AuthenticationRequired
        | GoogleApiError::ReauthorizationRequired
        | GoogleApiError::InvalidGrant
        | GoogleApiError::InsufficientScope => RemoteObserverError::AuthorizationRequired,
        GoogleApiError::PermissionDenied => RemoteObserverError::PermissionDenied,
        GoogleApiError::NotFoundOrInaccessible => RemoteObserverError::IncompleteCoverage,
        GoogleApiError::ExpiredCursor => RemoteObserverError::InvalidCursor,
        _ => RemoteObserverError::Unavailable,
    }
}
#[cfg(test)]
mod tests;

/// Resolve only observed parent chains; provider identities are never shown as paths.
fn remote_path(
    id: &RemoteId,
    root: &RemoteId,
    items: &BTreeMap<RemoteId, RemoteObservation>,
) -> Option<Path> {
    let mut id = id.clone();
    let mut names = Vec::new();
    for _ in 0..=128 {
        if &id == root {
            return Some(names.into_iter().rev().collect());
        }
        let item = items.get(&id)?;
        let Evidence::Known(name) = &item.name else {
            return None;
        };
        if name.is_empty()
            || std::path::Path::new(name).components().count() != 1
            || !std::path::Path::new(name)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
        {
            return None;
        }
        names.push(name.clone());
        let Evidence::Known(parents) = &item.parents else {
            return None;
        };
        if parents.len() != 1 {
            return None;
        }
        id = parents[0].clone();
    }
    None
}
