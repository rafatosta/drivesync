# Functional correction — Stage 1 evidence and scope reconciliation

Date: 2026-11-21 (America/Bahia). Branch: `refactor/engine`.
Starting HEAD: `1939868da9e140847cc8c64c0046759b5106be83`, with a clean working tree.
The supplied functional correction plan V1.0 and data-model specification V3.1
remain the references identified in the [Stage 0 baseline](FUNCTIONAL_CORRECTION_STAGE_0.md).
Status: implemented and automatically validated; not homologated with the real service.
Only Stage 1 is implemented here, without repeating the architectural audit or
restarting the Engine. Root `AGENTS.md` governs these source and documentation files.

## Delivered behavior

Engine-owned sources send normalized scoped rounds through `Observation` and
Detectors/Context; the Analyzer still receives only `File`, with no I/O added.
A round binds account, provider, sync-root ID, configured local/remote roots,
source-local generation, coverage and completion time. Empty captures are explicit.
Stale generations, duplicate keys and mismatched scopes are rejected. Complete,
partial and unavailable rounds replace previous evidence; partial and unavailable
rounds cannot infer absence or retain a formerly free destination.

Inventory provides an account/root enumeration including records not seen in the
latest capture, pending operations and conflicts. Enumeration is all-or-error,
validates stored scope/item identity and revision, and exposes no SQL to Context.
A reopened Context confronts persisted records with current captures. Historical
baseline values never become current metadata by themselves. A known local path
missing from a complete scan is absent; an identity observed elsewhere or a
replacement inode at the old path remains unresolved.

Context resolves parents through persisted directory associations and current
directory identity/type evidence, with configured roots as the explicit root
correspondence. It checks local occupancy and remote sibling-name matches, including
remote duplicates. Ambiguous remote projections cannot resolve a free local target.
Name/path projections across an unassociated tree gather candidate pairs only;
they do not merge logical identities or grant initial-association authorization.
Unassociated nested parents remain unresolved for later Stage 2 association work.

For remote IDs absent from a complete selected-tree listing, the Google observer
performs specific metadata reads, using persisted IDs even after restart:

| Evidence | Result |
|---|---|
| Specific metadata confirms `trashed = true` | `Absent` with confirmed trash evidence |
| Readable ancestry terminates outside the selected root | `OutsideScope` |
| 404/not-found-or-inaccessible, wrong ID, missing parents, cycle, inconsistent listing | `Unknown` |
| Permission denied | `Unavailable`, with `PermissionDenied` capability evidence |
| Other provider/network failure | `Unavailable` |

A failed ancestry read stays inconclusive. No deletion is inferred from a change
feed entry or listing omission. Any unresolved or omitted persisted remote identity prevents claiming
that remote name destinations are free. Missing IDs are also rechecked when an
unchanged change feed is drained. Probe reads cancel with the observer request;
per-round listing/change-feed page budgets do not include these additional reads.
Known-ID input is bounded at 100,000 and ancestry at 128 steps per probe.

## Minimal contract refinements

| Insufficiency | Boundary/refinement | Safety and regression evidence |
|---|---|---|
| Per-item observations cannot express an empty full scope, its validity or invalidation | `Observation::Capture(ScopeCapture)` with typed local/remote entries and scope/generation/coverage; Context emits a list of `File` values | Empty, partial, unavailable, stale and crossed-scope capture tests; source/Engine tests. Existing individual observations still describe individual reads, not complete scopes. |
| Lookup cannot find items omitted after restart | Typed `InventoryScopeQuery` and scoped `Inventory::enumerate` | Reopened-record test; multiple account/root and corruption tests. No schema change, migration or baseline write. |
| `File` cannot retain alternative initial pairings without falsely correlating sides | `File::association_candidates` stores current local/remote candidate states | Three-level unassociated tree and duplicate-name tests; IDs and authorization remain unassigned. |
| A fresh remote observer does not know persisted IDs to probe | `RemoteObserveRequest::known_remote_ids` | Restart probe tests distinguish trash, external ancestry, inaccessible IDs and denied access. Sessions stay private; no credentials enter this contract. |

No new synchronization action, initial association, baseline confirmation or recovery
replay is enabled. Those retain the Stage 2+ requirements. Analyzer production code,
Operations, Accounts, Provider API and GUI behavior are unchanged.

## Validation

All Rust commands run in `org.gnome.Sdk//50`, with the manifest's `rust-stable`
extension, existing Cargo cache explicitly exposed, and network disabled. The
reproduction wrapper is recorded in Stage 0. No SDK/dependency installation or
host Rust fallback was used.

| Command inside SDK | Result |
|---|---|
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets --offline --locked -- -D warnings` | Passed |
| `cargo test --workspace --offline --locked` | Passed: 613 tests, 0 failures, 7 ignored (14 new regression tests) |
| `cargo doc --workspace --no-deps --document-private-items --offline --locked` | Passed |
| `cargo test --workspace --doc --offline --locked` | Passed; 0 doctests defined |

Test breakdown: application 20 passed / 6 ignored; engine unit tests 285 passed /
1 ignored; public Accounts integration 1 passed; simulated Google Provider
integration 307 passed. No doctests are defined.

Regression coverage includes empty/local-only/remote-only/both-side trees, three
nested directory levels, associated and unassociated parents, duplicate names and
collisions, partial/unavailable evidence, permission errors, stale generations,
wrong account/provider/roots, inode replacement/movement, reopened missing records,
corrupt enumeration rows and specific remote-ID/ancestry reads. The Engine source
test starts the normal Engine lifecycle with a real temporary three-level local
tree and loopback remote provider, then verifies preserved bytes, GET-only remote
requests and an empty Inventory (no implicit adoption/baseline).

## Limits and continuation

Validation uses temporary filesystem/database fixtures, synthetic authorization,
an isolated test vault and loopback HTTP. No real account/root/credential data was
modified. It does not establish Google Drive or installed-Flatpak homologation.
The same seven opt-in desktop/Secret Service tests from Stage 0 remain excluded.
Captures are in memory and rebuilt at startup; no new persistent checkpoint or
operation queue exists. No atomic whole-tree snapshot or large-tree performance
homologation is claimed.

Stage 2 is the next dependency: persistent initial identity, authorized adoption
and nested baseline confirmation. New unassociated items still defer in the
existing Analyzer. This delivery is committed locally, with no remote publication.
