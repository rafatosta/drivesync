//! Analysis coordination boundary; algorithms remain inaccessible to other modules.
// The facade and Operator are not implemented yet. Keep the pure core private until
// integration rather than exposing it solely to suppress unused-code diagnostics.
#[allow(dead_code)]
mod reconciliation;
// Identification is isolated until the Analyzer facade coordinates both algorithms.
#[allow(dead_code)]
mod conflicts;
