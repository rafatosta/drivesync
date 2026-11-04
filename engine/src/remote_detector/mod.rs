//! Remote detection state, isolated from the retired synchronization engine.
//!
//! The journal does not perform synchronization or access the network.
mod detector;
mod journal;
mod provider;
