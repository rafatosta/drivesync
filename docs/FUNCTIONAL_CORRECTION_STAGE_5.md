# Functional correction — Stage 5: initial local-to-remote synchronization

Stage 5 connects initial local directory/upload plans to the existing Operations journal,
reserved Google IDs, resumable transport and verified Inventory convergence. Ordinary
`Engine::start_synchronization` can now materialize a local binary tree in an initially empty
remote root. Existing replicas still require explicit adoption permission; name/path/hash alone
do not authorize associating them. Initial planners now work in both directions. Continuous
update policy and broader deferred-item scheduling remain Stage 6.

## Evidence and parent dependencies

Context proves a remote destination absent only from complete current capture coverage and an
empty name query under a uniquely resolved root or confirmed parent association. It reconstructs
this evidence for registered NeverSynced local items that still have no remote identity. It
carries corresponding local and remote parents without inventing a remote item or baseline.
Unknown/partial coverage, unresolved parents and collisions block initial materialization.

The pure Analyzer emits `RegisterIdentity -> CreateRemoteDirectory` or `RegisterIdentity -> Upload`.
Already registered items resume planning at the material step. Binary uploads require known
size, a valid SHA-256, a stable local identity and readable source evidence. Operations rechecks
the destination through the authenticated provider immediately before creation, and verifies
confirmed parent associations before initial mutation. Parent revisions remain part of atomic
nested baseline confirmation. Drive does not offer a name uniqueness transaction: another
external client can race a preflight query; duplicate observations remain blocked rather than
silently associated or overwritten.

Verified remote creations wake the remote producer for a fresh round, so newly confirmed parent
IDs resolve waiting children without an external edit. Local sources remain read-only. Idle
retains its existing meaning and does not assert whole-account convergence.

## Recoverable upload execution

Before dispatch, Operations persists the normalized command, expected evidence, scope, execution
marker and reserved remote ID. Creation uses that ID and the private `drivesyncExecution` property.
The existing final readback checks ID, marker, name and parent. Verified upload completion also
requires matching binary size and SHA-256. The source is revalidated before starting transfer;
its open descriptor's identity/size/timestamps are checked around every content request and the
original source is rechecked before reporting success.

The normal and recovery paths share the same bounded 256 KiB transfer loop. Upload sessions and
acknowledged offsets are committed before content continuation. These private references are
redacted from Debug; OAuth credentials are never copied into Inventory.

Startup recovery first attempts read-only attribution by the reserved ID, marker and current
replica evidence. A recovered complete effect confirms convergence without another upload. For
an incomplete initial upload, recovery may continue only when all these checks pass:

- The original source, scope, local root and parent remain valid and its parent association is confirmed.
- The same saved session is queried through the provider's URI validation; no replacement session is created.
- The server reports a valid aligned prefix, at least as long as the durable acknowledged prefix and shorter than the known total (or zero for an empty upload).
- A fresh optimistic checkpoint commits that prefix before any further content request.
- Final readback attributes the result to the original reserved ID and proves equivalence before baseline confirmation.

Running and unverified AppliedUnconfirmed uploads support these transfer checkpoints. A verified
applied effect is never retransmitted. An OS `flock` lease on the account's Inventory database
excludes simultaneous execution/recovery from independent Inventory instances and releases on
cancellation/process exit; optimistic revisions additionally reject stale writes. The lease is
storage ownership, not a second queue, credential store or synchronization policy.

After a completed continuation, recovery allows three bounded readback attempts, separated by
250 ms, to obtain stable completion evidence. It retries observations only. Unresolved facts,
expired sessions, source changes, collisions, regressing/malformed progress and failed writes
retain their diagnostics, session reference and blocked state. There is no blind create retry,
no cleanup of uncertain data, and no automatic replacement of an expired session.

## Automated validation

All Rust tooling ran inside GNOME SDK 50 with rust-stable, matching the Devel manifest:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`: 649 passed, 11 ignored by default.
- `cargo doc --workspace --no-deps --document-private-items`
- `cargo test --workspace --doc`: passed; there are no executable doctest examples.

New tests cover anonymous and registered upload plans; incomplete evidence and collisions;
interruption after an actual first loopback content request and durable checkpoint; database
reopening and continuation from byte 262144; expired sessions; changed sources between runs and
during a content request; regressing progress; delayed readback metadata; lost final responses;
and exclusion of a competing Inventory instance. Request counts assert exactly one create,
exactly one first block and no redundant suffix transmission. Existing Stage 3/4 identity,
recovery, initial download and failure tests remain passing.

The two Stage 5 opt-in real-account tests were explicitly executed separately. The other nine
opt-in GUI/keyring/earlier-stage live tests were not rerun for this stage.

## Real-account validation on 2026-11-22

Validation used the authorized `rtosta.coc@gmail.com` session through Accounts inside the Devel
Flatpak build sandbox. Local and remote fixtures were isolated and preserved for inspection.
No unrelated data was changed and no permanent-delete operation was used.

The public Engine test
`connection::live_initial_upload_tests::real_local_tree_converges_and_reopens_without_duplicate_uploads`
started with an empty [remote root](https://drive.google.com/drive/folders/1bCeg7uOubP2jMZpFwx1ZLrC0k1r05zXr)
and this local tree:

```text
root.bin                 31 bytes
a/
  empty.bin               0 bytes
  b/
    c/
      large.bin     8388608 bytes
```

All six items reached Synced baselines with one confirmed material operation each and an Idle
status without an issue. A separate Engine process reopened the same scope and preserved every
Inventory row, identity, baseline, execution and revision. Independent plugin listings confirmed
all six items without duplicates. The plugin then downloaded `large.bin`: its complete 8388608
bytes matched the local source, SHA-256
`7d212b9c884f5c77896de960ae17cc341cda43b14d6a971f34ca29ebd4badf7f`.
The local fixture is `/home/tosta/GoogleDrive/DriveSync-stage5-validation-2026-11-22`.

The test
`orchestrator::operations::google::live_upload_recovery_tests::real_partial_upload_recovers_the_same_session_in_a_new_engine`
used separate prepare/recover/verify processes. Prepare sent a real 262144-byte block of a
786432-byte source, persisted its acknowledgment, then injected an interruption at the checkpoint.
The real service remained available; this was not a real Google outage or process kill.
Another Engine queried and continued the saved session, retained the original reserved ID,
confirmed all 786432 bytes and one material execution, and wrote the Synced baseline. A third
process preserved the confirmed row without rewriting it or transmitting content.

An early [resume fixture](https://drive.google.com/drive/folders/11cdOmQssNM7qvjNtHMPV1su3bcLimysQ)
completed its bytes but initially retained an inconclusive readback and no baseline. A later
read verified all evidence and confirmed that same ID without retransmission. This motivated
bounded observation retries. The final [fresh resume fixture](https://drive.google.com/drive/folders/1xCOJrw9cP5YiBYp05jnpetsnPwabe-1Y)
passed prepare, recovery and verification with that final behavior. Its sole file retains ID
`1JmdvV9xwgdlCixSq1i2EKqk3mbq1ZIEm`; source SHA-256 is
`222428124c1fa78d62a874b1ada312b25e22ca1135f3e3ebe4a73f0490977695`.
Both local fixtures are preserved as `-resume` and `-final-resume` siblings of the primary root.

These checks exercised real Google observation, account authentication, multi-block uploads and
Engine recovery. They did not click through the GTK window or validate installation/update of
a distributed Flatpak package.
