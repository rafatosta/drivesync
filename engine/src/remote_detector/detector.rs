//! Bounded inventory reconciliation and durable event publication.
//!
//! Scans are repeated until a full scan is followed by an empty change-log
//! interval. A nonconvergent or incomplete run never advances the checkpoint.
use super::{
    journal::{Journal, JournalError, Scope},
    provider::{Change, ChangePage, Corpus, File, FilePage, GoogleDrive, ProviderError},
};
use crate::{configuration::valid_id, AuthError, EngineError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet, VecDeque};

const FOLDER: &str = "application/vnd.google-apps.folder";

/// Resource limits for one check; exhaustion is reported as incomplete.
#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub requests: usize,
    pub items: usize,
    pub rounds: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            requests: 10_000,
            items: 100_000,
            rounds: 8,
        }
    }
}

/// Membership is based on complete inventories, not file names or paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Membership {
    /// The item is inside the monitored tree.
    Inside,
    /// The item is outside the monitored tree.
    Outside,
    /// The available information cannot establish membership.
    Unknown,
}

/// Baseline entries and subsequent observations have distinct meanings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    /// A baseline item, not an inferred creation.
    Initial,
    /// The initial inventory was completely recorded.
    InitialComplete,
    /// The item or subtree needs re-evaluation.
    Reevaluate,
}

/// An observation contains no operation decision and may require rechecking.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    /// Account identifier.
    pub account: String,
    /// Monitored root identifier.
    pub root: String,
    /// Baseline or later observation.
    pub kind: EventKind,
    /// File identifier, absent for the initial completion marker.
    pub file_id: Option<String>,
    /// Membership at the previous confirmed inventory.
    pub before: Membership,
    /// Membership at the new inventory, or unknown.
    pub after: Membership,
    /// Previously observed metadata, when available.
    pub previous: Option<File>,
    /// Current or provider-reported metadata, when available.
    pub current: Option<File>,
}

/// Event IDs are assigned durably and remain stable across redelivery.
#[derive(Debug, Serialize)]
pub struct DeliveredEvent {
    /// Stable identifier used to acknowledge this delivery.
    pub id: i64,
    /// Observed change, not an operation decision.
    pub event: Event,
}

/// A completed check; returning events never acknowledges them.
#[derive(Debug, Serialize)]
pub enum Check {
    /// The initial inventory and pending observations.
    Initialized(Vec<DeliveredEvent>),
    /// Pending observations, including previously unacknowledged ones.
    Changes(Vec<DeliveredEvent>),
    /// No pending observations remain.
    NoChanges,
}

/// Sanitized failure. A storage commit error can have an ambiguous outcome.
#[derive(Debug)]
pub enum DetectionError {
    /// The result is incomplete or a configured work limit was reached.
    Incomplete,
    /// Retry after a concurrent change or a temporary storage failure.
    Retry,
    /// Configuration, provider response or persisted state needs intervention.
    Intervention,
    /// Drive rejected the stored cursor; state must not be reset silently.
    CursorRejected,
    /// The provider response was malformed or internally inconsistent.
    InvalidResponse,
    /// The on-disk database uses an unsupported format.
    IncompatibleState,
    /// Sanitized account, vault or remote provider failure.
    Provider(EngineError),
}

/// High-level action required after a failed check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionFailureKind {
    /// The bounded check did not establish a complete result.
    Incomplete,
    /// Retry after a transient problem or conflicting writer.
    Recoverable,
    /// User or operator intervention is required.
    Intervention,
}

impl DetectionError {
    /// Classifies the failure without exposing provider response bodies.
    pub fn kind(&self) -> DetectionFailureKind {
        match self {
            Self::Incomplete => DetectionFailureKind::Incomplete,
            Self::Retry
            | Self::Provider(EngineError::RemoteRateLimited)
            | Self::Provider(EngineError::Authentication(
                AuthError::Network
                | AuthError::ProviderUnavailable
                | AuthError::TimedOut
                | AuthError::Busy,
            )) => DetectionFailureKind::Recoverable,
            Self::Intervention
            | Self::CursorRejected
            | Self::InvalidResponse
            | Self::IncompatibleState
            | Self::Provider(_) => DetectionFailureKind::Intervention,
        }
    }
}

impl std::fmt::Display for DetectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Incomplete => f.write_str("Remote verification is incomplete; no checkpoint was committed."),
            Self::Retry => f.write_str("Remote verification could not commit; retry after checking storage or concurrent activity."),
            Self::Intervention => f.write_str("Remote verification requires intervention; existing state was preserved."),
            Self::CursorRejected => f.write_str("Google Drive rejected the change cursor; existing state and pending events were preserved."),
            Self::InvalidResponse => f.write_str("Google Drive returned an invalid response; existing state was preserved."),
            Self::IncompatibleState => f.write_str("Remote detection state has an unsupported format; existing data was preserved."),
            Self::Provider(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for DetectionError {}

/// Port implemented by the production adapter and deterministic test provider.
trait Provider {
    fn start(&self, corpus: &Corpus) -> Result<String, ProviderError>;
    fn changes(&self, corpus: &Corpus, cursor: &str) -> Result<ChangePage, ProviderError>;
    fn children(
        &self,
        corpus: &Corpus,
        parent: &str,
        page: Option<&str>,
    ) -> Result<FilePage, ProviderError>;
    fn file(&self, id: &str) -> Result<File, ProviderError>;
}

impl Provider for GoogleDrive<'_> {
    fn start(&self, corpus: &Corpus) -> Result<String, ProviderError> {
        self.start(corpus)
    }
    fn changes(&self, corpus: &Corpus, cursor: &str) -> Result<ChangePage, ProviderError> {
        self.changes(corpus, cursor)
    }
    fn children(
        &self,
        corpus: &Corpus,
        parent: &str,
        page: Option<&str>,
    ) -> Result<FilePage, ProviderError> {
        self.children(corpus, parent, page)
    }
    fn file(&self, id: &str) -> Result<File, ProviderError> {
        self.file(id)
    }
}

impl From<JournalError> for DetectionError {
    fn from(error: JournalError) -> Self {
        match error {
            JournalError::Stale | JournalError::Storage => Self::Retry,
            JournalError::Incompatible => Self::IncompatibleState,
        }
    }
}

impl From<ProviderError> for DetectionError {
    fn from(error: ProviderError) -> Self {
        match error {
            ProviderError::Incomplete => Self::Incomplete,
            ProviderError::RejectedCursor => Self::CursorRejected,
            ProviderError::InvalidResponse => Self::InvalidResponse,
            ProviderError::Request(error) => Self::Provider(error),
        }
    }
}

type Inventory = BTreeMap<String, File>;

struct Budget {
    requests: usize,
    items: usize,
}

impl Budget {
    fn request(&mut self) -> Result<(), DetectionError> {
        self.requests = self
            .requests
            .checked_sub(1)
            .ok_or(DetectionError::Incomplete)?;
        Ok(())
    }

    fn items(&mut self, count: usize) -> Result<(), DetectionError> {
        self.items = self
            .items
            .checked_sub(count)
            .ok_or(DetectionError::Incomplete)?;
        Ok(())
    }
}

/// Checks a scope with the verified-account provider and commits atomically.
/// The current stage of an automatic remote check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteCheckProgress {
    ReadingChanges,
    ListingFolder(String),
    ApplyingChanges,
}

pub(super) fn check(
    provider: &GoogleDrive<'_>,
    journal: &Journal,
    scope: &Scope,
    corpus: &Corpus,
    limits: Limits,
    incremental: bool,
    progress: &dyn Fn(RemoteCheckProgress),
) -> Result<Check, DetectionError> {
    verify_with_progress(
        provider,
        journal,
        scope,
        corpus,
        limits,
        incremental,
        progress,
    )
}

#[cfg(test)]
fn check_with(
    provider: &impl Provider,
    journal: &Journal,
    scope: &Scope,
    corpus: &Corpus,
    limits: Limits,
) -> Result<Check, DetectionError> {
    verify(provider, journal, scope, corpus, limits, false)
}

#[cfg(test)]
fn verify(
    provider: &impl Provider,
    journal: &Journal,
    scope: &Scope,
    corpus: &Corpus,
    limits: Limits,
    incremental: bool,
) -> Result<Check, DetectionError> {
    verify_with_progress(
        provider,
        journal,
        scope,
        corpus,
        limits,
        incremental,
        &|_| {},
    )
}

fn verify_with_progress(
    provider: &impl Provider,
    journal: &Journal,
    scope: &Scope,
    corpus: &Corpus,
    limits: Limits,
    incremental: bool,
    progress: &dyn Fn(RemoteCheckProgress),
) -> Result<Check, DetectionError> {
    if !valid_id(&scope.root)
        || scope.account.is_empty()
        || scope.client.is_empty()
        || match corpus {
            Corpus::User => scope.corpus != "user",
            Corpus::SharedDrive(id) => !valid_id(id) || scope.corpus != format!("drive:{id}"),
        }
    {
        return Err(DetectionError::Intervention);
    }
    let old = journal.load(scope)?;
    let previous: Inventory = match &old {
        Some(checkpoint) => serde_json::from_slice(&checkpoint.inventory)
            .map_err(|_| DetectionError::Intervention)?,
        None => Inventory::new(),
    };
    if old.is_some() {
        validate_inventory(&previous, &scope.root)?;
    }
    let mut budget = Budget {
        requests: limits.requests,
        items: limits.items,
    };
    let mut cursor = match &old {
        Some(checkpoint) => checkpoint.cursor.clone(),
        None => {
            budget.request()?;
            provider.start(corpus)?
        }
    };
    let mut seen_changes = Vec::new();
    let mut stable = None;
    if incremental {
        if let Some(checkpoint) = &old {
            progress(RemoteCheckProgress::ReadingChanges);
            let (next, changes) = read_changes(provider, corpus, &cursor, &mut budget)?;
            cursor = next;
            if changes.is_empty() {
                if cursor != checkpoint.cursor {
                    journal.commit(
                        scope,
                        Some(checkpoint.generation),
                        &cursor,
                        &checkpoint.inventory,
                        &[],
                    )?;
                }
                return pending_check(journal, scope);
            }
            stable = apply_file_changes(&previous, &changes, &scope.root, corpus);
            seen_changes.extend(changes);
        }
    }
    if stable.is_none() {
        for _ in 0..limits.rounds {
            let inventory = scan(provider, corpus, &scope.root, &mut budget, progress)?;
            progress(RemoteCheckProgress::ReadingChanges);
            let (next, changes) = read_changes(provider, corpus, &cursor, &mut budget)?;
            cursor = next;
            if changes.is_empty() {
                stable = Some(inventory);
                break;
            }
            seen_changes.extend(changes);
        }
    }
    let current = stable.ok_or(DetectionError::Incomplete)?;
    let mut events = Vec::new();
    if old.is_none() {
        for file in current.values() {
            events.push(Event {
                account: scope.account.clone(),
                root: scope.root.clone(),
                kind: EventKind::Initial,
                file_id: Some(file.id.clone()),
                before: Membership::Unknown,
                after: Membership::Inside,
                previous: None,
                current: Some(file.clone()),
            });
        }
        events.push(Event {
            account: scope.account.clone(),
            root: scope.root.clone(),
            kind: EventKind::InitialComplete,
            file_id: None,
            before: Membership::Unknown,
            after: Membership::Inside,
            previous: None,
            current: None,
        });
    }
    let mut changed_ids = HashSet::new();
    for change in seen_changes {
        if change.change_type.as_deref() == Some("drive") {
            if matches!(corpus, Corpus::SharedDrive(_)) {
                return Err(DetectionError::Intervention);
            }
            continue;
        }
        let id = change.file_id.ok_or(DetectionError::Incomplete)?;
        if !valid_id(&id) {
            return Err(DetectionError::Intervention);
        }
        changed_ids.insert(id.clone());
        let before = previous.get(&id);
        let after = current.get(&id);
        let unknown = change.removed || change.file.is_none();
        if before.is_none() && after.is_none() && !unknown && old.is_some() {
            continue;
        }
        events.push(Event {
            account: scope.account.clone(),
            root: scope.root.clone(),
            kind: EventKind::Reevaluate,
            file_id: Some(id),
            before: if before.is_some() {
                Membership::Inside
            } else if old.is_some() {
                Membership::Outside
            } else {
                Membership::Unknown
            },
            after: if after.is_some() {
                Membership::Inside
            } else if unknown {
                Membership::Unknown
            } else {
                Membership::Outside
            },
            previous: before.cloned(),
            current: after.cloned().or(change.file),
        });
    }
    if old.is_some() {
        for id in previous.keys().chain(current.keys()) {
            if changed_ids.contains(id) {
                continue;
            }
            let before = previous.get(id);
            let after = current.get(id);
            if before.is_some() == after.is_some() {
                continue;
            }
            changed_ids.insert(id.clone());
            events.push(Event {
                account: scope.account.clone(),
                root: scope.root.clone(),
                kind: EventKind::Reevaluate,
                file_id: Some(id.clone()),
                before: if before.is_some() {
                    Membership::Inside
                } else {
                    Membership::Outside
                },
                after: if after.is_some() {
                    Membership::Inside
                } else {
                    Membership::Unknown
                },
                previous: before.cloned(),
                current: after.cloned(),
            });
        }
    }
    let inventory = serde_json::to_vec(&current).map_err(|_| DetectionError::Intervention)?;
    let payloads = events
        .iter()
        .map(serde_json::to_vec)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| DetectionError::Intervention)?;
    journal.commit(
        scope,
        old.as_ref().map(|c| c.generation),
        &cursor,
        &inventory,
        &payloads,
    )?;
    if old.is_none() {
        Ok(Check::Initialized(pending(journal, scope)?))
    } else {
        pending_check(journal, scope)
    }
}

fn pending_check(journal: &Journal, scope: &Scope) -> Result<Check, DetectionError> {
    let events = pending(journal, scope)?;
    if events.is_empty() {
        Ok(Check::NoChanges)
    } else {
        Ok(Check::Changes(events))
    }
}

/// Apply unambiguous file metadata without listing folders. Folder changes,
/// missing metadata and unknown parents keep the full reconciliation path.
fn apply_file_changes(
    previous: &Inventory,
    changes: &[Change],
    root: &str,
    corpus: &Corpus,
) -> Option<Inventory> {
    let mut current = previous.clone();
    for change in changes {
        let id = change.file_id.as_deref()?;
        let file = change.file.as_ref()?;
        if change.removed
            || id == root
            || id != file.id
            || !valid_id(id)
            || file.name.is_none()
            || file.trashed != Some(false)
            || file.mime_type.as_deref()? == FOLDER
            || previous
                .get(id)
                .is_some_and(|entry| entry.mime_type.as_deref() == Some(FOLDER))
            || match (corpus, file.drive_id.as_deref()) {
                (Corpus::User, None) => false,
                (Corpus::SharedDrive(expected), Some(actual)) => expected != actual,
                _ => true,
            }
        {
            return None;
        }
        let [parent] = file.parents.as_deref()? else {
            return None;
        };
        let parent = previous.get(parent)?;
        if parent.mime_type.as_deref() != Some(FOLDER) {
            return None;
        }
        current.insert(id.into(), file.clone());
    }
    Some(current)
}

pub(super) fn pending(
    journal: &Journal,
    scope: &Scope,
) -> Result<Vec<DeliveredEvent>, DetectionError> {
    journal
        .pending(scope)?
        .into_iter()
        .map(|entry| {
            let event: Event =
                serde_json::from_slice(&entry.payload).map_err(|_| DetectionError::Intervention)?;
            if event.account != scope.account || event.root != scope.root {
                return Err(DetectionError::Intervention);
            }
            Ok(DeliveredEvent {
                id: entry.id,
                event,
            })
        })
        .collect()
}

fn validate_inventory(inventory: &Inventory, root: &str) -> Result<(), DetectionError> {
    let root_file = inventory.get(root).ok_or(DetectionError::Intervention)?;
    if root_file.mime_type.as_deref() != Some(FOLDER) || root_file.trashed != Some(false) {
        return Err(DetectionError::Intervention);
    }
    for (id, file) in inventory {
        if id != &file.id
            || !valid_id(id)
            || file.name.is_none()
            || file.mime_type.is_none()
            || file.trashed != Some(false)
            || (file.mime_type.as_deref() == Some(FOLDER) && !can_list(file))
        {
            return Err(DetectionError::Intervention);
        }
        let mut current = id.as_str();
        let mut visited = HashSet::new();
        while current != root {
            if !visited.insert(current) {
                return Err(DetectionError::Intervention);
            }
            let item = inventory.get(current).ok_or(DetectionError::Intervention)?;
            let parents = item
                .parents
                .as_ref()
                .filter(|p| p.len() == 1)
                .ok_or(DetectionError::Intervention)?;
            let parent = inventory
                .get(&parents[0])
                .ok_or(DetectionError::Intervention)?;
            if parent.mime_type.as_deref() != Some(FOLDER) {
                return Err(DetectionError::Intervention);
            }
            current = &parent.id;
        }
    }
    Ok(())
}

fn scan(
    provider: &impl Provider,
    corpus: &Corpus,
    root: &str,
    budget: &mut Budget,
    progress: &dyn Fn(RemoteCheckProgress),
) -> Result<Inventory, DetectionError> {
    budget.request()?;
    let root_file = provider.file(root)?;
    if root_file.id != root
        || root_file.name.is_none()
        || root_file.mime_type.as_deref() != Some(FOLDER)
        || root_file.trashed != Some(false)
        || !can_list(&root_file)
    {
        return Err(DetectionError::Intervention);
    }
    match (corpus, root_file.drive_id.as_deref()) {
        (Corpus::User, None) => {}
        (Corpus::SharedDrive(id), Some(actual)) if id == actual => {}
        _ => return Err(DetectionError::Intervention),
    }
    budget.items(1)?;
    let mut inventory = BTreeMap::from([(root.to_owned(), root_file)]);
    let mut folders = VecDeque::from([root.to_owned()]);
    while let Some(parent) = folders.pop_front() {
        if let Some(name) = inventory.get(&parent).and_then(|file| file.name.as_ref()) {
            progress(RemoteCheckProgress::ListingFolder(name.clone()));
        }
        let mut token: Option<String> = None;
        let mut visited = HashSet::new();
        loop {
            budget.request()?;
            let page = provider.children(corpus, &parent, token.as_deref())?;
            if page.incomplete_search {
                return Err(DetectionError::Incomplete);
            }
            budget.items(page.files.len())?;
            for file in page.files {
                if !valid_id(&file.id)
                    || file.name.is_none()
                    || file.mime_type.is_none()
                    || file.trashed != Some(false)
                    || match (corpus, file.drive_id.as_deref()) {
                        (Corpus::User, None) => false,
                        (Corpus::SharedDrive(id), Some(actual)) => id != actual,
                        _ => true,
                    }
                    || !file
                        .parents
                        .as_ref()
                        .is_some_and(|p| p.len() == 1 && p[0] == parent)
                {
                    return Err(DetectionError::Incomplete);
                }
                if inventory.contains_key(&file.id) {
                    return Err(DetectionError::Incomplete);
                }
                if file.mime_type.as_deref() == Some(FOLDER) {
                    if !can_list(&file) {
                        return Err(DetectionError::Incomplete);
                    }
                    folders.push_back(file.id.clone());
                }
                inventory.insert(file.id.clone(), file);
            }
            match page.next_page_token {
                None => break,
                Some(next) if !next.is_empty() && visited.insert(next.clone()) => {
                    token = Some(next)
                }
                Some(_) => return Err(DetectionError::Incomplete),
            }
        }
    }
    Ok(inventory)
}

fn can_list(file: &File) -> bool {
    file.capabilities.as_ref().and_then(|c| c.can_list_children) == Some(true)
}

fn read_changes(
    provider: &impl Provider,
    corpus: &Corpus,
    start: &str,
    budget: &mut Budget,
) -> Result<(String, Vec<Change>), DetectionError> {
    let mut cursor = start.to_owned();
    let mut visited = HashSet::from([cursor.clone()]);
    let mut changes = Vec::new();
    loop {
        budget.request()?;
        let page = provider.changes(corpus, &cursor)?;
        budget.items(page.changes.len())?;
        changes.extend(page.changes);
        match (page.next_page_token, page.new_start_page_token) {
            (Some(next), None) if !next.is_empty() && visited.insert(next.clone()) => cursor = next,
            (None, Some(next)) if !next.is_empty() => return Ok((next, changes)),
            _ => return Err(DetectionError::Incomplete),
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
