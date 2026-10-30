//! Pure B/L/R classification shared by planning and observational diagnostics.
//! Evidence collection and execution remain outside this module.
mod classification;
mod models;
mod planning;
mod revalidation;
pub use classification::classify_differences;
pub use models::*;
pub(crate) use planning::{binary_decision, plan_diagnosis, refine_content};
pub use revalidation::{
    LocalState, RemoteState, ResolutionObservation, ResolutionProposal, StaleResolutionReason,
};
#[cfg(test)]
mod tests;
