# Synchronization Operations

Reconstruction step 7 implements the private `orchestrator::operations` boundary.
The Analyzer and executor share one `Operation` and `OperationPreconditions` contract.
Operations do not choose synchronization policy or analyze conflicts. Step 8 now wires the
[Orchestrator and GUI status](ORCHESTRATOR.md); [functional observation producers](OBSERVATION_SOURCES.md) are implemented in step 9.
[Functional correction Stage 2](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_2.md) adds metadata-only
identity registration, explicitly authorized adoption, and nested baseline confirmation.
[Stage 3](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_3.md) persists execution recovery commands and checkpoints
and reconciles initial creates/transfers before startup admits further work.
[Stage 4](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_4.md) connects initial remote directory/download plans,
confirms parent associations before child dispatch, and signals fresh local rounds after publication.
[Stage 5](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_5.md) adds initial local-to-remote planning and guarded
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
the child write. Workspace baselines use canonical derived link bytes and projected names with
`GoogleWorkspaceShortcut`, rather than binary content equivalence. A successful
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
Anonymous staging requires Linux `O_TMPFILE`. Replacement stages bytes beside the destination
and publishes them with an atomic exchange. When freedesktop home trash is on the same mount,
the displaced bytes go there with a restore receipt. When the mount differs, as it can in
Flatpak even for the same device number, the old inode stays at the reserved
`.drivesync-transfer-<execution hash>` path. Operations verifies that retained incarnation
and its content before reporting success. The execution journal records its relative path.
The scanner excludes it from synchronization. Retained versions can be recovered manually
and are never automatically removed.

Explicit local removals require a restore receipt. The Engine uses freedesktop home trash
when it is on the source mount. With a separate Flatpak mount, it creates a private
`.drivesync-trash-<32 hex digits>` directory beside the scope root, or inside the root when
the parent is on a different mount and the root is on the source mount. The name is reserved
at every depth, including in other scopes. The Engine persists the original path and the
destination identity before the no-replace rename and verifies the retained item before
confirmation. Unavailable or unsafe trash blocks the operation. Permanent deletion is forbidden.

The private directory contains the retained item in `files` and its original path in the
matching `info/*.trashinfo`. For manual recovery, stop synchronization and use a same-mount
rename that refuses to overwrite an occupied destination. Restoring the local file alone
leaves the remote item in the Drive trash. Workspace links contain a fixed HTTPS Drive URL
and have no executable mode or `Exec` entry.

Provider, Accounts, GUI and credential storage remain preserved. Connection exposes a separate
explicit initial-association start method; normal dashboard startup does not grant it.
Automated tests use temporary directories, synthetic authorization and loopback HTTP. Real
Engine validation in a Devel Flatpak build sandbox is recorded separately in the Stage 2
report; neither category establishes installed-GUI/package acceptance.

[Stage 6](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_6.md) adds verified bound-slot identity refresh for editor
replacement, update-effect recovery, persisted conditional-upload references and canonical
private/public property handling. Unknown incomplete replacements remain blocked.

[Stage 7](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_7.md) adds pinned, tagged local moves, conditional marked remote moves and read-only recovery for all four location actions. Destination and ancestor changes block confirmation.

[Stage 8](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_8.md) persists conflict facts and validates revision-bound choices before normal dispatch. Both divergent file contents are privately archived and checkpointed before replacement; directory choices archive locations without replacing descendants. Archive verification is part of uncertain-effect recovery.

[Stage 9](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_9.md) restricts Workspace actions to verified link creation, local metadata updates and baseline confirmation. Remote shortcuts and unsupported native types are refused. Link publication is checkpointed and recovered without uploading its bytes.

[Stage 10](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_10.md) adds opt-in recoverable trash propagation, conservative empty-folder checks and durable trash recovery. Complete absence alone never authorizes deletion; provider scope exit/access loss and local movement without a retained trash receipt preserve data. Normal startup keeps propagation disabled.

[Stage 11](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_11.md) records installed and real-account acceptance separately from controlled tests. Writer admission binds filesystem identity across restarts and excludes overlapping/same-provider writers; the public synchronization audit requires complete captured convergence rather than an idle queue. Portal-root and default sandbox-trash limits remain explicit.

## Operations observability

### Instrumentation matrix

| Boundary | Work or wait | Available evidence |
| --- | --- | --- |
| Operator admission and preparation | Execution gate, Inventory lease, preconditions, dispatch | Scoped execution and item IDs, normalized action, existing validation result |
| Inventory and Journal | Intent, checkpoint, effect and confirmation writes | Existing write receipt, recorded revision outcome, known or uncertain effect |
| Filesystem | Open and identity checks, anonymous staging, directory creation, moves, replacement and publication | Existing descriptor and mutation outcomes, completed copy byte count from `std::io::copy` |
| Local verification | Hashing regular files and preserved copies | Buffer lengths and existing metadata, SHA-256, integrity result |
| Remote executor | Session acquisition, metadata, destination, ancestor walks and effect readback | Request/page/item counts from existing calls and collections, validated outcomes |
| Download | Header/body response, staging writes, size and digest, revalidation | Received, written and hashed byte counts from existing buffers, size from existing evidence |
| Upload | Session preparation, reads, HTTP submission, acknowledgement and resume | Local reads and contiguous offsets from chunk boundaries and provider receipts |
| Recovery | Inventory enumeration, per-record validation and reconciliation, resume and confirmation | Original execution IDs, checked records, persisted receipts and recovery issues |
| Conflicts and trash | Archive copy and verification, retained-content checks and receipts | Bytes already processed by archive loops, existing checkpoint and mutation outcomes |

### Contracts and lifecycle

Operations binds an `ActivityInbox` to each Orchestrator account and synchronization root.
The mailbox exposes the current set and ordered updates. The supervisor consumes updates
independently of the functional queue and reads active snapshots for internal diagnostics.
Each activity has a monotonic ID, optional parent, scoped execution and item IDs when
available, an opaque display label, action, direction, phase, state, optional progress,
sequence, and monotonic start and update times. A recovery pass owns one child per checked
execution, using the ID from its persisted record. IDs from different inboxes are scoped
to their respective account and root.

The executor installs its reporting scope while polling the functional future. Filesystem,
RemoteExecutor, Journal, Recovery, conflicts and trash start children or report evidence
through the private `Span` contract. The Google specialization translates session,
metadata and transport evidence to the same phases and units. Reporting scopes also
cover synchronous calls in the future and have no background producer tasks.
The existing recovery observer supplies the current item for the public recovery
projection. Its callback carries the presentation fields needed by that projection.

Execution advances through receipt, resource acquisition, validation, preparation,
durable intent, dispatch, verification, effect persistence and confirmation as applicable.
States are running, waiting with a typed reason, and terminal with a typed outcome.
Terminal outcomes include completion, failure, skip, cancellation, unconfirmed effect
and inconclusive recovery. Effect evidence distinguishes no applied effect, applied,
verified and uncertain. The receipt records the outcome of the existing Inventory write.
An effect can be verified and recorded while its subsequent confirmation write fails.
An uncertainty receipt can persist evidence of the uncertainty. A completed child,
including a transfer, leaves the parent execution active. Terminal operation success
comes from the functional result after any required write.

### Progress and limits

- The existing download stream supplies received bytes. Completed staging writes supply
  written bytes. Integrity verification reports bytes processed by SHA-256 or by the
  MD5/SHA-256 evidence fallback. Digest comparison determines verification success.
  A write or integrity failure has a distinct outcome. An absent HTTP content length
  still permits a known total from validated remote metadata. The contract supports
  an unknown total. Google requires existing size evidence for verified binary publication.
- Upload reads, submissions and confirmed positions are distinct. Submissions measure
  bytes passed to the HTTP client. Socket-level delivery is outside that measurement.
  Confirmed progress is the contiguous prefix accepted by the provider. A resumed
  transfer starts at the freshly confirmed prefix. Attempt bytes and attempt speed
  exclude the earlier prefix. Failed requests may have submitted bytes without advancing
  the confirmed position.
- Transfer rates use real received or attempt-submitted bytes over monotonic elapsed time.
  Ancestor and recovery counters use records already visited. Recovery totals derive from
  the enumerated pending records that match the existing recoverable-state filter.
  Directory listing reports client calls, pages returned and the items in the response.
  Destination checks retain their existing single-page completeness requirement.
- Hashes use the Scanner threshold of 1 MiB for incremental publication. The final count
  is available below the threshold. Conflict archive copies use their existing buffers.
  Staged publication keeps the optimized `std::io::copy` and reports its returned count.
  The local counters reflect completed writes because the existing `write_all` interface
  can return a failure after an unreported partial write.

The channel retains up to 128 active activities and 256 recent updates per scope.
A slow or absent reader can miss overwritten records. Capacity failures omit additional
activity and leave the operation running. The 200 ms sampler, scope and identity types,
bounded retention and off-thread diagnostic writer are shared with Detectors. Operations
retains terminal records in the recent update buffer. Detectors uses source generations
and source summary slots. Starts, phase and wait transitions, final progress, and terminal
records publish immediately. Publication occurs independently of the public watch.
The channel discards late updates using activity IDs and sequence numbers, and closes
descendants when their parent or the inbox ends. An abandoned reporting scope retires
its activity. Functional cancellation follows the existing executor and shutdown rules.

Set `DRIVESYNC_TRACE_OPERATIONS=1` for stderr diagnostics. Output uses temporary scope
and item labels, normalized phases, waits, durations, measured progress, and typed outcomes.
The writer formats a bounded batch outside producer locks. Short transport wait transitions
and successful short upload checkpoints are omitted from trace output to avoid per-chunk
logs. The internal mailbox still publishes those waits and receipts. Diagnostic retention
is best effort while stderr is blocked.

### Validation and measured cost

The GNOME SDK 50 workspace passed formatting, Clippy with warnings denied, workspace tests,
private rustdoc generation and doctests. The workspace ran 754 passing tests and ignored 39
by default. The Operations tests cover scope, hierarchy, ordering, cancellation, bounded
retention, a blocked diagnostic writer, detector coexistence, integrity and staging failures,
upload acknowledgement and recovery, conflicts, trash, and unsuccessful Inventory writes.
The live Google Drive and installed Flatpak scenarios require separate acceptance.

The ignored `operations_observability_cost` test runs equivalent fixtures with reporting
enabled and disabled. Select a fixture with `DRIVESYNC_OPERATIONS_FIXTURE` set to `hash`,
`download`, `upload`, or `small`. Set `DRIVESYNC_OPERATIONS_FIXTURE_TELEMETRY=1` to enable
the inbox. The fixtures use one debug test process per run in GNOME SDK 50, alternate the
mode across five pairs, and leave tracing off. They hash a 32 MiB sparse local file, download
and verify 16 MiB over loopback, upload 4 MiB in the existing 256 KiB chunks with a controlled
in-memory journal, or check the identity and content of a 32-byte file 2,000 times.

Wall and CPU values are medians of the five runs. CPU uses process `getrusage` deltas
around the workload. Peak RSS covers the whole test process. Allocation requests are the
medians of three separate runs counting glibc `malloc`, `calloc`, and `realloc`. The
publication total includes wait transitions. The fixture leaves the inbox unread during
the workload and counts retained plus overwritten records afterward.

| Workload | Reporting | Wall (ms) | CPU (ms) | Peak RSS (KiB) | Allocation requests | Publications |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| SHA-256, 32 MiB | Disabled | 435.27 | 434.12 | 17,720 | 929 | 0 |
| SHA-256, 32 MiB | Enabled | 434.81 | 434.29 | 18,296 | 939 | 9 |
| Loopback download, 16 MiB | Disabled | 243.59 | 241.81 | 72,748 | 1,153 | 0 |
| Loopback download, 16 MiB | Enabled | 244.33 | 242.33 | 72,916 | 1,163 | 174 |
| Loopback upload, 4 MiB | Disabled | 69.68 | 68.86 | 34,208 | 5,238 | 0 |
| Loopback upload, 4 MiB | Enabled | 69.64 | 68.84 | 34,608 | 5,248 | 48 |
| 2,000 local checks, 32 bytes each | Disabled | 24.31 | 24.25 | 17,980 | 28,915 | 0 |
| 2,000 local checks, 32 bytes each | Enabled | 30.80 | 30.67 | 18,428 | 34,922 | 16,000 |

The median wall time for the hash and transfer fixtures differed by at most 0.74 ms.
The observed wall-time ranges overlapped between modes for all three workloads.
The short checks added 6.49 ms over 2,000 checks, or about
3.25 µs per check (26.7% at the median). This fixture concentrates activity lifecycle costs.
Reusing the active registry and including the final hash count in its terminal record
reduced the preliminary short-check publication count from 18,000 to 16,000.
Loopback includes test-server overhead. The debug workloads measure local instrumentation
cost and process variability. Production throughput and live provider latency require
separate measurement.
