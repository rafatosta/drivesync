# Orchestrator tests

`engine/tests/orchestrator.rs` exercises the public Orchestrator, unified
Detector, Inventory, Analyzer, and temporary SQLite databases with synthetic
accounts and filesystem roots.

## Covered behavior

- Local and Remote start together with delivery paused during setup; preparation
  starts one consumer and then enables delivery.
- New local files and folders are observed and analyzed without running a
  separate initial Inventory pass or crossing account boundaries.
- Rename, removal, stop/restart, and event recovery use Detector observations.
  Confirmed local absence comes from explicit collector evidence; unknown
  observations do not become deletions.
- Receipt and analysis persistence failures leave batches recoverable through
  release/redelivery, without premature acknowledgement or duplicate receipts.
- Incompatible Inventory identity is preserved; preparation retry,
  cancellation, concurrent preparation, caller-without-Tokio, remote root alias,
  and account isolation are covered.
- Upload and rename decisions remain blocked when evidence or authorization is
  insufficient. Tests preserve local bytes and verified baselines.
- Detector tests cover durable queue behavior, local/remote observation, source
  failures, causal order, leases, re-delivery, and remote recovery.

Application tests exercise the shared account restoration hook and the
developer interface with synthetic accounts. Tests requiring a desktop display,
portal setup, explicit real-account opt-in, or network access may be ignored by
default. No unit test establishes Google Drive integration or packaged-app
behavior.

## Current limits

Changes made locally while DriveSync is closed are intentionally outside this
branch. Tests use a known starting filesystem state and do not treat a missing
observation as proof that a resource is absent. Real provider behavior and the
live GTK application require separate isolated integration environments.
