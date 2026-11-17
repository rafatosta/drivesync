//! Shared presentation components with private widget implementations.
pub(crate) mod connection;
pub(crate) mod main_menu;
mod widgets;
pub(crate) use widgets::{content, row, scroll, svg_icon};
pub(crate) mod confirmation;

pub(crate) mod remote_folder_picker;

pub(crate) mod sync_status;

pub(crate) mod alerts;

pub(crate) mod recent_activity;
