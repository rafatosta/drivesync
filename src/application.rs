//! Application composition; all engine requests use its public root API.
pub(crate) mod accounts;
mod background;
pub(crate) mod detectors;
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
    pub detector_generation: Rc<Cell<u64>>,
    pub detectors: Rc<RefCell<std::collections::HashMap<String, detectors::Session>>>,
    pub navigation_generation: Rc<Cell<u64>>,
    pub pending: Rc<RefCell<Option<GoogleSignInId>>>,
}
#[cfg(test)]
mod tests;
