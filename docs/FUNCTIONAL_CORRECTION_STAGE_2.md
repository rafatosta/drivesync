# Functional correction — Stage 2 identity, adoption and baseline

Date: 2026-11-22 (America/Bahia). Branch: `refactor/engine`.
Starting HEAD: `700035f`, with a clean working tree. References are the supplied
functional correction plan V1.0 and model specification V3.1, identified in
[Stage 0](FUNCTIONAL_CORRECTION_STAGE_0.md), and the implemented
[Stage 1 evidence contract](FUNCTIONAL_CORRECTION_STAGE_1.md).

Status: implemented, validated automatically and exercised against the authorized
real test account within the isolated scope below. This is not installed-GUI
homologation or completion of bidirectional synchronization.

## Behavior

- Context supplies an opaque random registration token for a conclusively untracked
  `File`. `identity.item_id` and `inventory_revision` remain absent for `NotFound`.
  The token becomes a logical identity only through an Analyzer-approved Operations
  write with explicit initial revision `Some(0)`.
- `RegisterIdentity` rechecks a unilateral item and its absent counterpart/destination,
  then atomically records identity and current location with `NeverSynced`, without
  transferring bytes or confirming a baseline. Repeated proposals cannot register
  the same scoped remote ID or occupied local path under another logical ID.
- `Adopt` is executable only for one unambiguous equivalent candidate with proven
  parent correspondence and explicit session authorization. Directory equivalence
  requires directory types, names and corresponding parents, not binary size/hash.
  Regular files require known equal sizes and SHA-256 checksums. Unsupported types,
  unknown/partial states and divergent/ambiguous candidates do not produce adoption.
- `Engine::start_synchronization_with_initial_association(request)` is the explicit
  application entry point. Normal `start_synchronization`, including dashboard
  restoration, keeps authorization false. An active worker cannot silently change
  authorization; stop it before starting the exact scope with the other policy.
  Authorization is not saved and does not survive a new normal worker session.
- Operations rechecks local identity, timestamps, type and content, remote version,
  ancestry and sibling-name collisions using read-only provider calls before
  confirming association. Both Orchestrator and Operations enforce session policy.
- Baselines require verified equivalence and the actual directory association at
  any supported depth. A nested write checks the parent's identity, scoped record
  revision and confirmed local location within the same Inventory transaction.
  Local parent directory identity is checked without requiring historical directory
  mtimes to remain unchanged when children are added.
- `ConfirmBaseline` records revalidated convergence of an already associated item
  when both current sides changed to the same values. A bare `NoOp` has no write plan.
  A confirmed current local location is indexed independently of the old baseline;
  verified intermediate effects can update location without overwriting history.

These metadata-only actions are single Inventory transactions, with no external
mutation to replay and no new pending-operation queue. Recovery for transfers and
other material effects remains Stage 3 work. Existing mutating actions retain their
preconditions; Stage 2 does not plan initial upload/download or deletion.

## Contract refinements and evidence

| Insufficiency | Minimal change | Safety / validation |
|---|---|---|
| An untracked `File` has no persisted ID, while an operation requires an ID | `File::registration_id` is a proposal; `RegisterIdentity`/initial `Adopt` explicitly write revision zero | `NotFound` still carries no logical ID/revision; scoped-key checks before remote I/O plus transactional collision/revision guards prevent duplicates |
| Candidate pairs did not retain parent equivalence | `AssociationCandidate::parents_correspond` | Missing/ambiguous parent evidence blocks adoption; names/projected paths alone never establish identity |
| Adoption had no executable/persistent contract | Metadata actions `RegisterIdentity`, `Adopt`, `ConfirmBaseline`, and explicit session policy on the existing lifecycle | Unauthorized, divergent and duplicate candidates cannot adopt; same bytes/directories/empty files adopt without transfer |
| Baseline path doubled as current location | `InventorySnapshot::local_path`, updated only by Operations facts | Current-location index regression retains historical baseline; inode/path replacement remains conservative |
| Nested confirmation had no atomic parent dependency | `ParentAssociation` carried by metadata convergence and pending-operation baseline confirmation | Parent revision race test rejects the child write; three-level adoption test; Unix path serialization preserves bytes |
| Stale anonymous proposals repeated provider reads after another proposal succeeded | Operations checks existing scoped identity/path keys before revalidation | Real validation exposed the delay; duplicate/stale-proposal tests plus completed real rerun and restart |

No migration, compatibility adapter, data cleanup, credential copy, or schema SQL
change is introduced. The new JSON snapshot contract requires its current-location
field; an old scoped payload that cannot be decoded is unavailable, not missing or
implicitly migrated. Unrelated stored scopes are not rewritten. Accounts, Provider
API, folder configuration, and existing GUI flows are preserved.

## Automated validation

All Rust tools ran in `org.gnome.Sdk//50`, with the development manifest's
`rust-stable` extension, explicit access to the existing Cargo cache, and no network
for automated checks. The wrapper is recorded in Stage 0.

| Command inside SDK | Observed result |
|---|---|
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets --offline --locked -- -D warnings` | Passed |
| `cargo test --workspace --offline --locked` | Passed: 624 tests, 0 failures, 7 ignored |
| `cargo doc --workspace --no-deps --document-private-items --offline --locked` | Passed |
| `cargo test --workspace --doc --offline --locked` | Passed; 0 doctests defined |
| `cargo build -p drivesync-engine --example validate_stage2 --offline --locked` | Passed |

Breakdown: application 20 passed / 6 ignored; engine unit tests 296 passed / 1
ignored; public Accounts integration 1 passed; simulated Provider integration 307
passed. Eleven new regression tests cover session authorization, directory/file
adoption, missing authorization/parent evidence, divergent and duplicate candidates,
concurrent equivalence versus NoOp, durable registration/reopening, stale revisions,
nested directories/empty files, parent revision races, read-only Google revalidation,
and current location independent of baseline. Existing conservative inode-replacement
and incomplete-evidence tests also pass. The same opt-in desktop/Secret Service
checks remain ignored in the standard suite; they were not presented as executed.

## Real test-account validation

Account verified through the Google Drive plugin: `rtosta.coc@gmail.com`.
The same connected account was opened by the Engine through its existing Devel
Accounts metadata and Secret Service session; credentials were not printed or copied.
Fixture writes were performed only in a newly created isolated Drive subtree.

- Remote scope: [DriveSync-stage2-validation-2026-11-22](https://drive.google.com/drive/folders/1T8XRAmiLBOfh5j5ezRQZa3MpncQh8fZt).
- Local scope: `/home/tosta/GoogleDrive/DriveSync-stage2-validation-2026-11-22`.
- Fixtures: three nested directories; equal root text and nested binary; divergent
  same-name text; local-only and remote-only content; two duplicate remote names.
  A zero-byte local file was also observed. The plugin refused zero-byte upload, so
  real empty-pair adoption was not exercised; its automated filesystem test passed.
- Independent local SHA-256 manifest and scoped SQLite reads verified identities,
  baselines, revisions and lack of pending executions instead of relying on GUI status.
- Plugin readback independently verified all nine remote IDs, names and parents.
  A raw plugin download of the nested binary matched all 20 bytes and SHA-256
  `2b192e189cfb801feef94f9169f7a1ee980bcfee3a72becdb9fc7979600599cb`;
  its remote modification time remained unchanged.

| Phase | Observed result |
|---|---|
| Normal startup, no adoption authorization | Three unilateral identities, zero synchronized baselines |
| Explicitly authorized adoption | Eight identities total; five synchronized baselines (three directories, two equal files); three unilateral items remain `NeverSynced` |
| New Engine process, normal unauthorized startup | Same eight IDs, five baselines, revisions and confirmation timestamps unchanged after fresh observation rounds |
| Preservation | All six local files and paths unchanged; divergent and duplicate candidates unadopted; no pending executions |

The first authorized diagnostic reached only three baselines within its original
40-second window. Stale queued proposals were rejected earlier and the rerun
completed the remaining associations, retaining the already stored IDs without
rewriting them. This is evidence of atomic metadata persistence across stopping and
reopening, not a claim of recovery for uncertain material operations.

The diagnostic uses the normal Engine lifecycle and real Scanner/Google sources,
compiled in the prescribed SDK and executed inside the existing Flatpak Devel build
sandbox (`_build/architecture`, app ID `com.rtosta.DriveSync.Devel`, GNOME 50).
It does not inject prepared `File` values or use plugin access as a substitute for
Engine validation. `engine/examples/validate_stage2.rs` is opt-in and never runs as
part of ordinary application startup. The full GUI/package was not rebuilt or
homologated as an installed application in this stage.

Reproduction, with the SDK build wrapper from Stage 0 and an explicitly prepared
isolated fixture of the same shape:

```sh
cargo build -p drivesync-engine --example validate_stage2 --offline --locked
```

The resulting binary was invoked inside the Devel build sandbox with these
arguments, running `unauthorized`, then `authorized`, then `restart` in separate
processes (the first command above must also run inside the SDK):

```sh
flatpak build --with-appdir --share=network \
  --talk-name=org.freedesktop.secrets \
  --filesystem=/home/tosta/Projetos/drivesync:ro \
  --filesystem=/home/tosta/GoogleDrive \
  --filesystem=/home/tosta/.var/app/com.rtosta.DriveSync.Devel/data \
  _build/architecture \
  /home/tosta/Projetos/drivesync/target/debug/examples/validate_stage2 \
  /home/tosta/.var/app/com.rtosta.DriveSync.Devel/data/drivesync/accounts \
  /home/tosta/Projetos/drivesync/data/oauth/development.json \
  /home/tosta/GoogleDrive/DriveSync-stage2-validation-2026-11-22 \
  1T8XRAmiLBOfh5j5ezRQZa3MpncQh8fZt rtosta.coc@gmail.com restart
```

The test subtree and files were retained for inspection; no permanent deletion was
used. The unauthorized phase requires a fresh scope for its zero-baseline assertion.
Subsequent authorized/restart runs reuse this scope without duplicating identities.
The next dependency is Stage 3 durable recovery before enabling new material effects.
No push, PR, release or other remote code publication is part of this delivery.
