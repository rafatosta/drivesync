//! Golden input and output artifacts for the Analyzer submodule contracts.

#[path = "analyzer_contracts/apply.rs"]
mod apply;
#[path = "analyzer_contracts/context.rs"]
mod context;
#[path = "analyzer_contracts/decision.rs"]
mod decision;
#[path = "analyzer_contracts/support.rs"]
mod support;

#[path = "analyzer_contracts/context_behavior.rs"]
mod context_behavior;

#[path = "analyzer_contracts/decision_fixtures.rs"]
mod decision_fixtures;
#[path = "analyzer_contracts/decision_behavior.rs"]
mod decision_behavior;

#[path = "analyzer_contracts/apply_fixtures.rs"]
mod apply_fixtures;
#[path = "analyzer_contracts/apply_mocks.rs"]
mod apply_mocks;
#[path = "analyzer_contracts/apply_behavior.rs"]
mod apply_behavior;
