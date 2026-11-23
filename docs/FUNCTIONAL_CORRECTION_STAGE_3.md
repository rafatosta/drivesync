# Functional correction — Stage 3: minimum crash recovery

Stage 3 implements the recovery prerequisites for initial directory creation and binary
upload/download. It does not add the initial synchronization planners from Stages 4 and 5.

## Durable intent and attribution

Before dispatch, Operations persists the normalized command, execution ID, account/provider
and synchronization roots, original replica evidence and preconditions, completed steps,
local destination/parent identities, and reserved remote creation ID. The normalized operation
contract lives in shared private models; Analyzer, Inventory and Operations use the same type.
Native Unix path bytes survive serialization. No OAuth token enters this record.

Google reserves a file ID before dispatch and creates with that ID and the private
`drivesyncExecution` app property. A lost creation response is located by that reserved ID,
then verified using the execution marker, parent, name, kind, content and current local source.
Names and hashes alone never establish execution ownership. A 404 is inconclusive, not proof
that a request did not execute. There is no second creation request during recovery.

Local directory creation and downloaded-file publication tag the staged inode with
`user.drivesync.execution`. Operations fsyncs the stage and parent and commits its identity
before publishing with no-replace rename. Recovery requires both that identity and marker at
the destination, current root/parent identities, unchanged remote evidence and replica
equivalence. Filesystems without the required xattr support fail safely before publication.

Uploads persist the resumable session reference before sending content and checkpoint confirmed
byte offsets. The private reference is needed to query progress and is redacted from Debug;
it must receive the same storage protection as other private account state. It is not an OAuth
credential. Checkpoints use optimistic Inventory revisions; a failed write prevents publication
or further upload chunks. Uncertain stages and transfer references are preserved.

## Startup and outcomes

Before consuming observation work, the scope's Orchestrator enumerates active durable operations
under its connection ownership lease. Operations reads current filesystem/provider facts and
persists a diagnostic with one of three outcomes:

- **NotApplied:** a durable Pending barrier proves no dispatch, or a local destination is absent
  and no publication checkpoint exists. The execution becomes Failed, without a baseline.
- **Applied:** execution attribution and current equivalent replicas are proved. Inventory
  records the recovered effect and confirms a baseline only through the existing atomic parent
  dependency checks. A second process does not rewrite already confirmed operations.
- **Unknown:** missing command, changed root/source/destination, unavailable provider, incomplete
  transfer, unsupported action or revision conflict. The item stays blocked with preserved data.

Enumeration failure blocks all mutation admission for the scope. Recovery never executes the
stored command, retransmits content, creates another session, or cleans uncertain artifacts.
Incomplete/expired transfers and committed but unpublished stages deliberately remain blocked;
a continuation policy and recovery for update/move/trash/Workspace actions are outside this
minimum stage. Observation may continue; it cannot erase unresolved durable execution facts.

## Validation on 2026-11-22

Automated tests cover dispatch barriers, receipt loss, both final-receipt/baseline windows,
reopened databases, zero-byte downloads, staged-but-unpublished data, missing execution markers,
changed scope/root/source, resumable progress queries, concurrent revisions, and failed recovery
admission. A loopback HTTP test actually times out after a creation effect and recovers its
reserved ID with exactly one POST. All Rust commands run in GNOME SDK 50 with rust-stable,
using the development manifest's environment; host Rust is not used.

`cargo test --workspace` passed 638 tests, with 8 ignored by default. One of those ignored
real-account tests was explicitly executed in the separate phases below; the other seven
GUI/keyring opt-in tests were not executed for this stage. Final checks also include
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo doc --workspace --no-deps --document-private-items`, and `cargo test --workspace --doc`.

Real validation used the authorized `rtosta.coc@gmail.com` session through Accounts inside the
Devel Flatpak build sandbox. The opt-in test
`orchestrator::operations::google::live_recovery_tests::real_receipt_loss_and_new_engine_recovery`
ran as three separate processes (`prepare`, `recover`, `verify`). It executed real local folder
creation, binary download, remote folder creation and a two-chunk binary upload, then injected
receipt loss after each real effect. The second Engine recovered four confirmed baselines;
the third preserved them without replay. A never-dispatched intent became Failed/NeverSynced;
a reserved but inaccessible remote creation stayed Running/Unknown/NeverSynced. The receipt
loss was injected by the test backend; it was not a real Google network timeout.

The isolated [Drive fixture](https://drive.google.com/drive/folders/1V9tlTEQ5Nl-bZniBHjTTEg66O4zksFlV)
contains exactly the two remote sources and two created effects, independently listed through
the plugin after both restarts. The plugin downloaded the uploaded binary independently:
307200 bytes matched the original local bytes, SHA-256
`a7d8881521cbb1e4a5ca960198c7907b45625b39d3a7a86368fc8b4ecad01014`.
The fixture remains at `/home/tosta/GoogleDrive/DriveSync-stage3-validation-2026-11-22` and in
Drive for inspection. No unrelated data was changed. This validates Engine/Accounts/provider
integration in the build sandbox, not the installed GUI, portal, or package update path.
