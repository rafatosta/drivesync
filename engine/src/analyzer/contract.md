# Analyzer facade contract — forward only

Analyzer::analyze(File, feedback) is unchanged. The facade invokes Context,
Decision and Apply exactly once, in that order, preserving artifacts. It
validates File before the pipeline and structural target coverage at each
boundary. It never obtains evidence, executes operations or calls Detector,
Resolve or a previous stage. Feedback reports Inspecting, Reconciling and
Applying before their calls; no invented progress or duplicate Started.
Stage errors propagate unchanged. The Orchestrator owns acknowledgement.

Context owns all collection and enrichment needed for a decision. It may
read persisted facts, filesystem and Provider independently. It may not
delegate evidence gathering to Detector, Operator or Resolve. Missing fields
remain explicit and are not discarded. Decision consumes the completed
Context and produces a conclusive semantic outcome including terminal
InsufficientEvidence if necessary. Apply consumes it once, persists it and
executes all selected intents in order, renewing guards from verified effects.
Operator verifies and executes selected commands only.

A stale guard or external change interrupts work; partial effects are
preserved. No PendingReanalysis or EvidenceRequested callback. A subsequent
independent File starts its own analysis. Durable recovery may retry the same
execution without replaying earlier stages. Successful durable handling is not
a declaration of synchronization. InvalidResult rejects incompatible targets,
order, revisions or an actionable outcome for a non-actionable decision.

This is a staged conceptual contract. The Detector, Operator, Orchestrator
and Inventory implementations are intentionally unchanged. External File literals
require migration for FileEvidence. No analyzer stage algorithms are implemented.
See information-requirements.md. Compilation and RED are pending.
