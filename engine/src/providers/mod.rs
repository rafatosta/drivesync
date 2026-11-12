//! Technical mappings of external service APIs.
//!
//! Each submodule exposes the operations of a single provider. There is no
//! universal provider contract.

pub mod google;

mod registry;
pub use registry::{list_providers, ConnectionMethod, RegisteredProvider};
