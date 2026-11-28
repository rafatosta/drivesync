# Inventory

Inventory is the account-isolated persistence boundary for synchronization facts and operation
state. It owns the SQLite connection, typed reads, complete scoped enumerations, transaction
revisions, and execution lease. Detectors and Operations call its typed facade; neither the
Orchestrator nor those callers inspect SQL or database records for telemetry.

## Observability contract

One in-memory `ActivityInbox` is created for an account and optional synchronization root. The
connection lifecycle shares it with the initialization read, Detectors' Context, Operations, and
the Orchestrator supervisor. The inbox is independent of the functional work queue. It retains at
most 128 active activities and 256 recent updates, orders each activity by sequence, wakes a
nonblocking consumer, rejects updates after closure, and retains a bounded current/recent snapshot.
Progress uses the shared monotonic sampler at 200 ms. A full inbox omits telemetry only.

Activity IDs are process-local to the inbox. Parent and correlation IDs are optional; current
Inventory producers leave them empty because the synchronous typed calls do not receive a stable
parent token from the caller. `origin` distinguishes initialization, Detectors, and Operations.
Scopes before a root is known carry only the account; rooted events retain the configured root.
Diagnostic labels are one-way process-local hashes; no paths, account identifiers, record payloads,
SQL, credentials, provider tokens, or cursors are written.

| Work | Reporting | Actual measurements |
| --- | --- | --- |
| Database open | Open storage, prepare, terminal availability | Existing open and schema preparation outcome; no additional checks |
| Lookup | Count every call; publish individual activity only at 200 ms or longer | Functional outcome: found, absent, rejected/ambiguous, unavailable |
| Enumeration | Active activity and sampled loop progress | Records processed; total remains unknown; no count query |
| Write | Validation, transaction, apply, commit or rollback phase; terminal receipt | Identity, baseline, pending operation, recovery checkpoint/assessment, effect, confirmation, conflict or failure category; functional write status |
| Execution lease | Acquisition and terminal result | Existing lease acquisition outcome |
| Internal mutex | Separate access activity after release | Wait duration only when at least 1 ms |

The typed states are running, waiting for the internal mutex, complete, not found, rejected,
unavailable, rolled back, failed, and cancelled. Phases cover storage open/preparation, access,
query/record reading, validation, transaction/apply/commit/rollback, and lease acquisition. Progress
is a processed-record count with an unknown total; processed counts are not treated as persisted
counts. Small lookup outcomes are consolidated as found, absent, rejected, and unavailable.

Updates and results are read from the Inventory inbox and do not alter `SynchronizationStatus`,
`SynchronizationStatus.work`, or `pending`. `DRIVESYNC_TRACE_INVENTORY=1` enables bounded asynchronous
stderr diagnostics. Diagnostics are emitted by the consumer, not while holding the database lock.

## Transaction semantics

`Written` is reported only after the existing SQLite commit succeeds. Revision conflicts and
rejected requests remain functional rejections. Errors after opening a transaction are reported as
rolled back when the transaction is dropped without a successful commit. Telemetry never serves as
proof that synchronization converged and never changes rollback, revision, lease, or isolation
rules.

## Validation and limitations

The activity contract has focused tests for ordered lifecycle events, closure, scope distinction,
and bounded active/recent storage. Database functional tests continue to cover existing lookup,
enumeration, and write behavior. SDK validation uses the project's Flatpak GNOME 50 and Rust stable
extension. SDK tests do not exercise installed-package or live-provider integration.

On 2026-11-28, a controlled fixture ran seven alternating, warmed rounds of 10,000 lookups against
the same SQLite database with reporting off and on. In the final run, median wall time was
31,060 ns per lookup without reporting and 31,538 ns with reporting; the median paired difference
was -567 ns. Repeated runs varied in both directions, so this small local `NotFound` fixture does
not establish a reliable overhead estimate. It includes request construction and local SQLite work,
and does not represent provider or production workloads. CPU, allocation count, and memory were not
measured.

Caller activity correlation is not yet populated, and SQLite busy/locked errors remain folded into
the existing functional unavailable results. Lookup duration activities are retrospective when
the synchronous call returns; a blocked thread cannot publish a live wait transition. These limits
preserve the current synchronous contracts and avoid additional queries or a new async runtime.
