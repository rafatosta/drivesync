# Analyzer facade contract (behavior-first)

The public Analyzer::analyze(File, feedback) interface and its asynchronous return
are unchanged. AnalyzerEngine composes ContextService, DecisionService and
ApplyService. No direct persistence, provider, Operator, Detector or queue access.
The facade owns neither synchronization policies nor retry scheduling.

## Pipeline invariants

- Validate File before progress or stage calls; invalid envelopes return
  InvalidContext. The Orchestrator owns Started and acknowledgement.
- Report Inspecting, Reconciling and Applying exactly before the corresponding
  stage, with the unchanged File. No fabricated progress for private algorithms
  or transfer/verification. The return is authoritative and terminal for this call.
- Move the artifacts through the stages once in order. Preserve the original
  File and the exact Context through Decision. Reject substituted artifacts.
- Check structural target coverage and uniqueness against Context before Apply.
  Matched has one exact item/revision; Unmatched/Ambiguous one observation;
  Scope has each item/revision in stable order or Synchronization if empty.
  This is structural validation, not new synchronization reasoning.
- Propagate stage errors unchanged, never execute a later stage on error,
  never retry or recursively reanalyze inside this facade.
- The only authoritative successful return comes from Apply. Completed must
  match the decision target identities, revisions, cardinality and order.
  A mismatch returns InvalidResult. A duplicate may be proven by Apply for
  any valid delivery. Whole-delivery Obsolete requires all targets obsolete.
- Reject physically actionable outcomes for non-actionable decisions.
  In particular Conflict/NeedsEvidence cannot claim execution or baseline
  establishment. A stale guard may return PendingReanalysis even for a
  nonmutating semantic decision; evidence and operational blockers are
  legitimate downgrades of actionable work. Do not reinterpret the reason.
- PendingExecution, PendingRecovery, PendingReanalysis, ConflictRecorded and
  EvidenceRequested are durable handling success, NOT Synced. Ack is performed
  only by Orchestrator after Ok; Apply is responsible for actual durability.
- No facade receipt cache; each call passes through ports, including redelivery.
  The facade does not guarantee independent work scheduling or serialization.
  Concurrent claims and physical safety remain Apply's responsibility.

## Outstanding integration concerns

This is a specification of the internal composition, not an end-to-end test of
real modules or durable storage. Cancellation of the outer future, feedback
granularity, whole-sync boundedness, evidence sourcing, and recovery wakeups
remain for the consolidated review. Legacy integration tests using the removed
Analyzer API are not migrated by this commit.

## RED

cargo test -p drivesync-engine --test analyzer_contracts analyzer_behavior -- --ignored

AnalyzerEngine intentionally uses todo!(); tests are ignored until deliberately
run. No stage algorithms are added.
