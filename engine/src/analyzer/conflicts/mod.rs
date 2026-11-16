//! Independent, deterministic identification of incompatible concurrent changes.
//!
//! No commands, resolution options, automatic choices, persistence or I/O are
//! produced. The Analyzer will own any future interaction and action policy.
mod models;
#[cfg(test)]
mod tests;
use crate::inventory::{BaselineParent, ItemKind, PresenceState};
use crate::models::{LocalEntryKind, MetadataValue, RemoteEntryKind, RemoteFileRef};
pub(super) use models::*;
use EvidenceComparison::{Different, Equal, Unknown};

/// Classifies one item using only supplied snapshots and explicit evidence.
///
/// A proven divergence wins over unknown dimensions; requirements remain visible.
/// Otherwise any decisive unknown yields Undetermined. Field order is stable:
/// presence, content (files only), name, location. Retained snapshots of absent
/// resources are historical and never used as current evidence.
pub(super) fn evaluate(request: ConflictRequest) -> Result<ConflictAssessment, ConflictError> {
    validate(&request)?;
    let mut out = ConflictAssessment {
        item_id: request.item.id.clone(),
        item_revision: request.item.revision,
        presence: ConflictPresence::NoConflict,
        findings: vec![],
        requirements: vec![],
    };
    identify(&request, &mut out);
    out.presence = if out
        .findings
        .iter()
        .any(|f| f.status == FindingStatus::Divergent)
    {
        ConflictPresence::Conflict
    } else if out
        .findings
        .iter()
        .any(|f| f.status == FindingStatus::Unknown)
    {
        ConflictPresence::Undetermined
    } else {
        ConflictPresence::NoConflict
    };
    Ok(out)
}

fn identify(request: &ConflictRequest, out: &mut ConflictAssessment) {
    let item = &request.item;
    let evidence = &request.evidence;
    let trusted = item
        .baseline
        .as_ref()
        .filter(|_| evidence.baseline_confirmed);
    for (presence, complete, requirement) in [
        (
            &item.local_presence,
            evidence.local_scope_complete,
            EvidenceRequirement::ObserveLocal,
        ),
        (
            &item.remote_presence,
            evidence.remote_scope_complete,
            EvidenceRequirement::ObserveRemote,
        ),
    ] {
        if *presence == PresenceState::Unknown || !complete {
            out.require(requirement);
        }
    }
    if !out.requirements.is_empty() {
        finding(
            out,
            ConflictDimension::Presence,
            FindingStatus::Unknown,
            FindingReason::InsufficientEvidence,
        );
        return;
    }
    if item.local_presence == PresenceState::AbsentConfirmed
        && item.remote_presence == PresenceState::AbsentConfirmed
    {
        finding(
            out,
            ConflictDimension::Presence,
            FindingStatus::Compatible,
            FindingReason::Equivalent,
        );
        return;
    }
    if !matches!(item.kind, ItemKind::File | ItemKind::Directory) {
        out.require(EvidenceRequirement::VerifyKind);
        finding(
            out,
            ConflictDimension::Presence,
            FindingStatus::Unknown,
            FindingReason::UnsupportedKind,
        );
        return;
    }
    let local = item.local_latest.as_ref().or(item.local.as_ref());
    let remote = item.remote_latest.as_ref().or(item.remote.as_ref());
    if item.local_presence == PresenceState::Present
        && !matches!(
            local.unwrap().kind,
            MetadataValue::Known(LocalEntryKind::Regular | LocalEntryKind::Directory)
        )
    {
        out.require(EvidenceRequirement::ObserveLocal);
    }
    if item.remote_presence == PresenceState::Present
        && (!matches!(
            remote.unwrap().kind,
            MetadataValue::Known(RemoteEntryKind::File | RemoteEntryKind::Directory)
        ) || !matches!(remote.unwrap().trashed, MetadataValue::Known(false)))
    {
        out.require(EvidenceRequirement::ObserveRemote);
    }
    if !out.requirements.is_empty() {
        finding(
            out,
            ConflictDimension::Presence,
            FindingStatus::Unknown,
            FindingReason::InsufficientEvidence,
        );
        return;
    }
    if item.local_presence != item.remote_presence {
        let source_local = item.local_presence == PresenceState::Present;
        let Some(base) = trusted else {
            if item.baseline.is_none() {
                finding(
                    out,
                    ConflictDimension::Presence,
                    FindingStatus::Compatible,
                    FindingReason::NewItem,
                );
            } else {
                out.require(EvidenceRequirement::VerifyBaseline);
                finding(
                    out,
                    ConflictDimension::Presence,
                    FindingStatus::Unknown,
                    FindingReason::MissingBaseline,
                );
            }
            return;
        };
        if base.local.presence == PresenceState::AbsentConfirmed
            && base.remote.presence == PresenceState::AbsentConfirmed
        {
            finding(
                out,
                ConflictDimension::Presence,
                FindingStatus::Compatible,
                FindingReason::NewItem,
            );
            return;
        }
        if base.local.presence != PresenceState::Present
            || base.remote.presence != PresenceState::Present
        {
            out.require(EvidenceRequirement::VerifyBaseline);
            finding(
                out,
                ConflictDimension::Presence,
                FindingStatus::Unknown,
                FindingReason::MissingBaseline,
            );
            return;
        }
        let (content, name, parent) = if source_local {
            let file = local.unwrap();
            (
                evidence.content.local_to_baseline,
                compare(
                    Some(local_name(file)),
                    base.local.local_name_bytes.as_deref(),
                ),
                local_parent_comparison(file, evidence.local_parent.as_ref(), Some(&base.local)),
            )
        } else {
            let file = remote.unwrap();
            (
                evidence.content.remote_to_baseline,
                compare(known(&file.name), base.remote.name.as_ref()),
                compare(
                    remote_parent(file).as_ref(),
                    base.remote.parent_identity.as_ref(),
                ),
            )
        };
        let comparisons = [
            if item.kind == ItemKind::Directory {
                Equal
            } else {
                content
            },
            name,
            parent,
        ];
        if item.kind == ItemKind::File && content == Unknown {
            out.require(EvidenceRequirement::VerifyContent);
        }
        if name == Unknown {
            out.require(EvidenceRequirement::VerifyName);
        }
        if parent == Unknown {
            out.require(EvidenceRequirement::VerifyParent);
        }
        let (status, reason) = if comparisons.contains(&Different) {
            (FindingStatus::Divergent, FindingReason::DeleteVsChange)
        } else if comparisons.contains(&Unknown) {
            (FindingStatus::Unknown, FindingReason::InsufficientEvidence)
        } else {
            (FindingStatus::Compatible, FindingReason::UnilateralChange)
        };
        finding(out, ConflictDimension::Presence, status, reason);
        return;
    }
    finding(
        out,
        ConflictDimension::Presence,
        FindingStatus::Compatible,
        FindingReason::Equivalent,
    );
    let base = trusted.filter(|b| {
        b.local.presence == PresenceState::Present && b.remote.presence == PresenceState::Present
    });
    if item.kind == ItemKind::File {
        assess_dimension(
            out,
            ConflictDimension::Content,
            if base.is_some() {
                evidence.content.local_to_baseline
            } else {
                Unknown
            },
            if base.is_some() {
                evidence.content.remote_to_baseline
            } else {
                Unknown
            },
            evidence.content.current_sides,
            base.is_some(),
        );
    }
    let local = local.unwrap();
    let remote = remote.unwrap();
    assess_dimension(
        out,
        ConflictDimension::Name,
        compare(
            Some(local_name(local)),
            base.and_then(|b| b.local.local_name_bytes.as_deref()),
        ),
        compare(
            known(&remote.name),
            base.and_then(|b| b.remote.name.as_ref()),
        ),
        compare(
            Some(local_name(local)),
            known(&remote.name).map(|s| s.as_bytes()),
        ),
        base.is_some(),
    );
    assess_dimension(
        out,
        ConflictDimension::Location,
        local_parent_comparison(
            local,
            evidence.local_parent.as_ref(),
            base.map(|b| &b.local),
        ),
        compare(
            remote_parent(remote).as_ref(),
            base.and_then(|b| b.remote.parent_identity.as_ref()),
        ),
        evidence.current_parents,
        base.is_some(),
    );
}

fn assess_dimension(
    out: &mut ConflictAssessment,
    dimension: ConflictDimension,
    local: EvidenceComparison,
    remote: EvidenceComparison,
    current: EvidenceComparison,
    has_baseline: bool,
) {
    let (status, reason) =
        if current == Equal || (local == Equal && remote == Equal && current != Different) {
            (FindingStatus::Compatible, FindingReason::Equivalent)
        } else if matches!((local, remote), (Equal, Different) | (Different, Equal)) {
            (FindingStatus::Compatible, FindingReason::UnilateralChange)
        } else if local == Different && remote == Different && current == Different {
            (FindingStatus::Divergent, FindingReason::ConcurrentChanges)
        } else {
            out.require(match dimension {
                ConflictDimension::Content => EvidenceRequirement::VerifyContent,
                ConflictDimension::Name => EvidenceRequirement::VerifyName,
                _ => EvidenceRequirement::VerifyParent,
            });
            if !has_baseline {
                out.require(EvidenceRequirement::VerifyBaseline);
            }
            (
                FindingStatus::Unknown,
                if has_baseline {
                    FindingReason::InsufficientEvidence
                } else {
                    FindingReason::MissingBaseline
                },
            )
        };
    finding(out, dimension, status, reason);
}
fn finding(
    out: &mut ConflictAssessment,
    dimension: ConflictDimension,
    status: FindingStatus,
    reason: FindingReason,
) {
    out.findings.push(ConflictFinding {
        dimension,
        status,
        reason,
    });
}
fn known<T>(value: &MetadataValue<T>) -> Option<&T> {
    if let MetadataValue::Known(value) = value {
        Some(value)
    } else {
        None
    }
}
fn compare<T: PartialEq + ?Sized>(a: Option<&T>, b: Option<&T>) -> EvidenceComparison {
    match (a, b) {
        (Some(a), Some(b)) if a == b => Equal,
        (Some(_), Some(_)) => Different,
        _ => Unknown,
    }
}
fn local_name(file: &crate::models::LocalFile) -> &[u8] {
    file.relative_path
        .as_bytes()
        .rsplit(|b| *b == b'/')
        .next()
        .unwrap()
}
fn remote_parent(file: &crate::models::RemoteFile) -> Option<BaselineParent> {
    known(&file.parent_ids).and_then(|ids| {
        (ids.len() == 1).then(|| {
            BaselineParent::Remote(RemoteFileRef {
                account_id: file.reference.account_id.clone(),
                file_id: ids[0].clone(),
            })
        })
    })
}

fn validate(request: &ConflictRequest) -> Result<(), ConflictError> {
    let item = &request.item;
    if item.id.0.trim().is_empty() || item.revision == 0 {
        return Err(ConflictError::InvalidRequest);
    }
    if request.evidence.baseline_confirmed && item.baseline.is_none() {
        return Err(ConflictError::InvalidEvidence);
    }
    for file in [item.local.as_ref(), item.local_latest.as_ref()]
        .into_iter()
        .flatten()
    {
        file.validate()
            .map_err(|_| ConflictError::InconsistentInput)?;
        if file
            .local_id
            .as_ref()
            .is_some_and(|id| Some(id) != item.local_file_id.as_ref())
        {
            return Err(ConflictError::WrongItemIdentity);
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
                return Err(ConflictError::InconsistentInput);
            }
        }
    }
    for file in [item.remote.as_ref(), item.remote_latest.as_ref()]
        .into_iter()
        .flatten()
    {
        file.validate()
            .map_err(|_| ConflictError::InconsistentInput)?;
        if Some(&file.reference) != item.remote_ref.as_ref() {
            return Err(ConflictError::WrongItemIdentity);
        }
        if let Some(kind) = known(&file.kind) {
            if !matches!(
                (&item.kind, kind),
                (ItemKind::File, RemoteEntryKind::File)
                    | (ItemKind::Directory, RemoteEntryKind::Directory)
                    | (ItemKind::Other, _)
            ) {
                return Err(ConflictError::InconsistentInput);
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
        return Err(ConflictError::InconsistentInput);
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
        return Err(ConflictError::WrongItemIdentity);
    }
    if let (Some(retained), Some(latest)) = (&item.remote, &item.remote_latest) {
        if retained.provider != latest.provider {
            return Err(ConflictError::WrongItemIdentity);
        }
    }
    if let Some(base) = &item.baseline {
        if matches!(base.local.parent_identity, Some(BaselineParent::Remote(_)))
            || matches!(base.remote.parent_identity, Some(BaselineParent::Local(_)))
        {
            return Err(ConflictError::InconsistentInput);
        }
        if let (Some(BaselineParent::Remote(parent)), Some(resource)) =
            (&base.remote.parent_identity, &item.remote_ref)
        {
            if parent.account_id != resource.account_id {
                return Err(ConflictError::WrongItemIdentity);
            }
        }
    }
    let content = &request.evidence.content;
    if content.current_sides != Unknown
        && (item.kind != ItemKind::File
            || item.local_presence != PresenceState::Present
            || item.remote_presence != PresenceState::Present)
    {
        return Err(ConflictError::InvalidEvidence);
    }
    if request.evidence.baseline_confirmed
        && content.local_to_baseline == Equal
        && content.remote_to_baseline == Equal
        && content.current_sides == Different
    {
        return Err(ConflictError::InvalidEvidence);
    }
    Ok(())
}

/// Root is a known location, whereas an unmapped nested parent is unknown.
fn local_parent_comparison(
    file: &crate::models::LocalFile,
    current: Option<&crate::inventory::SyncItemId>,
    baseline: Option<&crate::inventory::BaselineSide>,
) -> EvidenceComparison {
    let Some(baseline) = baseline else {
        return EvidenceComparison::Unknown;
    };
    if !file.relative_path.as_bytes().contains(&b'/') {
        if baseline.parent_identity.is_none() {
            EvidenceComparison::Equal
        } else {
            EvidenceComparison::Different
        }
    } else if current.is_some() && baseline.parent_identity.is_none() {
        EvidenceComparison::Different
    } else {
        compare(
            current.cloned().map(BaselineParent::Local).as_ref(),
            baseline.parent_identity.as_ref(),
        )
    }
}
