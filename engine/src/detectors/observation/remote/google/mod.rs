//! Google read-only observation. Changes invalidate a scoped capture rather than imply deletion.
mod normalize;
use super::{interfaces::*, models::*};
use crate::detectors::observation::{ObservationCoverage, RemoteObservation};
use crate::{models::*, providers::google::*};
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet, VecDeque},
};
use tokio::sync::Mutex;
const FIELDS: &str =
    "id,name,parents,mimeType,size,sha256Checksum,version,modifiedTime,trashed,capabilities";
const FOLDER: &str = "application/vnd.google-apps.folder";
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
#[derive(Default)]
struct State {
    known: BTreeMap<RemoteId, RemoteObservation>,
    cursor: Option<String>,
    capture: Option<Capture>,
    feed_token: Option<String>,
    feed_dirty: bool,
    feed_seen: BTreeSet<String>,
    has_snapshot: bool,
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
            state.feed_dirty = false;
            state.feed_seen.clear();
        }
        if state.capture.is_none() && state.cursor.is_none() {
            state.capture = Some(self.begin(session).await?);
        }
        let mut pages = 0;
        if state.capture.is_none() {
            self.root(session).await?;
            while pages < budget {
                let token = state
                    .feed_token
                    .clone()
                    .or_else(|| state.cursor.clone())
                    .ok_or(RemoteObserverError::InvalidCursor)?;
                let response=self.client.changes_list(session,ChangeListOptions {page_token:token.clone(),page_size:Some(100),include_removed:Some(true),include_corpus_removals:Some(true),include_items_from_all_drives:Some(true),supports_all_drives:Some(true),
                    fields:Some(FieldMask("changes(fileId,removed,changeType),nextPageToken,newStartPageToken".into())),..Default::default()}).await;
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
                state.feed_dirty |= !response.changes.is_empty();
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
                    if state.feed_dirty {
                        // Account-wide change entries are only invalidations. Recapture the selected
                        // subtree to resolve moves, permissions and folder ancestry consistently.
                        state.capture = Some(self.begin(session).await?);
                    } else {
                        state.cursor = Some(next);
                    }
                    state.feed_token = None;
                    state.feed_dirty = false;
                    state.feed_seen.clear();
                    break;
                }
            }
            if state.capture.is_none() {
                let observations = if state.feed_token.is_none() {
                    state
                        .known
                        .values()
                        .cloned()
                        .map(|mut value| {
                            // A drained change feed proves unchanged known metadata up to this checkpoint.
                            value.observed_at = super::super::scanner::now();
                            value.page_token = state.cursor.clone();
                            value.event.as_mut().unwrap().observed_at = value.observed_at;
                            value.event.as_mut().unwrap().kind = ObservationKind::Snapshot;
                            value
                        })
                        .collect()
                } else {
                    Vec::new()
                };
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
            let capture = state
                .capture
                .as_mut()
                .ok_or(RemoteObserverError::IncompleteCoverage)?;
            let Some(parent) = capture.folders.front().cloned() else {
                break;
            };
            if capture.pages_read >= 100_000 {
                return Err(RemoteObserverError::IncompleteCoverage);
            }
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
            if capture.items.len() + items.len() > MAX_ITEMS {
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
            for (id, file) in items {
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
        if !capture.folders.is_empty() {
            return Ok(self.result(
                Vec::new(),
                ObservationCoverage::Partial,
                Some(capture.checkpoint.clone()),
                true,
            ));
        }
        // Inaccessible or moved-out items are outside this scope, never proven deleted.
        self.root(session).await?;
        let capture = state.capture.take().unwrap();
        let mut observations = Vec::new();
        for (id, old) in &state.known {
            if !capture.items.contains_key(id) {
                let mut departed = old.clone();
                departed.state = FileState::OutsideScope;
                departed.coverage = ObservationCoverage::Complete;
                departed.observed_at = super::super::scanner::now();
                departed.event.as_mut().unwrap().kind = ObservationKind::Removed;
                departed.event.as_mut().unwrap().observed_at = departed.observed_at;
                departed.name = Evidence::NotCollected;
                departed.parents = Evidence::NotCollected;
                departed.mime_type = Evidence::NotCollected;
                departed.size = Evidence::NotCollected;
                departed.checksum = Evidence::NotCollected;
                departed.version = Evidence::NotCollected;
                departed.modified_time = Evidence::NotCollected;
                departed.trashed = Evidence::NotCollected;
                departed.capabilities = Evidence::NotCollected;
                departed.page_token = Some(capture.checkpoint.clone());
                observations.push(departed);
            }
        }
        let mut next = capture.items;
        for (id, value) in next.iter_mut() {
            value.coverage = ObservationCoverage::Complete;
            value.page_token = Some(capture.checkpoint.clone());
            value.event.as_mut().unwrap().kind = if !state.has_snapshot {
                ObservationKind::Snapshot
            } else {
                match state.known.get(id) {
                    None => ObservationKind::Created,
                    Some(old) if old.parents != value.parents => ObservationKind::Moved,
                    Some(old) if old.name != value.name => ObservationKind::Renamed,
                    Some(old) if old.checksum != value.checksum || old.size != value.size => {
                        ObservationKind::Modified
                    }
                    Some(old)
                        if old.version != value.version
                            || old.capabilities != value.capabilities =>
                    {
                        ObservationKind::MetadataChanged
                    }
                    _ => ObservationKind::Snapshot,
                }
            };
        }
        // Publish a complete initial/recovery view, so every item receives the final coverage.
        observations.extend(next.values().cloned());
        state.known = next;
        state.has_snapshot = true;
        state.cursor = Some(capture.checkpoint);
        Ok(self.result(
            observations,
            ObservationCoverage::Complete,
            state.cursor.clone(),
            false,
        ))
    }
    fn result(
        &self,
        observations: Vec<RemoteObservation>,
        coverage: ObservationCoverage,
        cursor: Option<String>,
        has_more: bool,
    ) -> RemoteObserveResult {
        RemoteObserveResult {
            observations,
            coverage,
            continuation: cursor.map(|value| ProviderCursor {
                provider_id: self.config.provider_id.clone(),
                value,
            }),
            has_more,
        }
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
