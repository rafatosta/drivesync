//! Configured inventory scopes for developer inspection; never starts observation or initialization.
use super::Context;
use drivesync_engine::{
    inventory::inspection::{InspectInventoryRequest, InventoryTable},
    ListAccountsRequest,
};

/// Account label and a read-only request, or the reason its configured root is inaccessible.
#[derive(Clone)]
pub(crate) struct ConfiguredInventory {
    pub account_id: String,
    pub label: String,
    pub request: Result<InspectInventoryRequest, String>,
}
/// Reads completed account setup only. No OAuth, detector start, transfer or inventory creation occurs.
pub(crate) async fn configured(context: &Context) -> Result<Vec<ConfiguredInventory>, String> {
    let engine = context
        .engine
        .borrow()
        .clone()
        .ok_or("Account service unavailable")?;
    let accounts = engine
        .list_accounts(ListAccountsRequest { provider: None })
        .await
        .map_err(|e| crate::core::messages::engine_error(&e))?
        .accounts;
    let mut targets = Vec::new();
    for account in accounts {
        let Some(config) = crate::services::configuration::load(&account.id)
            .await
            .map_err(|_| "Saved account configuration could not be read")?
        else {
            continue;
        };
        let request = crate::services::local_folder::watch_root(&config.local_uri)
            .await
            .map(|local_root| InspectInventoryRequest {
                local_root,
                account_id: account.id.clone(),
                remote_root_id: config.remote_id,
                table: InventoryTable::Items,
                after_rowid: 0,
            });
        targets.push(ConfiguredInventory {
            account_id: account.id.0.clone(),
            label: format!(
                "{} · {}",
                account.display_name.as_deref().unwrap_or("Account"),
                account.email.as_deref().unwrap_or(&account.id.0)
            ),
            request,
        });
    }
    targets.sort_by(|a, b| a.label.cmp(&b.label).then(a.account_id.cmp(&b.account_id)));
    Ok(targets)
}
