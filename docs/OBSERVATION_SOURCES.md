# Functional observation sources

Reconstruction step 9 implements Scanner, continuous local observation, and the Google
RemoteObserver on the existing `Observation -> Context -> File -> Analyzer -> Operations`
contracts. [Functional correction Stage 1](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_1.md) extends the
original delivery with scoped captures, Inventory enumeration and candidate evidence.
Execution requests and Inventory schemas are unchanged. Public status also exposes display-only
collection telemetry, independent of normalized capture evidence. Producers read facts;
only Operations writes execution effects or baselines. Google calls continue through the preserved Provider API.

## Lifecycle and admission

Starting a completed account scope starts one local task and one provider-selected remote
task. Local collection runs on blocking workers; Google awaits bounded async Provider calls.
Both tasks deliver normalized observations through the same Detectors/Context facade and
use the coordinator's bounded queue for backpressure. They do not own another operation queue.

The first local scan publishes Scanning. Partial or unavailable sources publish a Waiting
blocker and suspend new execution dispatch even when the Analyzer can otherwise form a plan.
Fresh complete observations reopen admission. Existing operations still finish through their
normal verified receipt path; source activity cannot bypass execution preconditions.

Stop cancels collection and coordinator admission together, then waits for source tasks and
any in-flight operation receipt.
Session acquisition can refresh and persist credentials, so it retains the connection owner's
lease until Accounts finishes; it is not abandoned midway through credential persistence.
Remote read requests and polling sleeps cancel cooperatively. No source task retains Engine
ownership in a cycle. Unexpected task failures are supervised and published as failures.
Disconnect stops both sources before removing the account. Abrupt process termination can
still leave the existing operation recovery records; observation itself creates no baseline.

## Scanner and continuous local observation

The Scanner pins the configured root and opens every directory component without following
symlinks. It reports relative native paths, including non-UTF-8 bytes, entry kind, device/inode
identity, millisecond mtime/ctime, size, capabilities, and SHA-256 for stable regular files.
SHA-256 reads use 64 KiB buffers and compare file identity, size, and nanosecond mtime/ctime
before and after reading, including a final named-entry check. ctime is not creation time.
Symlinks and special entries are described but never traversed or hashed as regular files.

The root `.drivesync` directory and `.drivesync-transfer-*` staging names are excluded.
Directory names and observed identities are queued rather than keeping a descriptor open for
every sibling. Limits are 100,000 entries, depth 128, and 256 retained diagnostics per scan.
Permission errors, instability, and limits yield Partial; root loss/replacement is unavailable.
A partial scan does not replace the last complete snapshot or manufacture removal evidence.

The continuous local source wakes on native filesystem events, a refresh request, or a
two-second poll. The observer computes initial Scanner Snapshot events and later Watcher Created, Modified,
MetadataChanged and Removed facts. Removal requires a complete current scan omitting a
previously observed path. Engine source tasks deliver full scoped captures, retaining events
for present entries; Context also confronts persisted paths with these captures to describe
missing known entries without manufacturing events. An unavailable scope invalidates prior
current evidence, without declaring entries absent.

On eligible filesystems, inotify identifies dirty paths for incremental verification. Watch
coverage loss, namespace changes that need re-evaluation, and periodic audits trigger a full
scan. Stable native files can reuse recently verified hashes. Large trees and uncached files
can take longer than the polling interval. Renaming is observed as an old path disappearing
and a new path appearing. Verified coverage describes the read horizon, and Operations recheck
current state before a mutation.

## Google specialization

The registered Google specialist receives an Accounts-acquired, account-bound opaque session.
Unsupported provider identities have no Google fallback. Initial and Recovery modes capture
the selected tree breadth-first using `files.list`, retaining actual IDs and parents. Folder
shortcuts are not followed. Root identity, folder type, trash state, and list capability are
validated before and after a completed capture. Entries with inconsistent parents, duplicate
IDs, inaccessible folders, incompleteSearch, repeated tokens, or resource limits cannot
complete a capture. Partial rounds preserve internal continuation without publishing a false
complete tree or missing-item facts. After a partial Initial/Recovery round, Incremental
resumes that in-memory capture; an explicit Initial/Recovery request starts a fresh capture.

A change checkpoint is obtained before listing, closing the initial capture/change-feed gap.
Incremental rounds drain `changes.list` from that checkpoint. A complete feed of verified leaf
updates can refresh known metadata directly. Structural or incomplete evidence and the periodic
audit trigger a scoped recapture. Completed empty feeds reaffirm known unchanged metadata and
republish it to permit reconsidering previously deferred items.

A previously observed or persisted ID missing from a completed selected-tree capture is
queried individually, including after restart. Confirmed trash is Absent with Known(true)
trash evidence; OutsideScope requires a readable ancestry chain terminating outside the
selected root. A missing/inaccessible response remains Unknown; permission denial and other
request failures remain Unavailable. A listing omission alone never proves deletion. These
reads do not authorize propagation to trash or local removal.
A cursor expiry or read failure requests a fresh Recovery capture; source loss invalidates
previous current evidence. Unknown capability attributes remain NotCollected, malformed
numeric/digest/time attributes are Unavailable, and Workspace-only attributes are NotApplicable.

Some edited Office files expose MD5 but no SHA-256. For a downloadable binary with known
size and version, the observer streams its bytes, verifies length and Drive's MD5, and checks
the remote revision again before recording the computed SHA-256. Later captures can reuse
that digest for the same file version. The operator verifies its download against this
SHA-256 and rechecks the remote revision before replacing the local file.

Each round reads at most 16 listing/change-feed pages by default (hard round cap 64).
Specific missing-ID and ancestry probes are additional reads, bounded by the known-ID limit
and 128 ancestry steps per missing item, and cancel with the observer request. Captures retain
at most 100,000 items and 100,000 pages; feed continuation history is also bounded. Successful
incremental polls wait 15 seconds; unfinished captures continue after 50 ms. Errors retry after
15 seconds, with cooperative cancellation during reads. Cursors and source views are currently
in memory only. Restart performs a fresh capture; no observation checkpoint is written to
credentials, an ad hoc database, or Inventory without a defined checkpoint contract.

## Context consolidation and current synchronization limits

Context retains up to 100,000 current pairs, joining sides only when Inventory already
correlates them to the same scoped ItemId. Cache eviction leaves evidence unknown rather than
inventing a counterpart. Baselines never stand in for current observations. Source tasks
serialize each producer's delivery; UTC wall-clock values are diagnostic, not sequence numbers.
The next observation after source loss replaces its unavailable side after recovery. Inventory
lookup results, revisions, pending operations, and conflicts are freshly read. Engine sources
now deliver `Observation::Capture` rounds, including empty/partial/unavailable scopes, with
source-local generations and configured account/provider/root bindings. Context rechecks
persisted records absent from the capture, confronts confirmed current locations with new
evidence, and rebuilds destinations and collisions. Historical baselines never substitute
for current observations. It shares derived indexes
and one scoped enumeration within each round; correlated items are emitted once per round.
A complete local scan may prove a known path absent, but an observed identity at another path
keeps the logical item's state Unknown. Remote omissions stay Unknown without a specific
probe. Partial/unavailable rounds invalidate old capture evidence. Captures are not atomic
snapshots; Operations must still revalidate preconditions.

New same-name local and remote files are not associated by this cache. Current tree projections
only gather candidate pairs; duplicates remain explicit candidates/collisions. Directory
associations from Inventory resolve destination parents in either direction; unassociated
nested parents remain unresolved until their association is confirmed by adoption or a
successful initial creation. Only Synced directory parents without active uncertain operations
resolve child destinations. No source
or Context writes an association or baseline. [Stage 2](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_2.md) now
provides Operations-based registration and authorized adoption; ambiguous candidates,
unresolved moves/destinations and uncertain operations still defer. This step makes observation functional; it does not claim complete
initial upload/download or full conflict recovery. Local replacement receipts now retain
the confirmed inode identity, preventing the next scan from conflicting with its own effect.

Validation uses temporary files, synthetic account authorization, an isolated vault, and
loopback HTTP. It covers pagination, nested folders, cursor expiry, cancellation, coverage,
local recovery, identity-based pairing, and Engine lifecycle. No real Google account, document
portal integration, installed Flatpak, or native desktop acceptance is established by SDK tests.

The Stage 2 report separately records real test-account source/Engine validation inside a
Devel Flatpak build sandbox, including three-level adoption and re-opening without duplicate
identities or baseline rewrites. It does not claim installed GUI or portal acceptance.

[Stage 3](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_3.md) separately validates initial-effect recovery in a
new real-account Engine process. It introduces neither the initial synchronization planner nor
permission to associate preexisting items implicitly.

[Stage 4](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_4.md) adds initial remote directory/download plans.
Confirmed directory publication wakes the local source immediately; a fresh capture re-evaluates
remote children without requiring an external filesystem edit or additional remote change.

[Stage 5](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_5.md) also wakes remote observation after verified remote
creation. Complete collision-free captures prove absence for registered NeverSynced local items
without a remote ID; partial/unavailable captures cannot authorize a create or upload.

[Stage 6](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_6.md) carries pre-collection invalidation epochs. Own effects
force current remote recapture, and quiet Changes rounds periodically recapture as well. Cache
overflow invalidates counterparts and soft item limits grow on retry; hard limits remain explicit
coverage blockers. Synced-slot replacements require absent old inodes and unchanged remote binary
evidence, followed by independent Operations validation.

[Stage 7](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_7.md) correlates a changed path only with complete captures, a unique persisted incarnation and filesystem birth evidence. Rename permission checks follow the pinned descriptor into the actual directory, including inside Flatpak.

[Stage 8](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_8.md) permits complete, unambiguous bound-slot replacement evidence to expose simultaneous atomic-editor divergence as a conflict. This never authorizes an overwrite: the selected choice, current filesystem, provider state and original-inode absence are rechecked by Operations.

[Stage 9](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_9.md) applies explicit per-item projection, name and special-file policies. Unsupported entries do not prevent independent supported items from progressing; raw provider names remain distinct from derived local filenames.

[Stage 10](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_10.md) adds opt-in recoverable trash propagation, conservative empty-folder checks and durable trash recovery. Complete absence alone never authorizes deletion; provider scope exit/access loss and local movement without a retained trash receipt preserve data. Normal startup keeps propagation disabled.

[Stage 11](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_11.md) records installed and real-account acceptance separately from controlled tests. Writer admission binds filesystem identity across restarts and excludes overlapping/same-provider writers; the public synchronization audit requires complete captured convergence rather than an idle queue. Portal-root and default sandbox-trash limits remain explicit.

## Detector observability

The Detectors facade binds one activity inbox to the configured account and synchronization
root. Sources own producer sessions. LocalObserver translates Scanner events, and the remote
observer translates provider activity through its common contract. Context reports shared work
and retains its link to the delivery or observation that triggered it. The Orchestrator receives
the read side through `ActivityInbox`. It can read the current set during an operation and
projects source summaries through its existing status supervisor.

```text
Scanner -> LocalObserver -----+
Provider -> RemoteObserver ---+-> Detectors activity inbox -> Orchestrator status supervisor
Sources ---------------------+
Context ---------------------+

Observation -> Context -> File -> Orchestrator functional admission -> Analyzer / Operations
```

Every activity has an identifier within its worker, an optional parent, a local, remote, or
shared origin, a phase, and a state. The scope binds the set to an account and root. Running
and waiting states carry progress and a root-relative display position when available.
Waiting classifies the resource or event involved. A progress value has a unit, an observed
count, and an optional total derived from metadata or an existing collection. The sequence
orders updates for the same activity. Start and update times use a monotonic clock. Completion,
failure and cancellation release the activity. Source generation changes retire the previous
generation, so a late sample cannot reactivate it.

The mailbox holds at most 128 live activities and two source display summaries. Producers
use short critical sections for updates and release them before notifying the supervisor.
Progress is sampled per activity at 200 ms to limit contention and path construction in hot
loops. Starts, waiting transitions, final counts and outcomes bypass that interval. Concurrent
samples are accepted by sequence. The public watch holds the latest compatible values and
may coalesce intermediate updates. Producers never acquire the public watch lock or await a
telemetry consumer. The bounded functional queue still provides backpressure on file admission.

### Instrumentation

Paths in the table are relative to `engine/src/detectors`.

| Activity | Instrumentation | Contract and observed data |
| --- | --- | --- |
| Source preparation, startup and idle waits | `sources.rs`, `local`, `remote`, `supervise` | Sources reports waits for the blocking worker, refresh or filesystem events, a retry, credential acquisition, ownership, and continuation. |
| Receiving observations and queue admission | `sources.rs`, `deliver`, `deliver_files`, `track_admission` | Sources counts observations received, admitted files, and the size of each batch. A pending admission future reports a capacity wait. |
| Directory traversal and dirty-path reads | `observation/scanner/mod.rs`, `scan`, `scan_dirty`, `collect_entry` | Scanner traversals report relative position, visited entries or dirty paths, completed entries, completed directories, snapshot size, and reused checksums. The public in-flight count still tracks selected entries. |
| Hashing and watch recovery | `observation/scanner/mod.rs`, `hash`, `digest`, `scan_dirty` | Scanner reports SHA-256 bytes for files at least 1 MiB using the existing buffers and metadata size. The local contract reports why a native watch needs a rebuild. |
| Local snapshots and recovery | `observation/local/mod.rs`, `poll`, `snapshot`, `diff`, `unavailable` | LocalObserver reports materialization, comparisons, event association, and recovery batches against their known collection sizes. |
| Remote request and collection | `observation/remote/interfaces.rs`, `observation/remote/google/mod.rs`, `observe`, `round`, `root`, `begin` | RemoteObserver reports access and checkpoint reads, provider and state-lock waits, requested and received pages, collected items, and the completed and pending directory counts. The pending count describes the current frontier. |
| Incremental feed and recovery | `observation/remote/google/mod.rs`, `round` | RemoteObserver reports requested and received pages and actual changes examined across the logical feed. The compatibility projection uses pages from the current round and known items. Recovery exposes normalized continuation, coverage and budget reasons. |
| Remote response processing and evidence checks | `observation/remote/google/mod.rs`, `round`, `probe`, `fill_sha256`, `result` | RemoteObserver reports validation, materialization, normalization, known IDs checked, ancestry depth and metadata or checksum-response waits. |
| Context consolidation, indexing and reuse | `context/mod.rs`, `context/capture.rs`, `build_capture_with`, `View::new`, `same_evidence`, `republish` | Context reports existing observations and files processed, records and identities indexed, comparisons, lookups, and context lock waits. |
| Inventory and audit | `context/capture.rs`, `snapshots_with`, `known_remote_ids`, `context/audit.rs`, `audit_with` | Context reports Inventory access and lease waits, records validated, candidate IDs selected, and convergence checks. |

Google keeps one collection activity across partial rounds. The full-capture page count is
cumulative, and page budget controls a round. Full capture can safely resolve display paths
from already observed ancestry. The account-wide feed may have a different count of known
items and examined changes. Remote API or checksum response waits expose elapsed time.
The specialization uses existing requests and response processing for its measurements.

### Compatibility and evidence

`SynchronizationStatus.work` retains the source path, item and page counters, reading,
complete, retry scheduled or failed scan state, and the retry timestamp. A completed scan
displays the count of returned observations. Public Analyzer history retains the last item
actually analyzed and its cumulative counter. The supervisor updates only source fields, so
concurrent publications preserve Analyzer history and the first timestamp and missing
evidence associated with `pending`. The display projection never participates in capture
epochs, source health admission, Analyzer decisions or confirmed Operations receipts.

A partial round can display progress while its functional coverage remains partial.
An unexpected source exit retires its generation and projects Failed. Stopping the
Orchestrator clears public work and pending data and closes the activity inbox.

### Diagnostics and extension

Set `DRIVESYNC_TRACE_DETECTORS=1` before starting Engine to emit internal diagnostics on
stderr. Records include process-local account and root labels, origin, activity and parent IDs,
phase, state, elapsed time, progress, and terminal outcome. Account, root and relative item
labels use a process-local keyed hash. Credentials, provider cursors, raw errors and path
strings are absent from this output. The ring holds up to 256 records per scope. Summary
counters report coalesced samples, stale updates, expired buffered records or rejected
activities at capacity. Diagnostic output uses one process-wide in-flight write. Slow or
blocked output may miss records while the current activity snapshot and public projection
continue to advance.

A new observation activity should define a phase and useful existing measurements in its
own module. Translate the phase at its parent boundary, scope the activity to its real
lifetime, and leave functional results unchanged. Remote providers implement
`RemoteObserver::observe` with an optional `RemoteActivity` argument beside the scoped
request. Specialists use the shared remote units and sanitized reasons while retaining the
provider's responses and continuation data privately. Context and Scanner report at existing
iteration points. Blocking Context work can finish after its owning activity scope closes.
The inbox discards those publications. Per-item Context lookups are counted at batch level,
and checksum byte reporting is sampled for large files. Operation-owned precondition scans
use the read-only Detectors facade outside the source activity sessions.

### Validation and performance

The workspace suite in GNOME SDK 50 went from 737 passing and 35 ignored at `f01d472` to
748 passing and 38 ignored by default. Regression tests use isolated local directories,
loopback provider responses, and controlled queue and operation waits. They cover
intermediate and final counts, provider and queue waits, multiple activity origins,
bounded retention, scope changes, cancellation, failure, and the public status projection.
The GUI, public Engine types, and D-Bus integration are unchanged in the implementation diff.
The controlled run used no live account or installed Flatpak.

Four ignored fixtures were also run explicitly. The tree fixture creates 100,001 entries,
exercises the soft-budget partial read, then performs the complete retry. The checksum
fixture reads a 64 MiB sparse file. The remote fixture serves 6,400 items across 64 loopback
pages and four rounds. Each wall time, CPU time and peak RSS is the median of three runs in
the debug profile. The tree operation time is the complete retry. Total CPU includes setup,
and allocation requests count process-wide `malloc`, `calloc`, and `realloc` in a separate
run. Diagnostic output was disabled throughout.

| Fixture | Activity | Operation wall time | Process CPU | Peak RSS (KiB) | Allocation requests |
| --- | --- | ---: | ---: | ---: | ---: |
| Local tree at `f01d472` | Baseline fixture | 1,349 ms | 3.298 s | 167,708 | 5,367,045 |
| Local tree | Disabled | 1,368 ms | 3.345 s | 171,084 | 5,367,045 |
| Local tree | Enabled | 1,393 ms | 3.379 s | 171,756 | 5,367,077 |
| SHA-256 | Disabled | 903.0 ms | 0.904 s | 15,876 | 348 |
| SHA-256 | Enabled | 897.2 ms | 0.898 s | 16,464 | 357 |
| Loopback capture | Disabled | 87.6 ms | 0.133 s | 35,132 | 301,987 |
| Loopback capture | Enabled | 94.0 ms | 0.143 s | 35,072 | 301,999 |

The observed tree added 32 allocation requests compared with the current unobserved fixture.
The remote fixture added 12 requests and took 6.4 ms more at the median. The checksum timing
varied across runs. These figures describe the controlled debug fixtures and include
process and operating-system variability. Production throughput and live provider latency
remain unmeasured.
