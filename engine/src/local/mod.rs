//! Complete descriptor-relative inventories and bounded file-write stabilization.
//! Observations never authorize installation, upload, adoption or deletion.
mod inventory;
mod models;
mod monitoring;
pub(crate) use inventory::{check, ignored, scan, stamp, temporary};
pub(crate) use models::{Evidence, Inventory, Stamp};
pub use monitoring::{MonitorDirectory, MonitorError, MonitorTree};
#[cfg(test)]
mod tests;
