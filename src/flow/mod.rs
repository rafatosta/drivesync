//! Shared GTK application flow for the initial DriveSync MVP.
//!
//! Navigation, off-main-thread engine operations, and reusable presentation
//! helpers are independent submodules. Keep the existing crate-level access
//! paths stable for welcome, dashboard, folder picker and setup screens.
mod navigation;
mod widgets;
mod worker;

pub use navigation::{account_page, initial_destination, open_account, InitialDestination};
pub use widgets::{content, row, scroll};
pub use worker::work;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod layout_tests;
