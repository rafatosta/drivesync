# Synchronization Orchestrator

Reconstruction step 8 connects the existing contracts and publishes an account-scoped
`SynchronizationStatus`. The Engine owns lifecycle and account leases. Detectors owns
`Observation -> Context -> File`. Only Orchestrator invokes its private Analyzer and
Operations. Accounts, OAuth, Provider, and saved-folder formats remain intact.

## Application boundary

The public Engine API adds `start_synchronization`, `start_synchronization_with_initial_association`, `stop_synchronization`,
`synchronization_status`, and `subscribe_synchronization`. Starting takes an account,
absolute local root, and opaque remote root ID. It derives a stable synchronization scope
from the length-delimited account ID, remote ID, and raw local path bytes; the identity is
never derived from a path alone. The provider is resolved from Accounts, not selected by
the caller. Identical active configuration is idempotent. An active scope must stop before
its roots or initial-association policy can change. Different active accounts cannot configure
overlapping local trees. The initial-association start method grants permission only to that
exact worker session; ordinary/dashboard startup always uses false. Permission is not saved.

Synchronization Inventory files live in a dedicated `<accounts-directory>.synchronization`
sibling directory. Opening a scope neither migrates nor changes Accounts storage. Each
Inventory lookup/write retains account and synchronization-root isolation. Startup validates local access and creates no baseline. Once the worker is installed,
read-only sources acquire their own authorized sessions and publish observations.

Each account has a bounded queue of 32 `File` values and one sequential worker. Private
`Engine::observe` passes normalized observations through Detectors/Context before admission;
raw observations and operation plans are not public application commands. Producers get
backpressure and stopped workers reject further work. [Functional sources](OBSERVATION_SOURCES.md) now supply initial scans, continuous local
polling and Google observation through the same Detectors/Context File facade.

## Plan coordination

For each scoped `File`, Orchestrator invokes the pure Analyzer once:

- `Ready` executes an executable plan in dependency order.
- `NoOp` performs no write and does not manufacture a baseline.
- `Defer`, `Conflict`, `Unsupported`, and `InvalidInput` publish an explicit blocker.
- `Adopt` executes its metadata plan only when the Analyzer and worker session explicitly
  authorize it; otherwise it publishes AdoptionRequired. Orchestrator copies the exact planned
  candidate evidence into the request and never writes Inventory itself.

Each dispatched step gets a cryptographically random execution ID. Its result must match the
execution ID, item, and action and include a verified effect plus a committed, advancing
Inventory revision before any dependent step can run. Confirmed local/remote state, paths,
identities, remote version, and Inventory revision refresh subsequent request preconditions.
The action, destination, and dependency choices remain the Analyzer's plan. Missing receipts,
failed writes, skips, failures, or uncertain effects stop that plan. There is no blind replay,
automatic merge, or invented evidence for an unknown side.

FIFO items captured before an earlier operation may have stale revisions. Operations rejects
those preconditions and the status requests a fresh observation; Orchestrator does not rerun
source reads or infer a replacement plan. Current blockers are held by item identity so an
unrelated NoOp cannot conceal an unresolved item. This is current status, not durable issue
history or a recovery scheduler; durable execution facts remain in Inventory.

## Lifecycle and status

Workers hold the connection owner's read lease throughout an active item's plan. Disconnect
serializes against scope start/replacement, stops the account's worker, waits for its current
operation, then removes credentials through Accounts. Other active account operations or
sign-in attempts can still prevent removal through the existing ownership gate.

Stop closes admission, discards queued unexecuted files, and lets the current operation
produce its real receipt before stopping. No dependent step starts after stop is requested.
Dropping the last worker handle requests stop; an abrupt Engine/runtime shutdown can leave
`Running`/uncertain Inventory records. [Stage 3](FUNCTIONAL_CORRECTION_STAGE_3.md) reconciles
active records before consuming observations on the next scope startup. Unknown effects remain
blocked; failure to enumerate recovery blocks all mutation admission for the scope. A supervisor publishes
`Failed` and releases stop waiters if the worker panics. Failed work is never reported as Idle.

`SynchronizationSubscription` is a coalescing latest-state feed that can be awaited from GLib
without a caller Tokio runtime. It exposes account, lifecycle, actual activity, current item,
measured operation-count progress, typed blocker, and UTC timestamp, without sessions or
provider diagnostics. It is not an event log: rapid intermediate transitions may coalesce.
Idle means no pending plan for observed items; it does not prove whole-tree synchronization.

The dashboard prepares completed saved scopes and subscribes for the selected account.
Selection changes and widget teardown cancel its previous listener. Preparation failure
leaves accounts and saved choices intact. Status text is localized, and progress appears
only when a plan has a known operation count. Folder changes stop and replace the prior
scope before activating the newly saved one.

## Current acceptance boundary

Step 9 now starts functional observation sources and joins current sides only through a known
Inventory identity. Partial sources suspend execution admission. See [source behavior and
limits](OBSERVATION_SOURCES.md). [Functional correction Stage 2](FUNCTIONAL_CORRECTION_STAGE_2.md)
adds atomic identity registration, authorized adoption and baseline confirmation. Unilateral
registration alone does not confirm convergence. [Stage 4](FUNCTIONAL_CORRECTION_STAGE_4.md)
now adds initial remote directory/download plans after registration and requires confirmed
parents before child dispatch. A directory receipt requests a fresh local source round to
re-evaluate waiting children, without an external edit. Unknown counterparts,
ambiguous adoption, missing authorization and unresolved destinations still block explicitly.
Orchestrator does not duplicate Context or Analyzer logic to bypass those limitations.

Coordinator tests use the real Analyzer and controlled Operator receipts. Connection/source
tests use synthetic OAuth, an isolated vault, temporary local data, and loopback HTTP. These
checks do not establish live Google Drive, portal, or installed-Flatpak acceptance. The Stage 2
report separately records real test-account Engine checks inside a Devel Flatpak build sandbox.
Stage 4 additionally records real three-level initial downloads and another Engine process
reopening the same scope without duplicate operations or baseline rewrites.

[Stage 5](FUNCTIONAL_CORRECTION_STAGE_5.md) adds initial remote creation/upload plans. A verified
remote creation wakes the remote producer; the resulting current capture resolves waiting
children against confirmed parent IDs. Startup recovery can continue a validated incomplete
upload session under exclusive account execution ownership and optimistic checkpoints.

[Stage 6](FUNCTIONAL_CORRECTION_STAGE_6.md) invalidates inputs by an epoch sampled before source
collection, wakes both producers after effects or stale preconditions, and rechecks pending
recovery when the remote source returns. Captures started before an effect cannot release the
post-effect barrier; older-than-baseline remote versions defer. NoOp still performs no write.

[Stage 7](FUNCTIONAL_CORRECTION_STAGE_7.md) orders parent records before descendants and executes combined rename/move/content plans using verified intermediate receipts. Descendant path refresh remains a verified metadata transaction.

[Stage 8](FUNCTIONAL_CORRECTION_STAGE_8.md) records incompatible attributes as durable conflicts, merges disjoint attributes through verified receipts, and keeps uncertain effects ahead of new conflict decisions. Choice submission refreshes sources; only verified convergence retires a conflict.

[Stage 9](FUNCTIONAL_CORRECTION_STAGE_9.md) applies explicit per-item projection, name and special-file policies. Unsupported entries do not prevent independent supported items from progressing; raw provider names remain distinct from derived local filenames.

[Stage 10](FUNCTIONAL_CORRECTION_STAGE_10.md) adds opt-in recoverable trash propagation, conservative empty-folder checks and durable trash recovery. Complete absence alone never authorizes deletion; provider scope exit/access loss and local movement without a retained trash receipt preserve data. Normal startup keeps propagation disabled.
