//! Pure B/L/R classification shared by planning and observational diagnostics.
//! Evidence collection and execution remain outside this module.
mod classification;
mod models;
pub use classification::classify_differences;
pub use models::*;
#[cfg(test)]
mod tests;
