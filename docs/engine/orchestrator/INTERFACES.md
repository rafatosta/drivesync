# Orchestrator — operational contract

## Composition and preparation

`OrchestratorCreateRequest` receives a unified `DetectorCreateRequest` and the
policy flag that allows the Analyzer to dispatch operations. `Orchestrator::new`
opens the Detector's durable queue and leaves delivery paused. `prepare`
performs the following work off the GTK thread:

1. Open the account-scoped Inventory and bind the Detector's durable causal
   order source used by operation verification.
2. Start Local and Remote with delivery paused, waiting for Local to report an
   operational state.
3. Start the single event consumer, restore pending revalidation and operation
   work, and resume event delivery.

Preparation does not enumerate the local root, synthesize an initial baseline,
or run global analysis to make up for missing observations. It returns after
the consumer starts. `PreparationStatus` reports lifecycle state, the most
recent preparation error, and the in-memory count of durably processed events.
Neither preparation readiness nor the event count means physical
synchronization completed.

`PreparationFailure` distinguishes Detector, Inventory, Analyzer, invalid
observation, concurrent preparation, and internal failures. Error display does
not expose file payloads. The Detector's status and diagnostics remain
available through the unified public facade.

## Observation projection

- Validate `File`, `SyncId`, and the account on remote references.
- Resolve local observations by a known local ID or the Inventory path index;
  resolve remote observations only by their account-qualified reference.
- Persist the original `File`, original event ID, causal position, and source
  proof through the public `record_local` or `record_remote` API.
- Forward explicit local presence evidence. `AbsentConfirmed` requires the
  collector's validated lookup result. Missing metadata, coverage gaps,
  inaccessible paths, and unassociated locators remain unknown.
- Use the Inventory's durable receipt before revision checks on redelivery.
  The Orchestrator does not open Inventory SQL or Detector storage.
- Keep remote root diagnostics as Detector state. Do not turn technical
  notifications into file records.

An observation that affects known items is sent to the Analyzer with current
Inventory snapshots. Whole-scope notifications and existing revalidation
semantics may request analysis of current Inventory items. They are event-driven
or recovery work; preparation never performs a compensating global pass.

## Acknowledgement and recovery

The consumer takes bounded batches from the Detector queue and renews their
lease while processing. `Processed` acknowledgement follows successful
Inventory persistence and Analyzer work. A guard releases the whole lease on
failure, cancellation, or panic. There is no partial acknowledgement or
discard. If either database or the process fails between writes, Detector
redelivery and Inventory receipts allow interrupted work to resume.

The consumer dispatches persisted localized revalidation requests through the
Detector's generic reobservation API. It also revisits pending operation
recovery on the existing retry schedule. It does not inspect strategy
implementations or coordinate individual producers.

## Lifecycle and concurrency

One worker belongs to each prepared session. A new preparation invalidates the
previous epoch, retires the old worker off GTK, and prevents stale workers from
publishing readiness or acknowledging work. Stop pauses delivery and stops both
sources while preserving queued observations/checkpoints. Wakeups only signal
availability; the Detector remains the single queue owner. A heartbeat revisits
expired leases, and the lease guard protects in-flight batches.

## Scope boundary

Inventory owns persisted synchronization state; Analyzer owns reconciliation,
conflict decisions, and operation dispatch policy; Operator executes and
verifies requested operations. Neither depends on Local or Remote strategy
internals. Retrospective detection of local changes made while the application
was closed is not implemented here. Future observation producers belong inside
the Detector and use the same generic `File` contract; the Orchestrator remains
independent of their startup and scheduling.
