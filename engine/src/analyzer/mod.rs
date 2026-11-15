//! Analysis coordination boundary; algorithms remain inaccessible to other modules.
// The facade and Operator are not implemented yet. Keep the pure core private until
// integration rather than exposing it solely to suppress unused-code diagnostics.
#[allow(dead_code)]
mod reconciliation;
