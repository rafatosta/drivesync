//! Account operations consumed by UI controllers; credentials stay in the engine.
use drivesync_engine::*;
/// Begins provider-specific authorization through the public connection API.
pub(crate) async fn begin(
    engine: &Engine,
    account_id: Option<AccountId>,
) -> EngineResult<BeginGoogleSignInResult> {
    engine
        .begin_google_sign_in(BeginGoogleSignInRequest { account_id })
        .await
}
/// Waits for authorization and secure account persistence.
pub(crate) async fn finish(
    engine: &Engine,
    connection_id: GoogleSignInId,
) -> EngineResult<FinishGoogleSignInResult> {
    engine
        .finish_google_sign_in(FinishGoogleSignInRequest { connection_id })
        .await
}
/// Cancels an attempt without promising rollback of persistence already underway.
pub(crate) fn cancel(engine: &Engine, connection_id: GoogleSignInId) -> bool {
    engine
        .cancel_google_sign_in(CancelGoogleSignInRequest { connection_id })
        .cancelled
}
/// Opens the public authorization URL without retaining it in persistent storage.
pub(crate) async fn open_authorization_url(url: &str) -> Result<(), adw::gtk::glib::Error> {
    adw::gtk::gio::AppInfo::launch_default_for_uri_future(
        url,
        None::<&adw::gtk::gio::AppLaunchContext>,
    )
    .await
}
/// Changes only the local display name and returns the authoritative saved record.
pub(crate) async fn rename(
    engine: &Engine,
    account_id: AccountId,
    display_name: Option<String>,
) -> EngineResult<Account> {
    engine
        .update_account(UpdateAccountRequest {
            account_id,
            display_name,
        })
        .await
        .map(|result| result.account)
}

/// Removes credentials/metadata first; a draft cleanup failure cannot resurrect
/// the account and is reported separately for accurate presentation.
pub(crate) async fn disconnect(engine: &Engine, account_id: AccountId) -> EngineResult<bool> {
    engine
        .disconnect_account(RemoveAccountRequest {
            account_id: account_id.clone(),
        })
        .await?;
    let removed = super::configuration::remove(&account_id).await.is_ok();
    Ok(super::setup::discard_draft(&account_id).await && removed)
}
