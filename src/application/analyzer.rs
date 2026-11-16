//! Scoped analysis reads and explicit user association for the Developer Console.
//! Page reads never prepare or execute; a confirmed association reuses preparation.
use super::{inventory, Context};
use drivesync_engine::inventory::*;

#[derive(Clone)]
pub(crate) struct Scope {
    pub account_id: String,
    pub label: String,
    pub request: Result<OpenInventoryRequest, String>,
}
pub(crate) struct DecisionPage {
    pub entries: Vec<(RecordedAnalysis, SyncItem)>,
    pub next_cursor: Option<String>,
}
/// Uses saved account configuration and the existing prepared session's identity.
/// Opening this page never starts capture/scanning or invents a synchronization ID.
pub(crate) async fn configured(context: &Context) -> Result<Vec<Scope>, String> {
    let configured = inventory::configured(context).await?;
    let sessions = context.detectors.borrow();
    Ok(configured.into_iter().map(|candidate| {
        let request=candidate.request.and_then(|configured| {
            let session=sessions.get(&candidate.account_id).ok_or("Preparation has not started for this account. Refresh after startup.")?;
            let scope=session.orchestrator.detector().scope();
            if scope.local_root!=configured.local_root || scope.account_id!=configured.account_id || scope.remote_root_id!=configured.remote_root_id {
                return Err("Preparation does not match this account's current folder. Refresh after preparation.".into());
            }
            Ok(OpenInventoryRequest{local_root:configured.local_root,expected:SyncIdentity{sync_id:scope.sync_id.clone(),account_id:scope.account_id.clone(),remote_root_id:scope.remote_root_id.clone()},create_if_missing:false})
        });
        Scope{account_id:candidate.account_id,label:candidate.label,request}
    }).collect())
}
/// Blocking: call on a worker. Reads existing Inventory through domain methods only.
pub(crate) fn read(
    mut request: OpenInventoryRequest,
    after: Option<String>,
    include_outdated: bool,
) -> InventoryResult<DecisionPage> {
    request.create_if_missing = false;
    let inventory = InventoryManager::open(request)?.inventory;
    let page = inventory
        .list_analyses(ListAnalysesRequest {
            filter: AnalysisFilter {
                current_only: !include_outdated,
                ..Default::default()
            },
            pagination: Pagination {
                after,
                ..Default::default()
            },
        })?
        .page;
    let mut entries = Vec::with_capacity(page.entries.len());
    for analysis in page.entries {
        let item = inventory
            .get_item(GetItemRequest {
                selector: ItemSelector::Id(analysis.item_id.clone()),
            })?
            .item
            .ok_or_else(|| InventoryFailure {
                kind: InventoryError::NotFound,
                message: "Analyzed item unavailable".into(),
                retryable: false,
            })?;
        // Rows and their current names are separate public reads. Do not present
        // a receipt as current when the item changed between those reads.
        let mut analysis = analysis;
        analysis.stale |= item.revision != analysis.resulting_item_revision;
        entries.push((analysis, item));
    }
    Ok(DecisionPage {
        entries,
        next_cursor: page.next_cursor,
    })
}

pub(crate) struct AssociationCandidates {
    pub local: Vec<SyncItem>,
    pub remote: Vec<SyncItem>,
}
/// Blocking read only: enumerate independent identities without name matching.
pub(crate) fn association_candidates(
    mut request: OpenInventoryRequest,
) -> InventoryResult<AssociationCandidates> {
    request.create_if_missing = false;
    let info = request.expected.clone();
    let inventory = InventoryManager::open(request)?.inventory;
    let mut result = AssociationCandidates {
        local: vec![],
        remote: vec![],
    };
    let mut pagination = Pagination::default();
    loop {
        let page = inventory
            .list_items(ListItemsRequest {
                filter: ItemFilter::default(),
                pagination: pagination.clone(),
            })?
            .page;
        for item in page.entries {
            if item.baseline.is_some() {
                continue;
            }
            if item.local_presence == PresenceState::Present && item.remote_ref.is_none() {
                result.local.push(item)
            } else if item.remote_presence == PresenceState::Present
                && item.local.is_none()
                && item.local_latest.is_none()
                && item.local_file_id.is_none()
                && item
                    .remote_ref
                    .as_ref()
                    .is_some_and(|r| r.file_id.0 != info.remote_root_id)
            {
                result.remote.push(item)
            }
        }
        match page.next_cursor {
            Some(next) => pagination.after = Some(next),
            None => return Ok(result),
        }
    }
}
/// Explicit user action only. Persist identity choice off GTK, then reuse the
/// existing session preparation to reanalyze; never execute an Operator.
pub(crate) async fn associate(
    context: &Context,
    scope: Scope,
    choice: AssociateItemsRequest,
) -> Result<Option<String>, String> {
    let mut opening = scope.request?;
    opening.create_if_missing = false;
    let session = context
        .detectors
        .borrow()
        .get(&scope.account_id)
        .map(|s| s.orchestrator.clone());
    if let Some(session) = &session {
        let current = session.detector().scope();
        if current.sync_id != opening.expected.sync_id
            || current.local_root != opening.local_root
            || current.account_id != opening.expected.account_id
            || current.remote_root_id != opening.expected.remote_root_id
        {
            return Err("Account scope changed. Reopen the association dialog.".into());
        }
    }
    adw::gtk::gio::spawn_blocking(move || {
        InventoryManager::open(opening)?
            .inventory
            .associate_items(choice)
    })
    .await
    .map_err(|_| "Association worker unavailable".to_string())?
    .map_err(|e| format!("Association was refused: {:?}", e.kind))?;
    match session {
        Some(session) => match session.prepare().await {
            Ok(_) => Ok(None),
            Err(e) => Ok(Some(format!(
                "Association saved; preparation needs attention: {e:?}"
            ))),
        },
        None => Ok(Some(
            "Association saved. Reopen the application to prepare this account.".into(),
        )),
    }
}
