//! Bounded catch-up from a durable cursor, with bootstrap-window reconciliation.
use super::{Checkpoint, Notice};
use crate::{
    remote::{Change, ChangePage},
    AccountId, AuthError, Cancellation, EngineError, InventoryKind, RemoteInventory,
};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

/// Read-only provider seam. All methods must use the same account and root.
pub(crate) trait Provider {
    fn start(&mut self) -> Result<String, EngineError>;
    fn scan(&mut self) -> Result<RemoteInventory, EngineError>;
    fn page(&mut self, token: &str) -> Result<ChangePage, EngineError>;
}
fn cancelled(cancel: &Cancellation) -> Result<(), EngineError> {
    if cancel.is_cancelled() {
        Err(EngineError::Authentication(AuthError::Cancelled))
    } else {
        Ok(())
    }
}
fn invalid() -> EngineError {
    EngineError::Authentication(AuthError::InvalidResponse)
}

/// Returns a complete new checkpoint in memory. The caller commits it atomically
/// with its cursor before allowing transfers. Failure leaves prior state intact.
pub(crate) fn collect(
    account: &AccountId,
    root: &str,
    previous: Option<&Checkpoint>,
    cancel: &Cancellation,
    source: &mut impl Provider,
) -> Result<Checkpoint, EngineError> {
    collect_mode(account, root, previous, cancel, source, false)
}

/// Rebuilds observations after transfer revalidation exposes a stale feed cache.
/// The boundary precedes the scan, prior notices survive and the catch-up window
/// is replayed exactly as during bootstrap. This never changes local files.
pub(crate) fn reconcile(
    account: &AccountId,
    root: &str,
    previous: &Checkpoint,
    cancel: &Cancellation,
    source: &mut impl Provider,
) -> Result<Checkpoint, EngineError> {
    collect_mode(account, root, Some(previous), cancel, source, true)
}

fn collect_mode(
    account: &AccountId,
    root: &str,
    previous: Option<&Checkpoint>,
    cancel: &Cancellation,
    source: &mut impl Provider,
    force_scan: bool,
) -> Result<Checkpoint, EngineError> {
    cancelled(cancel)?;
    if let Some(old) = previous {
        validate(account, root, &old.inventory)?;
    }
    let reclassify_old_google_items = previous.is_some_and(|old| {
        old.inventory.entries.iter().any(|item| {
            item.kind == crate::InventoryKind::Skipped(crate::SkipReason::GoogleNativeOrShortcut)
        })
    });
    let mut current = previous
        .filter(|_| !force_scan && !reclassify_old_google_items)
        .cloned();
    let mut notices = previous.map(|old| old.notices.clone()).unwrap_or_default();
    let mut observed = previous.map(|old| old.inventory.clone());
    for _ in 0..3 {
        cancelled(cancel)?;
        let (cursor, inventory) = match &current {
            Some(old) => (old.cursor.clone(), old.inventory.clone()),
            None => {
                let cursor = source.start()?;
                cancelled(cancel)?;
                let inventory = source.scan()?;
                validate(account, root, &inventory)?;
                if let Some(old) = &observed {
                    reconcile_notices(old, &inventory, &mut notices)?;
                }
                observed = Some(inventory.clone());
                (cursor, inventory)
            }
        };
        let batch = drain(&cursor, cancel, source);
        let (next, events) = match batch {
            Err(EngineError::ChangeCursorRejected) => {
                current = None;
                continue;
            }
            result => result?,
        };
        match apply(&inventory, &events, cancel)? {
            Some(inventory) => {
                if let Some(old) = &observed {
                    reconcile_notices(old, &inventory, &mut notices)?;
                }
                cancelled(cancel)?;
                return Ok(Checkpoint {
                    cursor: next,
                    inventory,
                    notices,
                });
            }
            None => {
                // Capture a new boundary before rescanning, then replay its window.
                let cursor = source.start()?;
                cancelled(cancel)?;
                let inventory = source.scan()?;
                validate(account, root, &inventory)?;
                if let Some(old) = &observed {
                    reconcile_notices(old, &inventory, &mut notices)?;
                }
                observed = Some(inventory.clone());
                current = Some(Checkpoint {
                    cursor,
                    inventory,
                    notices: Vec::new(),
                });
            }
        }
    }
    Err(EngineError::RemoteChanged)
}

fn drain(
    start: &str,
    cancel: &Cancellation,
    source: &mut impl Provider,
) -> Result<(String, BTreeMap<String, Change>), EngineError> {
    let mut cursor = start.to_string();
    let mut seen = HashSet::new();
    let mut events = BTreeMap::new();
    let mut count = 0usize;
    for _ in 0..10_000 {
        cancelled(cancel)?;
        if !crate::remote::valid_change_token(&cursor) || !seen.insert(cursor.clone()) {
            return Err(invalid());
        }
        let page = source.page(&cursor)?;
        cancelled(cancel)?;
        if page.next.is_some() == page.terminal.is_some() {
            return Err(invalid());
        }
        for event in page.changes {
            count += 1;
            if count > 100_000 {
                return Err(EngineError::InventoryLimit);
            }
            if !crate::configuration::valid_id(&event.id)
                || event.item.as_ref().is_some_and(|item| item.id != event.id)
            {
                return Err(invalid());
            }
            events.insert(event.id.clone(), event);
        }
        match (page.next, page.terminal) {
            (Some(next), None) => cursor = next,
            (None, Some(terminal)) if crate::remote::valid_change_token(&terminal) => {
                return Ok((terminal, events))
            }
            _ => return Err(invalid()),
        }
    }
    Err(EngineError::InventoryLimit)
}

/// A changed folder location/type may expose descendants without leaf events.
/// Request reconciliation rather than guessing that the graph is complete.
fn apply(
    inventory: &RemoteInventory,
    events: &BTreeMap<String, Change>,
    cancel: &Cancellation,
) -> Result<Option<RemoteInventory>, EngineError> {
    let mut entries: BTreeMap<_, _> = inventory
        .entries
        .iter()
        .map(|item| (item.id.clone(), item.clone()))
        .collect();
    let mut unavailable = HashSet::new();
    for change in events.values() {
        cancelled(cancel)?;
        if change.id == inventory.root_id {
            if change.removed {
                return Err(EngineError::RemoteFolderUnavailable);
            }
            continue;
        }
        let old = entries.get(&change.id);
        if let Some(item) = &change.item {
            let in_tree = item.parent_id == inventory.root_id
                || entries
                    .get(&item.parent_id)
                    .is_some_and(|p| p.kind == InventoryKind::Folder);
            let folder_changed = item.kind == InventoryKind::Folder
                && in_tree
                && old.is_none_or(|p| {
                    p.kind != item.kind || p.parent_id != item.parent_id || p.name != item.name
                });
            let old_folder_changed = old.is_some_and(|p| {
                p.kind == InventoryKind::Folder
                    && (p.kind != item.kind || p.parent_id != item.parent_id || p.name != item.name)
            });
            if folder_changed || old_folder_changed {
                return Ok(None);
            }
            if in_tree {
                if old.is_some_and(|p| p.version.zip(item.version).is_some_and(|(a, b)| b < a)) {
                    return Err(EngineError::RemoteChanged);
                }
                entries.insert(item.id.clone(), item.clone());
                if entries.len() > 100_000 {
                    return Err(EngineError::InventoryLimit);
                }
                continue;
            }
        }
        entries.remove(&change.id);
        unavailable.insert(change.id.clone());
    }
    // Prune unavailable subtrees once per batch, rather than walking the entire
    // tree for every unrelated account event. No local filesystem effect occurs.
    if !unavailable.is_empty() {
        let mut children: HashMap<String, Vec<String>> = HashMap::new();
        for entry in entries.values() {
            children
                .entry(entry.parent_id.clone())
                .or_default()
                .push(entry.id.clone());
        }
        let mut queue: VecDeque<_> = unavailable.iter().cloned().collect();
        while let Some(parent) = queue.pop_front() {
            cancelled(cancel)?;
            for child in children.get(&parent).into_iter().flatten() {
                if unavailable.insert(child.clone()) {
                    queue.push_back(child.clone());
                }
            }
        }
        entries.retain(|id, _| !unavailable.contains(id));
    }
    let result = RemoteInventory {
        account_id: inventory.account_id.clone(),
        root_id: inventory.root_id.clone(),
        entries: entries.into_values().collect(),
    };
    validate(&result.account_id, &result.root_id, &result)?;
    Ok(Some(result))
}

/// Bounds and validates the ID graph; ambiguity in names is left to the planner.
pub(crate) fn validate(
    account: &AccountId,
    root: &str,
    inventory: &RemoteInventory,
) -> Result<(), EngineError> {
    if &inventory.account_id != account
        || inventory.root_id != root
        || inventory.entries.len() > 100_000
        || !crate::configuration::valid_id(root)
    {
        return Err(EngineError::InvalidPlan);
    }
    let mut ids = HashSet::from([root]);
    for item in &inventory.entries {
        if !crate::configuration::valid_id(&item.id) || !ids.insert(&item.id) {
            return Err(EngineError::InvalidPlan);
        }
    }
    // Planner safely blocks disconnected/cyclic paths and unsafe names; do not
    // convert those unknown observations into authorizations for writes.
    Ok(())
}
fn paths(
    inventory: &RemoteInventory,
) -> Result<BTreeMap<String, Option<std::path::PathBuf>>, EngineError> {
    let plan = crate::planner::plan(inventory, std::path::Path::new("/"), |_| {
        crate::LocalObservation::Unknown
    })?;
    Ok(plan
        .entries
        .into_iter()
        .map(|entry| (entry.remote_id, entry.relative_path))
        .collect())
}
/// Projects durable path observations as information, never as download errors.
/// Missing graph entries are unavailable observations, not deletion evidence.
pub(crate) fn previous_paths(
    checkpoint: &Checkpoint,
) -> Result<Vec<crate::PreviousPath>, EngineError> {
    if checkpoint.notices.is_empty() {
        return Ok(Vec::new());
    }
    let current = paths(&checkpoint.inventory)?;
    Ok(checkpoint
        .notices
        .iter()
        .map(|notice| crate::PreviousPath {
            previous_path: notice.path.clone(),
            current_path: current.get(&notice.id).cloned().flatten(),
        })
        .collect())
}
fn reconcile_notices(
    old: &RemoteInventory,
    new: &RemoteInventory,
    notices: &mut Vec<Notice>,
) -> Result<(), EngineError> {
    let before = paths(old)?;
    let after = paths(new)?;
    let mut known: HashSet<_> = notices
        .iter()
        .map(|n| (n.id.clone(), n.path.clone()))
        .collect();
    for (id, path) in before {
        if after.get(&id) != Some(&path) && known.insert((id.clone(), path.clone())) {
            notices.push(Notice { id, path });
        }
    }
    notices.retain(|n| after.get(&n.id) != Some(&n.path));
    notices.sort_by(|a, b| (&a.id, &a.path).cmp(&(&b.id, &b.path)));
    if notices.len() > 100_000 {
        return Err(EngineError::InventoryLimit);
    }
    Ok(())
}
