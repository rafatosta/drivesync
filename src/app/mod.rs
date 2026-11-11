//! Application composition; all engine requests use its public root API.
mod lifecycle;
pub(crate) mod main_menu;
pub(crate) mod navigation;
pub(crate) mod restoration;
use drivesync_engine::{Engine, GoogleSignInId};
pub(crate) use lifecycle::{run, APP_ID};
use std::{cell::RefCell, rc::Rc};
/// GTK-owned handles, containing no authorization material.
#[derive(Clone, Default)]
pub(crate) struct Context {
    pub engine: Rc<RefCell<Option<Engine>>>,
    pub pending: Rc<RefCell<Option<GoogleSignInId>>>,
}
#[cfg(test)]
mod tests;
