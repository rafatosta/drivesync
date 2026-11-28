# Inventory

Inventory is the account-isolated persistence boundary for synchronization facts and operation
state. It owns the SQLite connection, typed reads, scoped enumerations, transaction revisions,
and execution lease. Detectors and Operations call its typed facade. Neither callers nor the
Orchestrator inspect SQL or stored records for telemetry.

## Observability contract

Each account and optional synchronization root has an in-memory `ActivityInbox`, shared with
Inventory callers and the Orchestrator supervisor. Reporting is independent of functional work.
It does not change synchronization decisions, results, rollback, revision, lease, or isolation
rules. No database schema, SQL query, index, timeout, retry policy, transaction boundary, or lock
granularity is changed for telemetry.

The inbox holds at most 128 active activities, 128 registered lookups awaiting completion or
promotion, and 256 recent updates. Lookup registrations use preallocated bounded storage; no
task or thread is created per lookup. Capacity exhaustion omits telemetry and increments the
omission count. Recent updates use bounded overwrite storage. Consumers read current activities
and recent updates as snapshots; delivery of every update is not guaranteed. Closing the inbox
cancels active activities, drops in-flight lookup registrations, and ignores late completions.

### Mutex access

Every database access first tries the existing mutex without blocking. An uncontended acquisition
uses the fast path and creates no detailed access activity. On contention, Inventory publishes an
`Access / Waiting / Acquire` activity before using the existing blocking acquisition. After the
mutex is acquired, the same activity changes to `Access / Running`; it ends after the database
guard is released. A poisoned mutex publishes a failed terminal result and preserves the existing
unavailable behavior. No consumer callback runs under the database mutex, and publication does
not wait for a consumer.

The wait activity therefore appears while the caller is still waiting. It reports the total
activity lifetime; the current contract does not expose a separate lock-hold duration metric.

### Lookups and slow-query promotion

Every lookup contributes to consolidated outcome counters. A short lookup creates no detailed
activity. Its bounded in-flight registration records start time, scope, origin, context, and
terminal result. The existing Orchestrator supervisor checks registrations every 100 ms and
promotes those at least 200 ms old to visible `Read / Running` activities. The 200 ms visibility
threshold is distinct from the 100 ms supervisor check interval. This is a synchronous query
duration signal, not percent progress and not evidence of SQLite lock contention.

Completion removes the registration and records the summary under the inbox lock. If promotion
won the completion race, the terminal outcome closes that activity; if completion won, no
activity is created. Cancelled or closed registrations cannot be reactivated by late promotion.
If the bounded registration list is full, that lookup still runs and its outcome is counted, but
it cannot later be promoted.

### Cross-module references

`ActivityReference` identifies a producer module, producer instance, session, account, root, and
activity ID. This makes an ID unambiguous across inboxes and sessions. The type is shared
infrastructure and does not expose Detector or Operations objects to Inventory. Callers pass
optional `ActivityContext` explicitly, including through blocking tasks; Inventory rejects a
parent or correlation reference whose account or root does not match its own scope.

Activity identity, `parent`, and `correlation` have separate meanings. Parent describes a real
hierarchical child relationship. Correlation links work in one flow without asserting parentage.
Neither is inferred from a numeric ID or fabricated when no caller context exists. Detector
Context and Operations execution/recovery paths pass their active scoped references to relevant
Inventory lookups, enumerations, writes, and leases. References are telemetry only, never
functional or persisted identifiers. Debug output omits account and root values.

### SQLite error evidence

When rusqlite returns an SQLite error, Inventory records `SQLITE_BUSY`, `SQLITE_LOCKED`, or other
SQLite failure class and the numeric code, including the extended code when available. Lookup
failures are accumulated in lookup summary counters and attached to promoted lookup activities;
enumeration and write failures are attached to their activities. Existing functional results
such as `Unavailable`, `Error`, and revision conflict remain unchanged; raw SQLite messages are
not emitted.

The classification is retrospective evidence from a returned error. Inventory does not claim
that SQLite is currently waiting, does not add diagnostic queries, and does not add retries or
busy handlers. Internal SQLite waiting that eventually succeeds is not visible as a live wait
with the current rusqlite API and existing connection policy.

| Work | Reporting | Functional result |
| --- | --- | --- |
| Database open | Open, prepare, terminal availability | Existing open result |
| Lookup | Consolidated outcome; visible activity after 200 ms | Found, not found, ambiguous/rejected, unavailable, or error as before |
| Enumeration | Active activity and sampled processed-record count | Complete scoped result or existing error |
| Write | Validation, transaction, apply, commit/rollback, terminal receipt | Existing write status and revision behavior |
| Lease | Acquisition activity and terminal result | Existing lease acquisition outcome |
| Internal mutex | Live wait activity only on contention | Existing blocking lock and poison handling |

Progress for enumeration is a processed-record count with unknown total; it is not a persisted
record count. Updates do not alter `SynchronizationStatus.work`, `pending`, or the public Engine
API. `DRIVESYNC_TRACE_INVENTORY=1` enables bounded asynchronous stderr diagnostics. The diagnostic
writer formats outside producer locks; stderr itself remains best effort and is not part of the
functional path.

## Transactions

`Written` is reported only after the existing SQLite commit succeeds. Revision conflicts and
rejected requests remain functional outcomes. Errors after a transaction starts retain existing
rollback behavior. Telemetry is never proof that synchronization converged and never changes
transaction atomicity or recovery.

## Validation and measurements

Focused tests cover visible mutex waits before release, uncontended acquisition, poisoned mutexes,
slow lookup promotion before completion, completion/promotion races, saturation and close,
typed cross-module references and scope rejection, and SQLite error classification. A controlled
two-connection temporary-database test reproduces `SQLITE_BUSY` at the configured existing
timeout and verifies the functional lookup result is unchanged. Extended `SQLITE_LOCKED` codes
are covered by classifier tests; a reliable real two-connection `SQLITE_LOCKED` reproduction was
not available in this harness.

On 2026-11-28, the Flatpak GNOME 50 / Rust stable test harness ran seven alternating, warmed
rounds of 10,000 lookups against a local SQLite fixture containing 64 registered records. Each
round mixed found and not-found queries. Median wall time was 46,955 ns per lookup with reporting
off, 48,229 ns with activity production and no snapshot reads, and 48,255 ns with activity
production plus current/recent/summary snapshots every 100 lookups. The paired median difference
for production versus no reporting was 1,118 ns per lookup (about 2.4% of this fixture's
baseline). Across the seven samples, lookup ranges were 46,623–48,397 ns without reporting,
47,699–59,060 ns with production, and 48,135–49,698 ns with production and snapshots. The nearest
rank p95 for each seven-sample set is its maximum. Seven alternating runs of 100 64-row
enumerations measured medians of 803,295 ns without reporting and 827,968 ns with reporting;
ranges were 794,962–964,395 ns and 822,471–875,037 ns respectively. These are local measurements,
not proof of a general performance gain or regression; they include request construction and
SQLite work and do not represent provider workloads.

This harness does not measure Orchestrator scheduling, diagnostic stderr output, CPU time,
allocation count, or peak memory. There is no allocator profiler or process-metric harness
available in the project SDK. Consumer snapshots model periodic mailbox reads but are not a
full Orchestrator benchmark. Writes, concurrent reader/writer scaling, multiple-account aggregate
cost, deliberate slow-consumer behavior, and throughput under saturation were not included in
this performance run; functional saturation and isolation tests cover bounded behavior. These
are remaining measurement limits. SDK validation does not establish installed-package or live
provider integration.
