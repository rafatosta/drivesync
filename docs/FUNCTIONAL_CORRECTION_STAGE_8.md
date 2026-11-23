# Functional correction — Stage 8: persistent conflicts and real resolution

Stage 8 persists incompatible replica facts, exposes revision-bound choices through the public
Engine and connects dashboard buttons to actual operations. A boolean `resolved` value is never
overwrite permission. This stage does not add a deletion/trash propagation policy or Workspace
projection support; incompatible/absent/trashed types remain explicit blockers.

## Detection and durable evidence

The Analyzer compares content, name, parent, kind, existence and trash evidence against the
confirmed baseline. Incompatible changes to the same attribute become conflicts. Binary MIME
changes remain the same broad regular-file kind; conversion to a folder or native Workspace type
is a kind incompatibility. Insufficient evidence defers processing rather than proving a conflict.
Equivalent final replicas confirm metadata, and disjoint concurrent attributes merge through
ordered, independently verified location/content receipts instead of becoming false conflicts.

Each conflict records its own ID, ItemId, account/root scope, observed Inventory revision, local
and remote states, diagnostic reason and update time. Complete current scope captures are required to create
or replace those facts. Source-unavailable observations cannot replace an existing conflict's evidence.
Changed conflicting evidence creates a new review ID and discards any obsolete choice. Repeated
unchanged observations do not rewrite the record. Persistence failures are surfaced as operation
failures instead of claiming that a conflict was saved.

Unique initial collisions are persisted with NeverSynced baselines and no transfer or adoption.
Only an explicit choice may subsequently associate and replace their contents. Multiple ambiguous
candidates remain blocked; duplicate-name projection policy belongs to Stage 9. Atomic editor
replacements at an already synchronized slot may also expose divergent content as conflict
evidence, without silently replacing the persisted inode. A selected replacement still requires
full scope, old-inode absence, unique current identity, exact slot binding and no hardlink alias.

## Choices, freshness and execution

`Engine::list_conflicts(account_id)` reviews the selected account's configured synchronization
scope. `Engine::resolve_conflict` accepts a conflict ID, its displayed revision and one typed choice:

- **KeepLocal:** apply the local replica's supported name, parent and content to the remote replica.
- **KeepRemote:** apply the remote replica's supported name, parent and content to the local replica.

Choices are available for present regular binary replicas and corresponding directories. A folder
choice changes location only; it never plans a binary content replacement for the directory.
Missing/trashed replicas, incompatible kinds, unsupported native types and inconclusive identity
remain blocked rather than acquiring implicit deletion or conversion permission.

A choice pins the exact reviewed local identity/path/type/hash/timestamps and remote
ID/name/parents/type/hash/version. Optimistic revisions and the account execution lease serialize
it against dispatch. The request is scoped to the current account/root; an unrelated account or
stale revision cannot select another item's conflict. Source rounds are explicitly refreshed after
a choice. Operations checks the persisted token, direction, target and current evidence before
using normal filesystem and conditional-provider preconditions. No raw application operation or
baseline override is exposed.

Rename, move and content steps keep separate durable receipts. Only verified effects advance the
choice's accepted intermediate evidence. Pending or uncertain effects take priority over new
conflict decisions and are reconciled before more work. Verified whole-item convergence retires
the conflict and clears the choice atomically with the baseline. Independent observations never
set `resolved` as authorization to overwrite.

## Preserved versions and recovery

Before a selected file effect, Operations writes both original byte streams beneath:

```
<local-root>/.drivesync/conflicts/<conflict-id>/local.bin
<local-root>/.drivesync/conflicts/<conflict-id>/remote.bin
<local-root>/.drivesync/conflicts/<conflict-id>/manifest.json
```

The manifest retains original states/locations, hashes, lengths and the review ID, so copies stay
identifiable after later conflicts replace the active Inventory entry. Descriptor-anchored private
directories require the current owner and mode 0700; archive files use mode 0600, no symlink
following and no-replace publication. Length and SHA-256 are checked, files/directories are flushed,
and the manifest is published only after both verified copies exist. Existing originals are never
overwritten. Partial stages remain preserved on failure. Internal artifacts are excluded by the
existing scanner rules.

Directory choices preserve both original location states in `directory.json`. Moving or renaming
that directory preserves its descendants' contents; no tree replacement, merge or destructive
fallback is performed. Existing movement collision and ancestor checks remain in force.

An explicit ConflictArchive checkpoint commits completed preservation before a selected material
effect. A crash before that checkpoint proves NotApplied. Once committed, a missing or modified
archive is an Unknown recovery blocker. A lost effect receipt is reconciled by the existing
execution marker, conditional version and filesystem publication evidence, without replaying the
command. Incomplete conditional replacement sessions keep the existing explicit recovery blocker;
this stage never silently restarts or resumes an uncertain conditional upload. Normal replacement
trash/staging safeguards remain, in addition to the conflict archive.

## Dashboard integration

The selected account's dashboard exposes **Review conflicts**, showing persisted paths and the
incompatible attributes. **Use local version** and **Use Drive version** submit real Engine
requests with the displayed revision. Buttons cannot double-submit. The UI distinguishes choice
recording from completed resolution, detects subsequent invalidation, and asks for a new review
when evidence changes. After convergence, **Open preserved copies** opens the private recovery
directory through the desktop file launcher. It does not automatically launch a file manager.

Account selection resets the review target. Existing dialogs retain the account they explicitly
show. Portuguese strings are included and checked with msgfmt. No simulated activity or conflict
resolution is used by this flow.

## Validation evidence

All Rust tooling ran exclusively in GNOME SDK 50 with the rust-stable extension specified by the
Devel manifest. The final check commands are:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo doc --workspace --no-deps --document-private-items`
- `cargo test --workspace --doc`
- `msgfmt --check` for `po/pt_BR.po`

Workspace tests: **680 passed**, 15 opt-in tests ignored by default (application 20, Engine 352,
account API 1, provider integration 307). The dedicated real Engine/GTK tests were selected
explicitly in the Devel build sandbox, using the signed-in authorized account `rtosta.coc@gmail.com`.

Automated regressions cover content×content, independent concurrent attributes, moved parents,
incompatible kinds/native conversion, trash×edit, missing evidence, equivalent final states,
boolean-only resolution, stale versions, initial collisions, atomic replacements, persistent
reopen, untagged dispatch, folder-only choices, archive tampering/symlinks/no-replace publication,
pre-archive crash windows, uncertain-effect priority and lost-receipt recovery without replay.

Real isolated fixtures:

| Fixture | Evidence |
| --- | --- |
| [GUI final](https://drive.google.com/drive/folders/19yCuYJjrwq86s8i3ryBCQDXRu1wayyuk) | Separate-process conflict persistence; actual GTK button activation; KeepLocal convergence; both original files and original metadata archived |
| [Remote choice](https://drive.google.com/drive/folders/1YqwMghkMf6vsB82TmblZLBx6nuiSA6C6) | KeepRemote convergence; an uncertain local replacement was recovered as Applied; originals remained archived |
| [Initial GUI run](https://drive.google.com/drive/folders/1XorQxn8g4vUF4u3oUmFS2OTLR7w9jqZW) | An obsolete initial review was invalidated before any replacement; a fresh GTK review succeeded; separate restart verification stayed quiet |

Local roots use the corresponding folder names under `/home/tosta/GoogleDrive/`.
The final GUI fixture retains ItemId `44cf04acdf4d6ac8d9188f24abd540ae`, RemoteId
`1ksb6bgLXoBiSoqpkxZ_2vp_y6Hw4wPe7`, resolved conflict
`0eb14c94d7d9092d7023b3bea4111566` and confirmed Upload/UpdateRemoteContent receipts.
Its private archive contains `local008` and `remote08`. Independent connector downloads matched:

- GUI final/current local bytes `local008`: SHA-256 `98aa25e030363c74111dcf6b780dc44b9f4ad896bf4e20ea8182f13036648c08`.
- Remote choice/current local bytes `remote08`: SHA-256 `8f3bdbfcbbde0b70dba2ac2ec84472fcadb384a55d803c4b685a8b2fa23d96ee`.

The GTK test creates real widgets on the available display and emits the real choice button's
activation signal; its connected handler invokes the real Engine and verifies actual files.
This is functional GTK integration evidence, not a claim of manual visual, accessibility,
installed-package or Flatpak-update acceptance. Folder conflict choices are covered by automated
planning/archive tests; real directory movement execution was validated in Stage 7.

No unrelated files, credentials or test fixtures were removed. The existing cross-mount local
trash limitation can retain private old stages after replacement; the conflict archive is
independently complete and recovery does not delete those stages. Projection policy and safe
trash propagation remain Stages 9 and 10.


Validation logs are retained outside the repository at
`/home/tosta/Documentos/ChatGPT/drivesync 2/relatorios/etapa-8-20261123/`.
