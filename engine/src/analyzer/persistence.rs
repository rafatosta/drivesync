//! Explicit mappings between independent algorithms and Inventory-owned durable facts.
use super::{conflicts as c, reconciliation as r, *};
use crate::inventory::*;

fn relation(value: EvidenceRelation) -> c::EvidenceComparison {
    match value {
        EvidenceRelation::Equal => c::EvidenceComparison::Equal,
        EvidenceRelation::Different => c::EvidenceComparison::Different,
        EvidenceRelation::Unknown => c::EvidenceComparison::Unknown,
    }
}
fn change(value: EvidenceRelation) -> r::Change {
    match value {
        EvidenceRelation::Equal => r::Change::Unchanged,
        EvidenceRelation::Different => r::Change::Changed,
        EvidenceRelation::Unknown => r::Change::Unknown,
    }
}
fn comparison(value: EvidenceRelation) -> r::Comparison {
    match value {
        EvidenceRelation::Equal => r::Comparison::Equal,
        EvidenceRelation::Different => r::Comparison::Different,
        EvidenceRelation::Unknown => r::Comparison::Unknown,
    }
}
/// A known trashed remote object proves absence from the active tree, not
/// permanent deletion. Preserve the original Inventory snapshot for persistence.
/// Unknown presence/access and retained fields never establish this projection.
fn active_view(request: &AnalyzeRequest) -> AnalyzeRequest {
    let mut view = request.clone();
    if view.item.remote_presence == PresenceState::Present
        && view.evidence.remote_scope_complete
        && view
            .item
            .remote_latest
            .as_ref()
            .or(view.item.remote.as_ref())
            .is_some_and(|r| r.trashed == crate::models::MetadataValue::Known(true))
    {
        view.item.remote_presence = PresenceState::AbsentConfirmed;
        view.evidence.current_content = EvidenceRelation::Unknown;
    }
    view
}
pub(super) fn reconcile(request: &AnalyzeRequest) -> AnalyzerResult<r::ReconciliationAssessment> {
    let active = active_view(request);
    let request = &active;
    let e = &request.evidence;
    // Reconciliation assumes a confirmed baseline. Do not erase an untrusted
    // historical snapshot and reinterpret deletion as first-time creation.
    if request.item.baseline.is_some() && !e.baseline_confirmed {
        return Ok(r::ReconciliationAssessment {
            item_id: request.item.id.clone(),
            item_revision: request.item.revision,
            decisions: vec![],
            divergences: vec![],
            requirements: vec![r::Requirement::VerifyBaseline],
        });
    }
    let mut assessment = r::reconcile(r::ReconciliationRequest {
        item: request.item.clone(),
        evidence: r::ReconciliationEvidence {
            local_observation: if e.local_scope_complete {
                r::ObservationConfidence::Complete
            } else if request.item.local_presence == PresenceState::Unknown {
                r::ObservationConfidence::Unknown
            } else {
                r::ObservationConfidence::Partial
            },
            remote_observation: if e.remote_scope_complete {
                r::ObservationConfidence::Complete
            } else if request.item.remote_presence == PresenceState::Unknown {
                r::ObservationConfidence::Unknown
            } else {
                r::ObservationConfidence::Partial
            },
            local_content: change(e.local_content),
            remote_content: change(e.remote_content),
            content_comparison: comparison(e.current_content),
            local_parent: e.local_parent.clone(),
            parent_comparison: comparison(e.current_parents),
        },
    })
    .map_err(|cause| {
        error(match cause {
            r::ReconciliationError::InvalidItemIdentity => AnalyzerErrorKind::WrongSyncScope,
            r::ReconciliationError::InvalidEvidence => AnalyzerErrorKind::InvalidEvidence,
            _ => AnalyzerErrorKind::InvalidRequest,
        })
    })?;
    for observation in request
        .observations
        .iter()
        .filter(|observation| observation_targets_item(observation, &request.item))
    {
        match observation.event.action {
            crate::models::FileEventAction::Deleted
            | crate::models::FileEventAction::RemovedFromScope => {
                assessment.require(r::Requirement::VerifySurvivingState);
                assessment.require(r::Requirement::AuthorizeDeletion);
            }
            crate::models::FileEventAction::MoveFrom | crate::models::FileEventAction::MoveTo => {
                assessment.require(r::Requirement::VerifySurvivingState);
            }
            _ => {}
        }
    }
    Ok(assessment)
}

fn observation_targets_item(observation: &crate::models::File, item: &SyncItem) -> bool {
    use crate::models::{FileEventAction, FileLocation, FileScope};
    let local_matches = |path: &crate::models::LocalRelativePath| {
        item.local_latest
            .as_ref()
            .or(item.local.as_ref())
            .is_some_and(|local| &local.relative_path == path)
    };
    match &observation.scope {
        FileScope::Local(path) => local_matches(path)
            || [&observation.event.previous, &observation.event.current]
                .into_iter()
                .flatten()
                .any(
                    |location| matches!(location, FileLocation::Local(path) if local_matches(path)),
                ),
        FileScope::Remote(reference) => item.remote_ref.as_ref() == Some(reference),
        FileScope::WholeSync => matches!(
            observation.event.action,
            FileEventAction::RecheckRequired | FileEventAction::ScopeUnavailable
        ),
    }
}
pub(super) fn conflicts(request: &AnalyzeRequest) -> AnalyzerResult<c::ConflictAssessment> {
    let active = active_view(request);
    let request = &active;
    let e = &request.evidence;
    c::evaluate(c::ConflictRequest {
        item: request.item.clone(),
        evidence: c::ConflictEvidence {
            baseline_confirmed: e.baseline_confirmed,
            local_scope_complete: e.local_scope_complete,
            remote_scope_complete: e.remote_scope_complete,
            content: c::ContentEvidence {
                local_to_baseline: relation(e.local_content),
                remote_to_baseline: relation(e.remote_content),
                current_sides: relation(e.current_content),
            },
            local_parent: e.local_parent.clone(),
            current_parents: relation(e.current_parents),
        },
    })
    .map_err(|cause| {
        error(match cause {
            c::ConflictError::WrongItemIdentity => AnalyzerErrorKind::WrongSyncScope,
            c::ConflictError::InvalidEvidence => AnalyzerErrorKind::InvalidEvidence,
            _ => AnalyzerErrorKind::InvalidRequest,
        })
    })
}
fn r_dimension(value: r::Dimension) -> AnalysisDimension {
    match value {
        r::Dimension::Presence => AnalysisDimension::Presence,
        r::Dimension::Content => AnalysisDimension::Content,
        r::Dimension::Name => AnalysisDimension::Name,
        r::Dimension::Location => AnalysisDimension::Location,
    }
}
fn c_dimension(value: c::ConflictDimension) -> AnalysisDimension {
    match value {
        c::ConflictDimension::Presence => AnalysisDimension::Presence,
        c::ConflictDimension::Content => AnalysisDimension::Content,
        c::ConflictDimension::Name => AnalysisDimension::Name,
        c::ConflictDimension::Location => AnalysisDimension::Location,
    }
}
fn add(requirements: &mut Vec<AnalysisRequirement>, value: AnalysisRequirement) {
    if !requirements.contains(&value) {
        requirements.push(value);
    }
}
pub(super) fn assessment(
    reconciliation: r::ReconciliationAssessment,
    conflicts: c::ConflictAssessment,
) -> AnalyzerResult<NewAnalysisAssessment> {
    let mut findings = vec![];
    let mut requirements = vec![];
    for f in &conflicts.findings {
        findings.push(AnalysisFinding {
            origin: AnalysisOrigin::Conflicts,
            dimension: c_dimension(f.dimension),
            status: match f.status {
                c::FindingStatus::Compatible => AnalysisFindingStatus::Compatible,
                c::FindingStatus::Divergent => AnalysisFindingStatus::Divergent,
                c::FindingStatus::Unknown => AnalysisFindingStatus::Unknown,
            },
            reason: match f.reason {
                c::FindingReason::Equivalent => AnalysisReason::Equivalent,
                c::FindingReason::UnilateralChange => AnalysisReason::UnilateralChange,
                c::FindingReason::ConcurrentChanges => AnalysisReason::ConcurrentChanges,
                c::FindingReason::DeleteVsChange => AnalysisReason::DeleteVsChange,
                c::FindingReason::NewItem => AnalysisReason::NewItem,
                c::FindingReason::MissingBaseline => AnalysisReason::MissingBaseline,
                c::FindingReason::InsufficientEvidence => AnalysisReason::InsufficientEvidence,
                c::FindingReason::UnsupportedKind => AnalysisReason::UnsupportedKind,
            },
        });
    }
    for f in reconciliation.divergences {
        findings.push(AnalysisFinding {
            origin: AnalysisOrigin::Reconciliation,
            dimension: r_dimension(f.dimension),
            status: if matches!(
                f.reason,
                r::DivergenceReason::ConcurrentChanges | r::DivergenceReason::DeleteVsChange
            ) {
                AnalysisFindingStatus::Divergent
            } else {
                AnalysisFindingStatus::Unknown
            },
            reason: match f.reason {
                r::DivergenceReason::ConcurrentChanges => AnalysisReason::ConcurrentChanges,
                r::DivergenceReason::DeleteVsChange => AnalysisReason::DeleteVsChange,
                r::DivergenceReason::MissingBaseline => AnalysisReason::MissingBaseline,
                r::DivergenceReason::IncompatibleStates => AnalysisReason::IncompatibleStates,
                r::DivergenceReason::InconsistentEvidence => AnalysisReason::InconsistentEvidence,
            },
        });
        if !matches!(
            f.reason,
            r::DivergenceReason::ConcurrentChanges | r::DivergenceReason::DeleteVsChange
        ) {
            add(&mut requirements, AnalysisRequirement::VerifyBaseline);
        }
    }
    for r in reconciliation.requirements {
        add(
            &mut requirements,
            match r {
                r::Requirement::Observe(r::Side::Local) => AnalysisRequirement::ObserveLocal,
                r::Requirement::Observe(r::Side::Remote) => AnalysisRequirement::ObserveRemote,
                r::Requirement::Compare(r::Dimension::Content) | r::Requirement::VerifyContent => {
                    AnalysisRequirement::VerifyContent
                }
                r::Requirement::Compare(r::Dimension::Name) => AnalysisRequirement::VerifyName,
                r::Requirement::Compare(r::Dimension::Location) => {
                    AnalysisRequirement::VerifyParent
                }
                r::Requirement::Compare(r::Dimension::Presence)
                | r::Requirement::VerifyIdentity => AnalysisRequirement::VerifyIdentity,
                r::Requirement::VerifyBaseline => AnalysisRequirement::VerifyBaseline,
                r::Requirement::VerifySurvivingState => AnalysisRequirement::VerifySurvivingState,
                r::Requirement::AuthorizeDeletion => AnalysisRequirement::AuthorizeDeletion,
                r::Requirement::ResolveDestination => AnalysisRequirement::ResolveDestination,
                r::Requirement::SupportedKind => AnalysisRequirement::VerifyKind,
            },
        );
    }
    for r in &conflicts.requirements {
        add(
            &mut requirements,
            match r {
                c::EvidenceRequirement::ObserveLocal => AnalysisRequirement::ObserveLocal,
                c::EvidenceRequirement::ObserveRemote => AnalysisRequirement::ObserveRemote,
                c::EvidenceRequirement::VerifyBaseline => AnalysisRequirement::VerifyBaseline,
                c::EvidenceRequirement::VerifyContent => AnalysisRequirement::VerifyContent,
                c::EvidenceRequirement::VerifyParent => AnalysisRequirement::VerifyParent,
                c::EvidenceRequirement::VerifyName => AnalysisRequirement::VerifyName,
                c::EvidenceRequirement::VerifyKind => AnalysisRequirement::VerifyKind,
            },
        );
    }
    let intents = reconciliation
        .decisions
        .into_iter()
        .map(|decision| {
            let (source, kind) = match decision {
                r::Decision::Trash { target } => (
                    match target {
                        r::Side::Local => r::Side::Remote,
                        r::Side::Remote => r::Side::Local,
                    },
                    OperationKind::Trash,
                ),
                r::Decision::Create { source, .. } => (source, OperationKind::Create),
                r::Decision::Propagate { dimension, source } => (
                    source,
                    match dimension {
                        r::Dimension::Content => OperationKind::UpdateContent,
                        r::Dimension::Name => OperationKind::Rename,
                        r::Dimension::Location => OperationKind::Move,
                        r::Dimension::Presence => return Err(error(AnalyzerErrorKind::Internal)),
                    },
                ),
            };
            Ok(AnalysisIntent {
                direction: match source {
                    r::Side::Local => OperationDirection::Upload,
                    r::Side::Remote => OperationDirection::Download,
                },
                kind,
            })
        })
        .collect::<AnalyzerResult<Vec<_>>>()?;
    let disposition = if conflicts.requires_user_intervention() {
        AnalysisDisposition::PendingUserDecision
    } else if !requirements.is_empty() {
        AnalysisDisposition::PendingEvidence
    } else if !intents.is_empty() {
        AnalysisDisposition::PendingOperations
    } else {
        AnalysisDisposition::NoChanges
    };
    Ok(NewAnalysisAssessment {
        plan: None,
        disposition,
        findings,
        requirements,
        intents,
    })
}
