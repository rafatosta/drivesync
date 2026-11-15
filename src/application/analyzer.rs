//! Scoped, bounded analysis reads for the Developer Console; no preparation or execution.
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
