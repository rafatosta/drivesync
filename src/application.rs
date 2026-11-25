//! Application composition; all engine requests use its public root API.
mod acceptance;
pub(crate) mod accounts;
mod background;
mod lifecycle;
use crate::ui::components::main_menu;
pub(crate) mod navigation;
use crate::ui::dashboard::restoration;
use drivesync_engine::{Engine, GoogleSignInId};
pub(crate) use lifecycle::{run, APP_ID};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
/// GTK-owned handles, containing no authorization material.
#[derive(Clone, Default)]
pub(crate) struct Context {
    pub engine: Rc<RefCell<Option<Engine>>>,
    pub navigation_generation: Rc<Cell<u64>>,
    pub pending: Rc<RefCell<Option<GoogleSignInId>>>,
}
/// Restores completed synchronization scopes before presenting the saved accounts.
pub(crate) async fn restore_engine(
    config: drivesync_engine::EngineConfig,
    context: &Context,
) -> drivesync_engine::EngineResult<Vec<drivesync_engine::Account>> {
    let current = context.engine.borrow().clone();
    let engine = match current {
        Some(engine) => engine,
        None => Engine::open(config).await?,
    };
    let result = engine
        .list_accounts(drivesync_engine::ListAccountsRequest { provider: None })
        .await?;
    crate::services::configuration::restore_synchronization(&engine, &result.accounts).await;
    *context.engine.borrow_mut() = Some(engine);
    Ok(result.accounts)
}
#[cfg(test)]
mod tests;
