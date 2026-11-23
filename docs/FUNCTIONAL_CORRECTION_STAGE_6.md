# Functional correction — Stage 6: continuous synchronization

Stage 6 connects existing content-update plans to current observations, repeated source rounds,
post-effect invalidation and recovery. A known eligible item is reconsidered after availability,
permissions, parent associations, destinations or receipts change, without requiring another
content edit. Tree rename/move policy remains Stage 7 and conflict resolution remains Stage 8.

## Current evidence and own effects

Each source samples the scope's invalidation epoch **before collection starts** and includes it
in its capture. Confirmed effects and failed dispatch preconditions invalidate prior inputs and
wake both producers. Orchestrator blocks dispatch from either pre-invalidation capture until
both sources collect again. This prevents an in-flight read, or a newly delivered but already
stale batch, from reversing an effect that the Engine just committed. Within a multi-step plan,
subsequent steps still use only verified receipts and advancing Inventory revisions.

An invalidated remote producer performs a full current recapture instead of relabeling its cached
metadata as fresh. Even with an unchanged Changes cursor, two quiet remote rounds trigger a
recapture, so missing/delayed feed invalidations do not strand eligible items. Remote evidence
older than a confirmed baseline is deferred rather than planned as an opposite-side edit.
Capture generations remain producer-order identifiers; neither generations nor UTC timestamps
substitute for the epoch sampled before collection.

Read failures invalidate coverage and suspend admission. Local root-read failures recreate the
local observer on subsequent attempts. Context/Inventory consolidation failures publish blockers
and leave producers running for retry. Recovery enumeration failures can be retried when a healthy
remote round returns. An uncertain operation is reconciled from current facts before new work;
a proven result requests another post-recovery pair of captures. NoOp never writes a baseline.

## Atomic editor replacement and identity refresh

Context no longer replaces a persisted local identity with an observed inode merely because a
lookup or cache entry exists. For a regular file at an already synchronized slot, it may instead
supply explicit replacement evidence when complete current captures establish all of the following:

- The same persisted ItemId/RemoteId owns that exact path and corresponding parents.
- The old inode is absent from the current local scope and the new identity has one observed path.
- Remote name, parent, type, size and SHA-256 still match the baseline. A later metadata-only remote
  version is permitted; a regressing version or divergent content is not.

Operations independently validates the binding and revision, rechecks the source, rejects a
multiply linked replacement, and performs a complete local rescan before dispatch. A new inode
at another path, the old inode elsewhere, conflicting keys, ambiguous aliases or missing evidence
cannot authorize this refresh. Inode numbers alone never establish identity or a move.

Changed bytes use UpdateRemoteContent and refresh the persisted inode only with a verified effect.
Equal-byte replacements use the existing ConfirmBaseline metadata transaction, preserving ItemId
and RemoteId without another upload. Observation alone never writes an identity or baseline.
This is synchronization of an already bound slot, not implicit adoption of preexisting replicas.

## Update receipts and preserved versions

Remote replacements carry the execution marker through the existing conditional upload path.
The canonical provider boundary maps private/public properties to the official v2 representation,
preserves their visibility, and decodes the v2 property list back into canonical maps. Existing
visible properties are retained when adding the execution marker. Strong ETag checks remain
mandatory; no unconditional write fallback was introduced. See the official
[version comparison](https://developers.google.com/workspace/drive/api/guides/v3versusv2) and
[property reference](https://developers.google.com/workspace/drive/api/reference/rest/v2/properties).
Upload sessions and offsets are now retained for conditional replacements as well as creates.

A lost complete remote-update receipt can be attributed by its original RemoteId, marker, advanced
version and current equivalent content. Read-only final-effect recovery tolerates incidental
timestamp changes only when the pinned inode, size and full SHA-256 still match; transfer dispatch
and continuation retain their stricter source preconditions. A local update tags/checkpoints its new stage before the
atomic exchange. Recovery verifies that staged inode and marker at the current destination,
rechecks the remote source and confirms only proven convergence. Old bytes and uncertain artifacts
are preserved. Tests cover both update directions after the effect but before the final receipt,
including reopening Inventory and preserving the prior local bytes in isolated trash.

Uncertain incomplete conditional replacement sessions remain blocked with their private references;
automatic continuation remains limited to the validated initial-upload policy from Stage 5.
Unsupported actions and divergent/insufficient facts remain explicit RecoveryRequired conditions.
There is no blind replay and no cleanup of uncertain content.

## Capacity and cache behavior

A per-item cache overflow invalidates cached counterparts rather than silently retaining stale
facts. The next complete rounds re-enumerate all persisted items, including absent entries and
items with no new event. A cache miss supplies Unknown until its counterpart is observed again.

Local and remote item budgets start at 100000 and grow on explicit limit failures, up to 1600000.
Collection retries the unchanged scope; it does not evict a portion of the tree and claim Complete.
The replacement-validation rescan follows the same bounded growth. Hard exhaustion, depth limits,
unreadable entries and incomplete provider searches remain partial/unavailable blockers. Reducing
scope or restoring sufficient coverage is required beyond those hard bounds; items are not silently
abandoned. Account workers, epochs, captures, Inventory scopes and execution leases remain isolated.

## Automated validation

All Rust tooling ran in GNOME SDK 50 with rust-stable, matching the Devel manifest:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`: 662 passed, 12 ignored by default.
- `cargo doc --workspace --no-deps --document-private-items`
- `cargo test --workspace --doc`: passed, with no executable doctest examples.

New checks cover post-effect epochs and stale remote versions, replacement evidence and unchanged
bytes, cache eviction and refetch, local/remote limit expansion, quiet-feed recapture, 503/403 source
loss and return, reconsidering unchanged eligible input, two independently scoped account workers,
private/public conditional properties, and both content-update receipt-loss windows. Existing
scanner tests hash bytes independently of size/mtime, and earlier-stage initial-transfer/recovery
checks remain passing. Offline/online and two-account checks use isolated loopback/synthetic
sources; they do not claim that the host network was disconnected or two real accounts were used.

The Stage 6 opt-in real-account test was executed separately. The other eleven opt-in desktop,
keyring and earlier-stage live tests were not rerun for this stage.

## Real-account validation on 2026-11-22

Validation used the authorized `rtosta.coc@gmail.com` session through the public Engine inside
the Devel Flatpak build sandbox. The test
`connection::live_continuous_tests::real_edits_atomic_replacement_permission_return_and_restart_converge`
used the [isolated validated root](https://drive.google.com/drive/folders/1MtMv2oDZfm8wmTt0FhyMaZ_Ou8S5Vq7J)
and `/home/tosta/GoogleDrive/DriveSync-stage6-validation-2026-11-22-validated`.

The sequence covered initial upload, a local content edit with identical size/mtime, atomic editor
replacement, equal-byte atomic replacement without another transfer, an external remote edit,
local read permission loss/return during another remote edit, and a local edit after stopping the
Engine. A new process found that last edit through initial captures, without a fresh user event;
another process then preserved the complete scoped row without another transfer or baseline write.
ItemId and RemoteId were retained. Six material operations were Confirmed: initial upload, three
remote content updates and two local content updates. Historical failed precondition attempts
from the debugging runs remain in the audit history; they were not deleted or counted as success.
Five-second unchanged observation windows asserted exact Inventory stability after own effects.

Independent plugin listing confirmed one remote binary, ID `1mPWk0yWu5z1DLOUP-1JL44HoDRj9DQRf`.
The plugin downloaded its complete eight bytes and independently matched the final local content,
`restart6`, SHA-256 `0cf700c0c58432b762eb09ded298bf3cb4c485c4f403b7b79365dc3f3976c1c8`.

Earlier isolated runs exposed the conditional-property decoder error, a stale remote capture
behind a newly confirmed baseline, and a metadata-only version change during replacement refresh.
Those failures preserved data and motivated the regression checks and fixes above. The earlier
roots with the base name and `-final` suffix are preserved for inspection; they are not described
as fully converged acceptance runs. The validated root continued its already existing atomic edit
without another content edit after the fixes, then completed the remaining checks.

The build sandbox places GoogleDrive and the actual application data/trash directory in different
bind mounts. A preserved isolated rename probe returned errno 18 (EXDEV), confirming the boundary.
Consequently the two local replacements published their verified new inodes but could not commit
the old versions into home trash. Recovery verified current convergence without retransmission;
`atomic03` and `remote04` remain preserved as eight-byte private `.drivesync-transfer-*` stages.
No uncertain artifact was removed. This validates recovery with preserved old bytes, not successful
cross-mount trash transport. The extra five-byte ignored probe is also preserved. Normal same-mount
trash and preserved-version receipts are covered by the isolated executor test. The Flatpak trash
transport limitation remains explicit; no destructive fallback was added.

Permission changes were restricted to this test file and returned to mode 0600. No unrelated local
or remote content was changed. These checks exercised real Google and Engine behavior; they did
not click through the GTK window or validate installation/update of a distributed package.
