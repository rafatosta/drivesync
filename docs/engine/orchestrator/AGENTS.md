# Orchestrator agent guidance

## Responsibility

The Orchestrator composes the public Detector, Inventory, and Analyzer
interfaces. It starts one continuous observation consumer and does not know
Local or Remote strategy internals. The Detector owns the only durable queue.

- Preparation opens Inventory, starts Local and Remote with delivery paused,
  restores pending revalidation/operation work, then consumes observations.
- Preserve `File.id`, payload, causal position, and account-qualified remote
  identity when recording observations.
- Confirm a batch only after durable Inventory processing and Analyzer work.
  On failure or cancellation release the lease and allow redelivery.
- Never infer an absent resource from missing metadata, incomplete coverage, or
  an unassociated path. Preserve explicit checks before destructive operations.
- Analyzer owns reconciliation, conflicts, and operation policy. Operator owns
  execution and verification. Do not move those decisions into the consumer or
  GUI.
- Keep GTK free of blocking I/O, retain session epochs and safe stop/restart,
  and keep availability signals separate from event delivery/acknowledgement.

## Scope and validation

Changes made locally while DriveSync is closed are outside the current scope.
Do not add a compensating global analysis during preparation. Any future
retrospective observation producer belongs inside Detector and uses the
generic observation contract; Orchestrator must not launch or coordinate it.

Use temporary roots, synthetic identities, real module facades, and isolated
SQLite databases. Cover re-delivery, injected persistence failures,
stop/restart, account isolation, and the absence of physical side effects.
Execute Rust verification only through the repository's Flatpak manifest.
