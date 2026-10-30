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
        match crate::download::content_fingerprint(item, source, cancel) {
            Ok(remote_sha) => {
                let comparison = if remote_sha.eq_ignore_ascii_case(local_sha) {
                    crate::ContentComparison::Equal
                } else {
                    crate::ContentComparison::Different
                };
                entry.action = crate::conflicts::refine_content(comparison);
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
                entry.action = crate::conflicts::refine_content(crate::ContentComparison::Unknown)
            }
        }
    }
    Ok(())
}
/// Builds a complete plan and optional equivalence evidence from a finished scan.
/// This does not persist observations, execute a transfer or confirm a base.
pub(crate) fn prepare(
    inventory: &RemoteInventory,
    root: &std::path::Path,
    bases: &HashMap<String, ConfirmedBase>,
    local: &crate::local::Inventory,
    source: &mut impl Source,
    cancel: &Cancellation,
) -> Result<DownloadPlan, EngineError> {
    let mut plan = crate::planner::plan_with_bases(inventory, root, bases, |_, path, _| {
        Ok(local.observe(path))
    })?;
    super::locations::refine_locations(&mut plan, inventory, bases, local, source, cancel)?;
    refine(&mut plan, inventory, source, cancel)?;
    Ok(plan)
}
