# Functional observation sources

Reconstruction step 9 implements Scanner, continuous local observation, and the Google
RemoteObserver on the existing `Observation -> Context -> File -> Analyzer -> Operations`
contracts. [Functional correction Stage 1](FUNCTIONAL_CORRECTION_STAGE_1.md) extends the
original delivery with scoped captures, Inventory enumeration and candidate evidence.
Public status, execution requests and Inventory schemas are unchanged. Producers read facts;
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

The continuous local source rescans two seconds after the preceding round finishes. The
observer computes initial Scanner Snapshot events and later Watcher Created, Modified,
MetadataChanged and Removed facts. Removal requires a complete current scan omitting a
previously observed path. Engine source tasks deliver full scoped captures, retaining events
for present entries; Context also confronts persisted paths with these captures to describe
missing known entries without manufacturing events. An unavailable scope invalidates prior
current evidence, without declaring entries absent.

This is polling, not an inotify journal: transient changes between rounds can coalesce.
Renaming is observed as an old path disappearing and a new path appearing. It does not assert
a rename from potentially recycled inodes or invent a kernel move cookie. Every round reads
regular-file content for hashing; large trees or large files can therefore take longer than
the polling interval. Scan completeness describes a successful traversal, not an atomic
filesystem snapshot. Operations must still recheck current state before a mutation.

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
Incremental rounds drain `changes.list` from that checkpoint. Account-wide changes are
invalidation signals, not evidence of item deletion: after any changes, the specialist
recaptures the selected subtree. This safely handles ancestor moves and permission changes
without imposing Google models or cursor semantics on Context. Completed empty feeds reaffirm
known unchanged metadata and republish it to permit reconsidering previously deferred items.

A previously observed or persisted ID missing from a completed selected-tree capture is
queried individually, including after restart. Confirmed trash is Absent with Known(true)
trash evidence; OutsideScope requires a readable ancestry chain terminating outside the
selected root. A missing/inaccessible response remains Unknown; permission denial and other
request failures remain Unavailable. A listing omission alone never proves deletion. These
reads do not authorize propagation to trash or local removal.
A cursor expiry or read failure requests a fresh Recovery capture; source loss invalidates
previous current evidence. Unknown capability attributes remain NotCollected, malformed
numeric/digest/time attributes are Unavailable, and Workspace-only attributes are NotApplicable.

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
or Context writes an association or baseline. [Stage 2](FUNCTIONAL_CORRECTION_STAGE_2.md) now
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

[Stage 3](FUNCTIONAL_CORRECTION_STAGE_3.md) separately validates initial-effect recovery in a
new real-account Engine process. It introduces neither the initial synchronization planner nor
permission to associate preexisting items implicitly.

[Stage 4](FUNCTIONAL_CORRECTION_STAGE_4.md) adds initial remote directory/download plans.
Confirmed directory publication wakes the local source immediately; a fresh capture re-evaluates
remote children without requiring an external filesystem edit or additional remote change.

[Stage 5](FUNCTIONAL_CORRECTION_STAGE_5.md) also wakes remote observation after verified remote
creation. Complete collision-free captures prove absence for registered NeverSynced local items
without a remote ID; partial/unavailable captures cannot authorize a create or upload.
