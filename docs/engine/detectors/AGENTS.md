# Detector agent guidance

## Responsibility

Detector unifies Local filesystem observations and Remote provider observations
behind one durable event queue and one generic `File` contract. Inventory,
Analyzer, and Operator do not depend on strategy internals.

- `start` starts Local and Remote together. Do not expose a start-one-origin
  public API.
- `pause/resume` suspend or enable delivery only. Observation and durable
  storage continue during a delivery pause.
- `stop` stops both sources and pauses delivery while preserving queued events
  and checkpoints.
- Preserve observation IDs, payloads, causal positions, and explicit
  uncertainty. Partial results and failures never prove deletion.
- `status` reports aggregate state plus individual Local, Remote, delivery,
  queue, and error information without hiding a healthy source.
- A paused `take_events` fails without leasing a new batch. Existing leases may
  still be acknowledged or released.
- Access service APIs only through Providers. Do not make Detector depend on
  Inventory or Analyzer.
- Keep queue persistence, cursor safety, and redelivery behavior across
  restarts.

## Future observation producers

Changes made locally while DriveSync is closed are outside the current scope.
A future retrospective local-observation producer belongs inside Detector,
uses the generic `File` contract, and will be implemented from scratch. Keep
the Orchestrator unaware of that producer's strategy and lifecycle.

## Validation

Use fake source contracts, synthetic filesystem roots, and isolated SQLite
databases. Cover simultaneous origins, durable redelivery, idempotence,
failures, concurrency, restart, and remote recovery. Follow the root guidance
for Flatpak-only Rust verification.
