//! Complete descriptor-relative inventories and bounded file-write stabilization.
//! Observations never authorize installation, upload, adoption or deletion.
mod inventory;
mod models;
pub(crate) use inventory::{check, ignored, scan, stamp};
pub(crate) use models::{Evidence, Inventory, Stamp};
#[cfg(test)]
mod tests;
