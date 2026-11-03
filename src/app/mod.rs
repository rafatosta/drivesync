//! Application lifecycle, navigation and composition of feature interfaces.
mod lifecycle;
pub(crate) mod main_menu;
pub(crate) mod navigation;
pub(crate) mod restoration;
pub(crate) use lifecycle::{run, APP_ID};

#[cfg(test)]
mod layout_tests;
