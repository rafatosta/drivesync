//! Strong content equivalence is evidence about bytes, never object identity.
use crate::{
    download::Source, Cancellation, ConfirmedBase, ConflictReason, DownloadPlan, EngineError,
    LocalObservation, PlanAction, RemoteInventory,
};
use std::collections::HashMap;

/// Refines only concurrent content changes with continuous local identity and B.
/// An item read failure leaves it unknown; cancellation and root suspension stop preparation.
pub(crate) fn refine(
    plan: &mut DownloadPlan,
    inventory: &RemoteInventory,
    bases: &HashMap<String, ConfirmedBase>,
    source: &mut impl Source,
    cancel: &Cancellation,
) -> Result<(), EngineError> {
    for (entry, item) in plan.entries.iter_mut().zip(&inventory.entries) {
        crate::local::check(cancel)?;
        if entry.action != PlanAction::Blocked(ConflictReason::ConcurrentChanges) {
            continue;
        }
        let LocalObservation::Verified(fingerprint) = &entry.expected_local else {
            continue;
        };
        let local_sha = fingerprint
            .split(':')
            .next()
            .ok_or(EngineError::InvalidPlan)?;
        let Some((base_sha, _, _)) = bases
            .get(&entry.remote_id)
            .and_then(|base| crate::planner::fingerprint_parts(&base.local_fingerprint))
        else {
            entry.action = PlanAction::DeferredUnknown;
            continue;
        };
        match crate::download::content_fingerprint(item, source, cancel) {
            Ok(remote_sha) => {
                let comparison = if remote_sha.eq_ignore_ascii_case(local_sha) {
                    crate::ContentComparison::Equal
                } else {
                    crate::ContentComparison::Different
                };
                let remote_change = if remote_sha.eq_ignore_ascii_case(base_sha) {
                    crate::ContentComparison::Equal
                } else {
                    crate::ContentComparison::Different
                };
                entry.action = crate::conflicts::refine_content(remote_change, comparison);
            }
            Err(EngineError::Authentication(crate::AuthError::Cancelled)) => {
                return Err(EngineError::Authentication(crate::AuthError::Cancelled))
            }
            Err(
                error @ (EngineError::InvalidPlan
                | EngineError::StateStorage
                | EngineError::RootSuspended(_)),
            ) => return Err(error),
            Err(_) => {
                entry.action = crate::conflicts::refine_content(
                    crate::ContentComparison::Unknown,
                    crate::ContentComparison::Unknown,
                )
            }
        }
    }
    Ok(())
}
/// Builds a complete plan and optional equivalence evidence from a finished scan.
/// Revalidates local evidence for equivalent, divergent and location decisions
/// after remote reads, using the already locked root. A changed or unreadable
/// file defers its diagnosis; cancellation or root suspension aborts preparation.
/// This does not persist observations, install content or confirm a base.
/// Later edits still require fresh execution/resolution checks.
pub(crate) fn prepare(
    inventory: &RemoteInventory,
    root: &crate::download::filesystem::Root,
    bases: &HashMap<String, ConfirmedBase>,
    local: &crate::local::Inventory,
    source: &mut impl Source,
    cancel: &Cancellation,
) -> Result<DownloadPlan, EngineError> {
    let mut plan = crate::planner::plan_with_bases(inventory, root.path(), bases, |_, path, _| {
        Ok(local.observe(path))
    })?;
    super::locations::refine_locations(&mut plan, inventory, bases, local, source, cancel)?;
    refine(&mut plan, inventory, bases, source, cancel)?;
    // Network reads may outlive the scan. Never publish stale content evidence.
    root.validate()?;
    for entry in &mut plan.entries {
        let path = match &entry.action {
            PlanAction::Classified { local_path, .. } => Some(local_path),
            PlanAction::Equivalent | PlanAction::Blocked(ConflictReason::ConcurrentChanges) => {
                entry.relative_path.as_ref()
            }
            _ => None,
        };
        let Some(path) = path else { continue };
        crate::local::check(cancel)?;
        let valid = match local.entries.get(path) {
            Some(crate::local::Evidence::Stable { fingerprint, stamp }) => {
                crate::local::stamp(root, path).as_ref() == Ok(stamp)
                    && crate::download::filesystem::fingerprint_root(root, path, cancel).as_ref()
                        == Ok(&Some(fingerprint.clone()))
                    && crate::local::stamp(root, path).as_ref() == Ok(stamp)
            }
            _ => false,
        };
        crate::local::check(cancel)?;
        root.validate()?;
        if !valid {
            entry.action = PlanAction::DeferredUnknown;
        }
    }
    Ok(plan)
}
