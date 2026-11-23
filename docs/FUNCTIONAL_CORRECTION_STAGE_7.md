# Functional correction — Stage 7: tree renames and moves

Stage 7 connects location plans to complete scoped observations, durable parent dependencies,
verified intermediate effects and restart recovery. It preserves ItemId and RemoteId while
relocating an established tree in either direction. It does not authorize deletion, infer identity
from a name/hash, or introduce Stage 8 conflict-resolution choices.

## Identity and current paths

LocalIdentity includes optional nanosecond filesystem birth time. The scanner and executor obtain
it from filesystem metadata. Device/inode/mount remain auxiliary scoped keys; a new path requires
an equal known birth time as additional incarnation evidence. Missing birth time blocks that
correlation. No migration, database reset or identity inference for old records was introduced.

Context leaves strict Inventory key lookup unchanged. It separately reconciles persisted records
against complete current captures: the old path must be unoccupied, exactly one captured entry
must carry the same incarnation, exactly one persisted owner may claim that identity, and the
new path must have no other persisted owner. Recycling an inode, partial coverage, multiple
aliases, occupied old slots and unknown remote counterparts cannot authorize relocation.
A current-path index resolves moved parent directories without repeatedly searching the tree.

Operations independently rechecks a changed path, persisted identity/revision, a complete bounded
scanner round and regular-file link count. Hardlinks, including aliases outside the scope, block
movement. An item leaving scope is not treated as a rename destination or deletion request.

## Parent dependencies and intermediate receipts

Complete batches place parents before descendants, including reopened Inventory records that
appear only through remote observation. This is necessary: repeatedly attempting a child before
its parent's new durable path is confirmed would invalidate captures and starve the parent.
Descendant locations are refreshed through verified metadata confirmation; baselines are not
rewritten merely from an observation. Each confirmation advances the revision and invalidates
older plans. Confirmed destination associations are required before moving into a newly created
parent. Unknown destinations and collisions remain explicit blockers.

Combined changes execute rename, move, then content replacement. Each step has its own durable
command, verified readback and receipt. Later steps use the receipt's current path and remote
version. Corresponding equal location attributes are compared independently, allowing a restart
after a rename/move receipt to continue remaining content work without manufacturing a concurrent
edit conflict. A baseline is confirmed only when both current replicas and their parent association
converge. Other concurrent divergent attributes still block execution.

The scanner checks rename permission on the directory reached by its pinned descriptor (`fd/.`),
rather than on the procfs descriptor symlink. A read-only `/proc` mount in the Devel sandbox must
not be reported as a read-only synchronization directory.

## Dispatch and recovery

Local moves retain no-replace rename semantics. Source/destination parent identities are pinned,
the source incarnation and link count are checked, and an execution xattr is flushed before the
rename. Parent directories are flushed afterwards. Source/destination identity and bytes are
revalidated; an inconclusive post-effect read leaves an uncertain effect, never a confirmed baseline.

Remote renames/moves preserve existing properties and write the private execution marker through
the existing conditional v2 metadata boundary. A strong validator and fresh readback remain
mandatory. Ancestor location facts are checkpointed before location effects and checked
before dispatch, after the effect and during recovery. Metadata-only ancestor version advances are not location changes: Google can advance a folder version
when a child is moved into it. Changed/inaccessible ancestor locations prevent
confirmation. The configured local root is pinned separately; the configured remote root is the
scope boundary rather than an ancestor inferred from a name.

Recovery supports RenameLocal, MoveLocal, RenameRemote and MoveRemote without replaying commands.
Local effects require the tagged destination, same incarnation/bytes, absent source and unchanged
pinned parents. Remote effects require the execution marker, advanced version, requested location,
unchanged non-target attributes, unchanged source and stable ancestry. A still-unchanged source
can prove NotApplied. Missing pre-dispatch ancestry checkpoint proves dispatch did not begin.
Other ambiguous states remain recovery blockers, preserving source data and durable intent.

## Validation

Validation completed on 2026-11-22–23, on `refactor/engine`. All Rust checks ran in
GNOME SDK 50 with the rust-stable extension specified by the Devel manifest:

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: **668 passed**, 13 opt-in tests ignored by default
  (application 20, Engine 340, account API 1, provider integration 307).
- `cargo doc --workspace --no-deps --document-private-items`: passed.
- `cargo test --workspace --doc`: passed, zero examples.
- The explicitly selected real-account `verify` test passed in a separate process
  (1 passed, 346 filtered), including its six-second full-Inventory stability assertion.

Final remote fixture: [DriveSync-stage7-validation-2026-11-22-validated](https://drive.google.com/drive/folders/1r2o9IslL-XPCR24V5JUZHvMGoSYIU7Zz).
Local fixture: `/home/tosta/GoogleDrive/DriveSync-stage7-validation-2026-11-22-validated`.
All four records have Synced baselines, current paths and only confirmed material operations:

| Current local path | Preserved remote ID | Confirmed material actions |
| --- | --- | --- |
| `returned` | `1gquwCMcg6BoskRKjAH_L5Vi3qbAyI19t` | CreateRemoteDirectory, RenameRemote, MoveRemote, RenameLocal, MoveLocal |
| `returned/sub` | `12S2BvVI7LOrKnKyPmg13CWjt1jnZ6Ljj` | CreateRemoteDirectory |
| `returned/sub/new.bin` | `1jjnfucqsL1EKn_EW6t0h4Kv4enJefcmQ` | Upload, RenameRemote, UpdateRemoteContent |
| `destination` | `1BfD0p7hDXZUTx6R7f35gVadBH8eNy2qw` | CreateRemoteDirectory |

Independent connector listings confirmed the remote organization. A raw download through the
connector matched the complete local bytes `tree0002` (8 bytes), SHA-256
`04425eeaf8e8428da86534cc5dbcadb5b3a9fefc39bc687ef6fba21a8966d853`.
No expiring download URI or credential is included in the report.

An earlier isolated `...-final` fixture also exercised real uncertain-effect recovery:
MoveRemote had already succeeded, but a destination folder version advance from 1 to 2 made
its receipt uncertain. Readback proved unchanged ancestor location, remote version 3 and the
matching execution marker. After correcting the overly strict ancestor-version check, recovery
confirmed that same effect without replay; subsequent local moves and descendant confirmations
converged. The final `...-validated` scope has no unresolved operations.

Logs retained outside the repository:
`/home/tosta/Documentos/ChatGPT/drivesync 2/relatorios/etapa-7-20261123/`.
Temporary logs from the earlier interrupted session were not retained across the environment
restart; current workspace checks and the separate real restart verification were rerun.

The opt-in test is `connection::live_tree_tests::real_tree_moves_edits_new_parent_and_restart_preserve_identities`.
It checks an initial three-level tree, a stopped-scope local rename+move into a newly created parent,
a renamed+edited descendant, remote rename+move back to the root, original identities, converged
baselines for every descendant and a quiet interval. A separate `verify` process checks restart.

Automated regressions cover recycled birth evidence, aliases, partial/outside captures, parent-first
ordering, intermediate location receipts with remaining content work, filesystem collision and
hardlink rejection, replaced destination parents, lost receipts for all four location actions,
no replay and concurrent remote ancestor changes. Existing transfer/recovery suites cover the
content step and source/version failures.

Real validation uses `rtosta.coc@gmail.com`, the existing Devel credentials and
`flatpak build --with-appdir` in `_build/architecture`, with isolated roots only. This executes the
public Engine lifecycle with actual Google requests and local filesystem effects. It does not
claim installation/update of a Flatpak package or GTK interaction testing.

No remote or local test fixture is deleted. Initial diagnostic runs are retained as evidence of
the parent-ordering and procfs-capability bugs found and corrected during this stage.


## Remaining boundaries

Filesystem birth-time evidence and execution xattrs are required for the supported safe movement
paths; unsupported filesystems remain explicit blockers. Hardlinks and uncertain correlations do
not fall back to a name/hash match. Conflicting concurrent changes remain blocked pending Stage 8.
Projection/type policy and trash propagation remain Stages 9 and 10. No installed-package or GTK
interaction acceptance is claimed by these Engine and connector checks.
