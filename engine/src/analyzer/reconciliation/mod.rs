//! Deterministic three-way decisions for one inventory item, without I/O.
//!
//! Decisions describe intent, never executable Operator commands. Materializing a
//! command requires destination mapping, capabilities and atomic preconditions,
//! which are unavailable in the current engine. Unknown evidence stays explicit.
mod models;
#[cfg(test)]
mod tests;

use crate::inventory::{BaselineParent, ItemKind, PresenceState, SyncItem};
use crate::models::{LocalEntryKind, MetadataValue, RemoteEntryKind, RemoteFileRef};
pub(super) use models::*;

/// Compares a single snapshot in fixed presence/content/name/location order.
///
/// Rejects structural contradictions before producing decisions. Caller-supplied
/// content evidence must describe this revision and the confirmed baseline;
/// this pure function cannot prove that an external measurement is trustworthy.
/// Retained snapshots do not override unknown fields in the latest collection.
pub(super) fn reconcile(
    request: ReconciliationRequest,
) -> Result<ReconciliationAssessment, ReconciliationError> {
    validate(&request)?;
    let item = &request.item;
    let mut out = ReconciliationAssessment {
        item_id: item.id.clone(),
        item_revision: item.revision,
        decisions: vec![],
        divergences: vec![],
        requirements: vec![],
    };
    if !matches!(item.kind, ItemKind::File | ItemKind::Directory) {
        out.require(Requirement::SupportedKind);
        return Ok(out);
    }
    for (side, presence, coverage) in [
        (
            Side::Local,
            &item.local_presence,
            request.evidence.local_observation,
        ),
        (
            Side::Remote,
            &item.remote_presence,
            request.evidence.remote_observation,
        ),
    ] {
        if *presence == PresenceState::Unknown || coverage != ObservationConfidence::Complete {
            out.require(Requirement::Observe(side));
        }
    }
    if item.local_presence == PresenceState::Present {
        let local = item.local_latest.as_ref().or(item.local.as_ref()).unwrap();
        if !matches!(
            local.kind,
            MetadataValue::Known(LocalEntryKind::Regular | LocalEntryKind::Directory)
        ) {
            out.require(Requirement::Observe(Side::Local));
        }
    }
    if item.remote_presence == PresenceState::Present {
        let remote = item
            .remote_latest
            .as_ref()
            .or(item.remote.as_ref())
            .unwrap();
        if !matches!(
            remote.kind,
            MetadataValue::Known(RemoteEntryKind::File | RemoteEntryKind::Directory)
        ) || !matches!(remote.trashed, MetadataValue::Known(false))
        {
            out.require(Requirement::Observe(Side::Remote));
        }
    }
    if !out.requirements.is_empty() {
        identify_initial_direction(item, &mut out);
        return Ok(out);
    }
    match (&item.local_presence, &item.remote_presence) {
        (PresenceState::AbsentConfirmed, PresenceState::AbsentConfirmed) => return Ok(out),
        (PresenceState::Present, PresenceState::Present) => {}
        _ => {
            reconcile_presence(&request, &mut out);
            return Ok(out);
        }
    }
    let Some(base) = &item.baseline else {
        out.divergences.push(Divergence {
            dimension: Dimension::Presence,
            reason: DivergenceReason::MissingBaseline,
        });
        out.require(Requirement::VerifyIdentity);
        if item.kind == ItemKind::File && request.evidence.content_comparison != Comparison::Equal {
            out.require(Requirement::VerifyContent);
        }
        return Ok(out);
    };
    if base.local.presence != PresenceState::Present
        || base.remote.presence != PresenceState::Present
    {
        out.divergences.push(Divergence {
            dimension: Dimension::Presence,
            reason: DivergenceReason::IncompatibleStates,
        });
        out.require(Requirement::VerifyBaseline);
        return Ok(out);
    }
    if item.kind == ItemKind::File {
        compare_dimension(
            &mut out,
            Dimension::Content,
            request.evidence.local_content,
            request.evidence.remote_content,
            request.evidence.content_comparison,
        );
    }
    let local = item.local_latest.as_ref().or(item.local.as_ref()).unwrap();
    let remote = item
        .remote_latest
        .as_ref()
        .or(item.remote.as_ref())
        .unwrap();
    let local_name = local
        .relative_path
        .as_bytes()
        .rsplit(|b| *b == b'/')
        .next()
        .unwrap();
    let remote_name = known(&remote.name).map(|s| s.as_bytes());
    compare_dimension(
        &mut out,
        Dimension::Name,
        delta(Some(local_name), base.local.local_name_bytes.as_deref()),
        delta(remote_name, base.remote.name.as_deref().map(str::as_bytes)),
        comparison(Some(local_name), remote_name),
    );
    let remote_parent = known(&remote.parent_ids)
        .and_then(|ids| {
            (ids.len() == 1).then(|| RemoteFileRef {
                account_id: remote.reference.account_id.clone(),
                file_id: ids[0].clone(),
            })
        })
        .map(BaselineParent::Remote);
    let local_parent = request
        .evidence
        .local_parent
        .clone()
        .map(BaselineParent::Local);
    compare_dimension(
        &mut out,
        Dimension::Location,
        // A root-level path has a known root parent, represented by None.
        // For nested paths, None still means that parent mapping is missing.
        if !local.relative_path.as_bytes().contains(&b'/') {
            if base.local.parent_identity.is_none() {
                Change::Unchanged
            } else {
                Change::Changed
            }
        } else if local_parent.is_some() && base.local.parent_identity.is_none() {
            Change::Changed
        } else {
            delta(local_parent.as_ref(), base.local.parent_identity.as_ref())
        },
        delta(remote_parent.as_ref(), base.remote.parent_identity.as_ref()),
        request.evidence.parent_comparison,
    );
    Ok(out)
}

/// A first observation can identify a replication direction before a destination
/// is verified. Preserve unknown presence and require identity/destination checks:
/// this is a blocked intent, never proof that the opposite resource is absent.
/// Existing bindings, retained snapshots and baselines disallow this shortcut.
fn identify_initial_direction(item: &SyncItem, out: &mut ReconciliationAssessment) {
    if item.baseline.is_some() {
        return;
    }
    let source = if item.local_presence == PresenceState::Present
        && item.remote_presence == PresenceState::Unknown
        && item.remote_ref.is_none()
        && item.remote.is_none()
        && item.remote_latest.is_none()
        && out.requirements == [Requirement::Observe(Side::Remote)]
    {
        Side::Local
    } else if item.remote_presence == PresenceState::Present
        && item.local_presence == PresenceState::Unknown
        && item.local_file_id.is_none()
        && item.local.is_none()
        && item.local_latest.is_none()
        && out.requirements == [Requirement::Observe(Side::Local)]
    {
        Side::Remote
    } else {
        return;
    };
    out.decisions.push(Decision::Create {
        source,
        kind: item.kind.clone(),
    });
    out.require(Requirement::VerifyIdentity);
    out.require(Requirement::ResolveDestination);
}

fn reconcile_presence(request: &ReconciliationRequest, out: &mut ReconciliationAssessment) {
    let item = &request.item;
    let source = if item.local_presence == PresenceState::Present {
        Side::Local
    } else {
        Side::Remote
    };
    let Some(base) = &item.baseline else {
        out.decisions.push(Decision::Create {
            source,
            kind: item.kind.clone(),
        });
        out.require(Requirement::ResolveDestination);
        return;
    };
    if base.local.presence == PresenceState::AbsentConfirmed
        && base.remote.presence == PresenceState::AbsentConfirmed
    {
        out.decisions.push(Decision::Create {
            source,
            kind: item.kind.clone(),
        });
        out.require(Requirement::ResolveDestination);
        return;
    }
    if base.local.presence != PresenceState::Present
        || base.remote.presence != PresenceState::Present
    {
        out.require(Requirement::VerifyBaseline);
        return;
    }
    let (content, name, parent) = match source {
        Side::Local => {
            let file = item.local_latest.as_ref().or(item.local.as_ref()).unwrap();
            let name = file.relative_path.as_bytes().rsplit(|b| *b == b'/').next();
            (
                request.evidence.local_content,
                delta(name, base.local.local_name_bytes.as_deref()),
                delta(
                    request
                        .evidence
                        .local_parent
                        .clone()
                        .map(BaselineParent::Local)
                        .as_ref(),
                    base.local.parent_identity.as_ref(),
                ),
            )
        }
        Side::Remote => {
            let file = item
                .remote_latest
                .as_ref()
                .or(item.remote.as_ref())
                .unwrap();
            let parent = known(&file.parent_ids).and_then(|ids| {
                (ids.len() == 1).then(|| {
                    BaselineParent::Remote(RemoteFileRef {
                        account_id: file.reference.account_id.clone(),
                        file_id: ids[0].clone(),
                    })
                })
            });
            (
                request.evidence.remote_content,
                delta(known(&file.name), base.remote.name.as_ref()),
                delta(parent.as_ref(), base.remote.parent_identity.as_ref()),
            )
        }
    };
    let changes = [
        if item.kind == ItemKind::Directory {
            Change::Unchanged
        } else {
            content
        },
        name,
        parent,
    ];
    if changes.contains(&Change::Changed) {
        out.divergences.push(Divergence {
            dimension: Dimension::Presence,
            reason: DivergenceReason::DeleteVsChange,
        });
    } else if changes.contains(&Change::Unknown) {
        out.require(Requirement::VerifySurvivingState);
    } else {
        // Remote absence may be propagated to the local trash; the reverse
        // direction remains blocked on explicit deletion authorization.
        out.decisions.push(Decision::Trash { target: source });
        if source == Side::Remote {
            out.require(Requirement::AuthorizeDeletion);
        }
    }
}

fn compare_dimension(
    out: &mut ReconciliationAssessment,
    dimension: Dimension,
    local: Change,
    remote: Change,
    current: Comparison,
) {
    if current == Comparison::Equal {
        return;
    }
    match (local, remote) {
        (Change::Unchanged, Change::Unchanged) => {
            if current == Comparison::Different {
                out.divergences.push(Divergence {
                    dimension,
                    reason: DivergenceReason::InconsistentEvidence,
                });
            }
        }
        (Change::Changed, Change::Unchanged) => out.decisions.push(Decision::Propagate {
            dimension,
            source: Side::Local,
        }),
        (Change::Unchanged, Change::Changed) => out.decisions.push(Decision::Propagate {
            dimension,
            source: Side::Remote,
        }),
        (Change::Changed, Change::Changed) if current == Comparison::Different => {
            out.divergences.push(Divergence {
                dimension,
                reason: DivergenceReason::ConcurrentChanges,
            })
        }
        _ => out.require(Requirement::Compare(dimension)),
    }
}

fn known<T>(value: &MetadataValue<T>) -> Option<&T> {
    if let MetadataValue::Known(value) = value {
        Some(value)
    } else {
        None
    }
}
fn delta<T: PartialEq + ?Sized>(current: Option<&T>, baseline: Option<&T>) -> Change {
    match (current, baseline) {
        (Some(a), Some(b)) if a == b => Change::Unchanged,
        (Some(_), Some(_)) => Change::Changed,
        _ => Change::Unknown,
    }
}
fn comparison<T: PartialEq + ?Sized>(a: Option<&T>, b: Option<&T>) -> Comparison {
    match (a, b) {
        (Some(a), Some(b)) if a == b => Comparison::Equal,
        (Some(_), Some(_)) => Comparison::Different,
        _ => Comparison::Unknown,
    }
}

fn validate(request: &ReconciliationRequest) -> Result<(), ReconciliationError> {
    let item: &SyncItem = &request.item;
    if item.id.0.trim().is_empty() || item.revision == 0 {
        return Err(ReconciliationError::InvalidRequest);
    }
    for file in [item.local.as_ref(), item.local_latest.as_ref()]
        .into_iter()
        .flatten()
    {
        file.validate()
            .map_err(|_| ReconciliationError::InconsistentInput)?;
        if file
            .local_id
            .as_ref()
            .is_some_and(|id| Some(id) != item.local_file_id.as_ref())
        {
            return Err(ReconciliationError::InvalidItemIdentity);
        }
        if let Some(kind) = known(&file.kind) {
            if !matches!(
                (&item.kind, kind),
                (ItemKind::File, LocalEntryKind::Regular)
                    | (ItemKind::Directory, LocalEntryKind::Directory)
                    | (ItemKind::Symlink, LocalEntryKind::Symlink)
                    | (ItemKind::Other, _)
                    | (_, LocalEntryKind::Unknown)
            ) {
                return Err(ReconciliationError::InconsistentInput);
            }
        }
    }
    for file in [item.remote.as_ref(), item.remote_latest.as_ref()]
        .into_iter()
        .flatten()
    {
        file.validate()
            .map_err(|_| ReconciliationError::InconsistentInput)?;
        if Some(&file.reference) != item.remote_ref.as_ref() {
            return Err(ReconciliationError::InvalidItemIdentity);
        }
        if let Some(kind) = known(&file.kind) {
            if !matches!(
                (&item.kind, kind),
                (ItemKind::File, RemoteEntryKind::File)
                    | (ItemKind::Directory, RemoteEntryKind::Directory)
                    | (ItemKind::Other, _)
            ) {
                return Err(ReconciliationError::InconsistentInput);
            }
        }
    }
    if item.local_presence == PresenceState::Present
        && item.local_latest.as_ref().or(item.local.as_ref()).is_none()
        || item.remote_presence == PresenceState::Present
            && item
                .remote_latest
                .as_ref()
                .or(item.remote.as_ref())
                .is_none()
    {
        return Err(ReconciliationError::InconsistentInput);
    }
    if item
        .local_file_id
        .as_ref()
        .is_some_and(|id| id.0.trim().is_empty())
        || item
            .remote_ref
            .as_ref()
            .is_some_and(|id| id.account_id.0.trim().is_empty() || id.file_id.0.trim().is_empty())
        || request
            .evidence
            .local_parent
            .as_ref()
            .is_some_and(|id| id.0.trim().is_empty())
    {
        return Err(ReconciliationError::InvalidItemIdentity);
    }
    if let (Some(retained), Some(latest)) = (&item.remote, &item.remote_latest) {
        if retained.provider != latest.provider {
            return Err(ReconciliationError::InvalidItemIdentity);
        }
    }
    if request.evidence.content_comparison == Comparison::Different
        && request.evidence.local_content == Change::Unchanged
        && request.evidence.remote_content == Change::Unchanged
    {
        return Err(ReconciliationError::InvalidEvidence);
    }
    if let Some(base) = &item.baseline {
        if matches!(base.local.parent_identity, Some(BaselineParent::Remote(_)))
            || matches!(base.remote.parent_identity, Some(BaselineParent::Local(_)))
        {
            return Err(ReconciliationError::InconsistentInput);
        }
        if let (Some(BaselineParent::Remote(parent)), Some(resource)) =
            (&base.remote.parent_identity, &item.remote_ref)
        {
            if parent.account_id != resource.account_id {
                return Err(ReconciliationError::InvalidItemIdentity);
            }
        }
    } else if request.evidence.local_content != Change::Unknown
        || request.evidence.remote_content != Change::Unknown
    {
        return Err(ReconciliationError::InvalidEvidence);
    }
    if request.evidence.content_comparison != Comparison::Unknown
        && (item.kind != ItemKind::File
            || item.local_presence != PresenceState::Present
            || item.remote_presence != PresenceState::Present)
    {
        return Err(ReconciliationError::InvalidEvidence);
    }
    Ok(())
}
