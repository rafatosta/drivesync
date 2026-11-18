//! Developer console destinations for engine inspection.
mod analyzer;
mod detector;
mod inventory;
mod logs;
mod operator;
mod orchestrator;
mod overview;
mod pipeline;
mod provider;
mod window;

pub(crate) use window::install;
