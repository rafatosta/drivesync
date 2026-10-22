//! Drive folder browser and selection dialog.
//!
//! Builds the native dialog separately from the browsing state and pagination
//! logic. Folder metadata comes from the engine; no transfers occur here.
mod controller;
mod view;

pub use controller::show;
