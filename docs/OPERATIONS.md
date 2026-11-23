# Synchronization Operations

Reconstruction step 7 implements the private `orchestrator::operations` boundary.
The Analyzer and executor share one `Operation` and `OperationPreconditions` contract.
Operations do not choose synchronization policy or analyze conflicts. Step 8 now wires the
[Orchestrator and GUI status](ORCHESTRATOR.md); [functional observation producers](OBSERVATION_SOURCES.md) are implemented in step 9.
[Functional correction Stage 2](FUNCTIONAL_CORRECTION_STAGE_2.md) adds metadata-only
identity registration, explicitly authorized adoption, and nested baseline confirmation.
[Stage 3](FUNCTIONAL_CORRECTION_STAGE_3.md) persists execution recovery commands and checkpoints
and reconciles initial creates/transfers before startup admits further work.
[Stage 4](FUNCTIONAL_CORRECTION_STAGE_4.md) connects initial remote directory/download plans,
confirms parent associations before child dispatch, and signals fresh local rounds after publication.
[Stage 5](FUNCTIONAL_CORRECTION_STAGE_5.md) adds initial local-to-remote planning and guarded
continuation of persisted upload sessions from freshly queried server progress. Account database
leases exclude competing execution/recovery; no expired session is silently replaced.

## Execution and provider selection

`DefaultOperatorFactory` checks the configured provider against Accounts metadata before
constructing an executor. Google is the built-in specialist; every other provider returns
`UnsupportedProvider`. There is no implicit Google fallback. Sessions are acquired through
Accounts for each execution and never enter serializable requests, results, or Inventory.
The connection owner retains its account/link lease while Orchestrator awaits execution,
just as it does for its existing remote queries. Factory construction and execution belong on the
engine worker runtime, not the GTK main loop; filesystem and Inventory calls perform I/O.

`ExecuteOperationRequest` carries the normalized command, execution ID, expected Inventory
revision, scoped identity, observed local/remote state, and completed dependencies. The
extra observation fields are stability evidence, not a second decision model. The caller
must refresh those fields and the revision after every completed plan step. Root operations,
missing evidence, unresolved prior executions, invalid dependencies, and account or root
mismatches fail before mutation.

The Google specialist supports upload, download, content replacement, metadata rename/move,
trash, folder creation, and inert Google Workspace desktop links. Local directory creation,
rename, move, replacement, and trash are performed by `operations::filesystem`, not a
filesystem Provider. Metadata mutations and remote replacements require a real strong ETag
and the analyzed remote version. No unconditional fallback or transport-level mutation retry is used. Fresh observations may
authorize a new plan after a known precondition failure.
Remote ancestor walks are bounded and reject cycles, ambiguous parents, and scope escapes.
Destination queries reject incomplete searches and unresolved pagination. Drive permits
concurrent duplicate names; a preflight collision query is not a server-side uniqueness lock.

## Persistence and partial effects

Before material-effect dispatch, Operations commits `Pending`, then `Running`, with optimistic revisions.
Every terminal attempt records either failure, an observed effect, or an uncertain effect.
A lost response, dropped future, process interruption, or failed final database write cannot
be reported as a completed persisted success. `Pending`, `Running`, and
`AppliedUnconfirmed` records block new executions for that item until recovery resolves them.
Reusing any recorded execution ID is rejected; the executor does not blindly replay creates.

`OperationConfirmed` confirms a verified individual effect without inventing a synchronized
baseline. `BaselineConfirmed` is used only when both replicas demonstrably converge, including
known name, parent relationship, type, and matching SHA-256 for binary content. The current
baseline check resolves the configured roots or a persisted directory association at any
supported depth. Parent identity/location/revision dependencies are checked atomically with
the child write; Workspace projection baselines still require their own mapping policy. A successful
individual operation can therefore leave the previous baseline intact. NoOp does not write one.

`RegisterIdentity`, `Adopt`, and `ConfirmBaseline` revalidate using read-only provider calls
and local filesystem reads, then commit one atomic Inventory metadata transaction instead of
creating a pending material operation. Registration leaves `NeverSynced`; adoption additionally
requires explicit session policy and unique equivalent replicas. Known equal sizes/SHA-256
confirm regular files; directories need no binary metadata. Stale anonymous proposals are
rejected by their existing scoped keys before additional remote reads. The current location
index is independent of historical baseline paths, and bare NoOp still performs no write.

Inventory remains a transactional fact store. Its new `OperationConfirmed` transition checks
that an effect was recorded and marked verified; it performs no synchronization analysis.
A persistence failure after mutation returns `Unconfirmed` and preserves the observed effect
in `OperationResult` while the durable running/applied record remains available for recovery.

## Local safety and transfer requirements

Local paths retain Unix bytes. Each directory component is opened without following symlinks;
mutations use directory descriptors and Linux `renameat2`. Relative escapes, absolute paths,
the internal `.drivesync` root, symlinks, and special files are rejected. Exclusive creation
and `RENAME_NOREPLACE` prevent ordinary destination overwrites. Filesystem identity, size,
modification/change timestamps, and available SHA-256 are rechecked against the request.
These checks do not provide a transactional lock against arbitrary concurrent filesystem
writers; callers must treat uncertain outcomes as requiring fresh observation.

Transfers use bounded 256 KiB upload chunks and incremental download reads. Downloaded binary
content must match the provider SHA-256, size, and unchanged remote version before publication.
Anonymous staging requires Linux `O_TMPFILE`. Replacement stages bytes in the destination
filesystem, atomically exchanges entries, and moves the previous bytes to trash. Failed
staging may retain `.drivesync-transfer-<execution hash>` files for recovery; automatic
cleanup of uncertain artifacts is deliberately absent.

Local removals and preserved replacement versions use freedesktop home trash with a restore
receipt. Both trash directories and receipt files are private. Cross-filesystem trash,
unavailable trash, or unsafe directory ownership/permissions is refused without deleting
source content. Workspace links contain a fixed HTTPS Drive URL and no `Exec` entry or
executable mode. Permanent deletion is never used.

Provider, Accounts, GUI and credential storage remain preserved. Connection exposes a separate
explicit initial-association start method; normal dashboard startup does not grant it.
Automated tests use temporary directories, synthetic authorization and loopback HTTP. Real
Engine validation in a Devel Flatpak build sandbox is recorded separately in the Stage 2
report; neither category establishes installed-GUI/package acceptance.

[Stage 6](FUNCTIONAL_CORRECTION_STAGE_6.md) adds verified bound-slot identity refresh for editor
replacement, update-effect recovery, persisted conditional-upload references and canonical
private/public property handling. Unknown incomplete replacements remain blocked. Cross-mount
trash failure preserves the previous bytes in private stages; no destructive fallback exists.

[Stage 7](FUNCTIONAL_CORRECTION_STAGE_7.md) adds pinned, tagged local moves, conditional marked remote moves and read-only recovery for all four location actions. Destination and ancestor changes block confirmation.

[Stage 8](FUNCTIONAL_CORRECTION_STAGE_8.md) persists conflict facts and validates revision-bound choices before normal dispatch. Both divergent file contents are privately archived and checkpointed before replacement; directory choices archive locations without replacing descendants. Archive verification is part of uncertain-effect recovery.
