//! Provider-independent, asynchronous remote folder selection.
#![allow(dead_code, unused_imports)] // Reusable component awaiting integration by a setup owner.
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
