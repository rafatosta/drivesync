//! Evidence adapter for location/content combinations, without applying renames.
use crate::{
    conflicts::classify_differences, download::Source, local::Inventory, AttributeChange as A,
    BaseEvidence, Cancellation, ConfirmedBase, ContentComparison as C, DifferenceInput,
    DifferenceKind, DownloadPlan, EngineError, InventoryKind, LocalObservation, PlanAction,
    RemoteInventory, SideDifference,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

fn attribute(before: &str, after: &str) -> A {
    if before == after {
        A::Unchanged
    } else {
        A::Changed(after.into())
    }
}
/// Finds a unique unchanged-content relocation only after the old path is absent.
/// A replaced old path, inode-only match, or multiple aliases cannot prove a move.
fn locate(
    base: &ConfirmedBase,
    local: &Inventory,
    relocated: &HashMap<String, Option<PathBuf>>,
) -> Option<(PathBuf, String)> {
    let old = &base.location.as_ref()?.relative_path;
    let (_, bd, bi) = crate::planner::fingerprint_parts(&base.local_fingerprint)?;
    match local.observe(old) {
        LocalObservation::Verified(fingerprint) => {
            let (_, dev, ino) = crate::planner::fingerprint_parts(&fingerprint)?;
            ((dev, ino) == (bd, bi)).then(|| (old.clone(), fingerprint))
        }
        LocalObservation::Absent => relocated
            .get(&base.local_fingerprint.to_ascii_lowercase())?
            .as_ref()
            .map(|path| (path.clone(), base.local_fingerprint.clone())),
        _ => None,
    }
}
fn local_parent(old: &Path, current: &Path, paths: &HashMap<PathBuf, String>, root: &str) -> A {
    if old.parent() == current.parent() {
        return A::Unchanged;
    }
    match current.parent() {
        Some(parent) if parent.as_os_str().is_empty() => A::Changed(root.into()),
        Some(parent) => paths
            .get(parent)
            .map_or(A::Unknown, |id| A::Changed(id.clone())),
        None => A::Unknown,
    }
}
/// Refines only observed local changes involving location. Remote-only moves keep
/// their existing download-only behavior. Every classified result remains pending:
/// no rename, remote update, duplicate download or baseline advancement is enabled.
pub(crate) fn refine_locations(
    plan: &mut DownloadPlan,
    inventory: &RemoteInventory,
    bases: &HashMap<String, ConfirmedBase>,
    local: &Inventory,
    source: &mut impl Source,
    cancel: &Cancellation,
) -> Result<(), EngineError> {
    let mut relocated: HashMap<String, Option<PathBuf>> = HashMap::new();
    for (path, evidence) in &local.entries {
        if let crate::local::Evidence::Stable { fingerprint, .. } = evidence {
            relocated
                .entry(fingerprint.to_ascii_lowercase())
                .and_modify(|candidate| *candidate = None)
                .or_insert_with(|| Some(path.clone()));
        }
    }
    let paths: HashMap<_, _> = inventory
        .entries
        .iter()
        .zip(&plan.entries)
        .filter(|(item, _)| item.kind == InventoryKind::Folder)
        .filter_map(|(item, entry)| entry.relative_path.clone().map(|p| (p, item.id.clone())))
        .collect();
    for (entry, item) in plan.entries.iter_mut().zip(&inventory.entries) {
        crate::local::check(cancel)?;
        if item.kind != InventoryKind::Binary || entry.relative_path.is_none() {
            continue;
        }
        let Some(base) = bases.get(&item.id) else {
            continue;
        };
        if base.remote_version == 0 {
            entry.action = PlanAction::DeferredUnknown;
            continue;
        }
        let Some(location) = &base.location else {
            continue;
        };
        let Some((path, fingerprint)) = locate(base, local, &relocated) else {
            continue;
        };
        let Some((base_hash, _, _)) = crate::planner::fingerprint_parts(&base.local_fingerprint)
        else {
            continue;
        };
        let Some((local_hash, _, _)) = crate::planner::fingerprint_parts(&fingerprint) else {
            continue;
        };
        let local_content = if local_hash.eq_ignore_ascii_case(base_hash) {
            C::Equal
        } else {
            C::Different
        };
        let local_location_changed = path != location.relative_path;
        let remote_location_changed =
            item.name != location.remote_name || item.parent_id != location.remote_parent;
        if !local_location_changed && !(remote_location_changed && local_content == C::Different) {
            continue;
        }
        // Destination collisions remain structural blockers, never reconcilable.
        if let LocalObservation::Verified(target) = &entry.expected_local {
            if crate::planner::fingerprint_parts(target).map(|(_, d, i)| (d, i))
                != crate::planner::fingerprint_parts(&fingerprint).map(|(_, d, i)| (d, i))
            {
                continue;
            }
        } else if !matches!(entry.expected_local, LocalObservation::Absent) {
            continue;
        }
        if item.version.is_none_or(|v| v < base.remote_version)
            || (remote_location_changed && item.version == Some(base.remote_version))
        {
            entry.action = PlanAction::DeferredUnknown;
            continue;
        }
        let remote_hash = if item.version == Some(base.remote_version) {
            base_hash.to_owned()
        } else {
            match crate::download::content_fingerprint(item, source, cancel) {
                Ok(hash) => hash,
                Err(
                    error @ (EngineError::InvalidPlan
                    | EngineError::StateStorage
                    | EngineError::RootSuspended(_)
                    | EngineError::Authentication(crate::AuthError::Cancelled)),
                ) => return Err(error),
                Err(_) => {
                    entry.action = PlanAction::DeferredUnknown;
                    continue;
                }
            }
        };
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map_or(A::Unknown, |name| attribute(&location.remote_name, name));
        let input = DifferenceInput {
            base: BaseEvidence::Confirmed,
            local: SideDifference::Present {
                content: local_content,
                name,
                parent: local_parent(&location.relative_path, &path, &paths, &inventory.root_id),
            },
            remote: SideDifference::Present {
                content: if remote_hash.eq_ignore_ascii_case(base_hash) {
                    C::Equal
                } else {
                    C::Different
                },
                name: attribute(&location.remote_name, &item.name),
                parent: attribute(&location.remote_parent, &item.parent_id),
            },
            current_content: if remote_hash.eq_ignore_ascii_case(local_hash) {
                C::Equal
            } else {
                C::Different
            },
        };
        let decision = classify_differences(&input);
        entry.action = if decision.kind == DifferenceKind::Unknown {
            PlanAction::DeferredUnknown
        } else {
            PlanAction::Classified {
                decision,
                local_path: path,
            }
        };
    }
    Ok(())
}
