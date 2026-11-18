//! Projects detector observations through public Inventory methods.
//! Unknown presence stays unknown; continuous ingestion never starts a full scan.
use super::PreparationFailure;
use crate::{inventory::*, models::*};

#[derive(Debug)]
pub(super) struct Ingested {
    pub item: Option<SyncItemId>,
    pub analyze_all: bool,
    pub revalidate: Option<FileScope>,
}
/// Keeps each File and its ID intact so a redelivered observation hits Inventory's
/// durable receipt before revision checks. Locator paths never pair local/remote resources.
pub(super) fn ingest(
    inventory: &InventoryManager,
    file: &File,
    identity: &SyncIdentity,
) -> Result<Ingested, PreparationFailure> {
    file.validate()
        .map_err(|_| PreparationFailure::InvalidObservation(file.id.clone()))?;
    if file.sync_id != identity.sync_id {
        return Err(PreparationFailure::InvalidObservation(file.id.clone()));
    }
    let (selector, remote) = match &file.scope {
        FileScope::WholeSync => {
            if file.data.is_some() {
                return Err(PreparationFailure::InvalidObservation(file.id.clone()));
            }
            return Ok(Ingested {
                item: None,
                analyze_all: true,
                revalidate: None,
            });
        }
        FileScope::Remote(reference) => {
            if reference.account_id != identity.account_id {
                return Err(PreparationFailure::InvalidObservation(file.id.clone()));
            }
            // Root availability/coverage notifications carry no item metadata.
            // Detector retains the technical source state; no fake SyncItem is created.
            if reference.file_id.0 == identity.remote_root_id
                && file.data.is_none()
                && matches!(
                    file.event.action,
                    FileEventAction::RecheckRequired | FileEventAction::ScopeUnavailable
                )
            {
                return Ok(Ingested {
                    item: None,
                    analyze_all: true,
                    revalidate: None,
                });
            }
            (ItemSelector::RemoteRef(reference.clone()), true)
        }
        FileScope::Local(path) => {
            let selector = match &file.data {
                Some(FileData::Local(local)) if local.local_id.is_some() => {
                    ItemSelector::LocalId(local.local_id.clone().unwrap())
                }
                _ => ItemSelector::LocalRelativePath(path.clone()),
            };
            (selector, false)
        }
    };
    let existing = inventory
        .get_item(GetItemRequest { selector })
        .map_err(PreparationFailure::Inventory)?
        .item;
    if !remote && existing.is_none() {
        if let FileScope::Local(path) = &file.scope {
            let occupied = inventory
                .get_item(GetItemRequest {
                    selector: ItemSelector::LocalRelativePath(path.clone()),
                })
                .map_err(PreparationFailure::Inventory)?
                .item;
            if let Some(occupied) = occupied {
                // A different physical identity at the old path is not an
                // automatic association. Analyze the known item without rescanning.
                return Ok(Ingested {
                    item: Some(occupied.id),
                    analyze_all: true,
                    revalidate: None,
                });
            }
        }
    }
    if file.data.is_none() && existing.is_none() {
        if remote {
            if file.event.action == FileEventAction::RemovedFromScope {
                return Ok(Ingested {
                    item: None,
                    analyze_all: true,
                    revalidate: None,
                });
            }
            return Err(PreparationFailure::InvalidObservation(file.id.clone()));
        }
        // A vanished, never-associated local locator cannot become an invented
        // tombstone. Keep the event without fabricating an Inventory item.
        return Ok(Ingested {
            item: None,
            analyze_all: true,
            revalidate: None,
        });
    }
    let target = if remote {
        let FileScope::Remote(reference) = &file.scope else {
            unreachable!()
        };
        ObservationTarget::ByRemoteRef(reference.clone())
    } else {
        existing
            .as_ref()
            .map(|item| ObservationTarget::Existing(item.id.clone()))
            .unwrap_or(ObservationTarget::NewItem)
    };
    let remote_trashed = remote
        && file.event.action == FileEventAction::RemovedFromScope
        && matches!(
            &file.data,
            Some(FileData::Remote(remote))
                if remote.trashed == MetadataValue::Known(true)
        );
    let local_presence = if remote {
        None
    } else {
        file.event.local_presence
    };
    let presence =
        if remote_trashed || local_presence == Some(LocalPresenceEvidence::AbsentConfirmed) {
            PresenceState::AbsentConfirmed
        } else if local_presence == Some(LocalPresenceEvidence::Unknown) {
            PresenceState::Unknown
        } else if file.data.is_some() {
            PresenceState::Present
        } else {
            PresenceState::Unknown
        };
    let proof = ObservationProof {
        source: if remote {
            ObservationSource::RemoteChange
        } else {
            ObservationSource::LocalEvent
        },
        observed_at: file.observed_at,
        scan_or_generation: file
            .event
            .local
            .as_ref()
            .map(|event| event.capture_id.clone()),
        complete_scope: remote_trashed
            || local_presence == Some(LocalPresenceEvidence::AbsentConfirmed),
    };
    let expected_revision = existing.map(|item| item.revision);
    let result = if remote {
        inventory.record_remote(RecordRemoteObservation {
            selector: target,
            expected_revision,
            observation: crate::inventory::RemoteObservation {
                observed: file.clone(),
                presence,
                proof,
            },
        })
    } else {
        inventory.record_local(RecordLocalObservation {
            selector: target,
            expected_revision,
            observation: crate::inventory::LocalObservation {
                observed: file.clone(),
                presence,
                proof,
            },
        })
    }
    .map_err(PreparationFailure::Inventory)?;
    let newly_applied = result.disposition == ObservationDisposition::Applied && !result.duplicate;
    Ok(Ingested {
        item: newly_applied.then_some(result.item.id),
        // Namespace and parent changes can invalidate another item's plan.
        analyze_all: newly_applied,
        revalidate: (result.disposition == ObservationDisposition::Incomparable
            && !result.duplicate)
            .then(|| file.scope.clone()),
    })
}

#[cfg(test)]
#[path = "ingestion_tests.rs"]
mod tests;
