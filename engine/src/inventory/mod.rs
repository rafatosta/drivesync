//! Per-synchronization transactional inventory, independent of observation and execution.
//! Blocking methods must run off the UI thread; status reads a separate cached snapshot.
mod models;
mod storage;
pub use models::*;
pub use storage::{inspection, InventoryManager};
