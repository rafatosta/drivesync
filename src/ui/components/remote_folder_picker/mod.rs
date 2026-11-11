//! Provider-independent, asynchronous remote folder selection.
#![allow(dead_code, unused_imports)] // The binary exposes a complete source boundary for future adapters.
mod browser;
mod dialog;
mod folder_item;
mod mock;
mod model;
mod navigation;
mod source;
pub use dialog::RemoteFolderPicker;
pub use mock::MockFolderSource;
pub use model::*;
pub use source::*;

#[cfg(test)]
mod tests;

pub(crate) mod engine_source;
