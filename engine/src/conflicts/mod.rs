//! Pure B/L/R classification shared by planning and observational diagnostics.
//! Evidence collection and execution remain outside this module.
mod classification;
mod models;
mod planning;
pub use classification::classify_differences;
pub use models::*;
pub(crate) use planning::{binary_decision, plan_diagnosis, refine_content};
#[cfg(test)]
mod tests;
