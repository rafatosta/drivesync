# Functional correction — Stage 4: initial remote-to-local synchronization

Stage 4 connects current scope captures, initial planning, ordered Operations and the Stage 3
recovery journal. Ordinary `Engine::start_synchronization` can now materialize an existing
remote binary tree into an initially empty local root. Existing equivalent local and remote
replicas still require explicit adoption authorization; existing local content is never silently
adopted or replaced by an initial download. Initial upload planning remains Stage 5.

## Evidence, ordering and publication

Context derives a destination only from complete current local/remote captures, an unambiguous
remote name, and the configured root or a confirmed directory association. It carries the
absent destination's name and parent and can reconstruct that absence after identity registration
without pretending an inode exists. A parent with an active uncertain operation cannot authorize
children. Missing parents, partial captures, duplicate names and occupied destinations remain
blocked. Context does not write Inventory or execute synchronization policy.

The pure Analyzer emits `RegisterIdentity -> CreateLocalDirectory` or
`RegisterIdentity -> Download` for an anonymous remote item. A registered NeverSynced item with
freshly proven local absence resumes with the material step alone. Remote version, binary size,
valid SHA-256, download capability and corresponding destination parents are required. Workspace
native items are not included in this binary download planner.

Orchestrator refreshes each dependent request only from verified committed receipts. Operations
rechecks the confirmed parent association before initial child dispatch and atomically checks
the parent's revision when confirming the child's baseline. Confirmed local directory publication
signals the local producer to collect a fresh round immediately. That round re-evaluates cached
remote children against the new parent association; no additional external edit or remote change
is required. Observation and execution remain separate responsibilities.

Downloads stream into anonymous temporary storage and must completely write the expected size
and SHA-256. Read/write failure, surplus bytes, invalid digest or a changed remote version stops
publication. The verified file is staged, tagged and checkpointed through the Stage 3 journal
before a no-replace rename. A concurrent local destination is preserved. Directory creation also
rechecks the remote version after its effect; a lost/stale receipt remains uncertain for recovery.
Initial transfers do not use replacement or trash; the existing replacement path still refuses
unavailable trash rather than destroying the previous bytes.

An individual item's committed convergence confirms its association and baseline, including
nested parent dependencies. `Idle` retains its existing meaning: no currently pending plan for
observed items. It is not an assertion that the whole tree or account is synchronized.

## Automated validation

All Rust commands run inside GNOME SDK 50 with rust-stable, matching the development manifest:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo doc --workspace --no-deps --document-private-items`
- `cargo test --workspace --doc`

New tests cover anonymous/registered initial plans, incomplete evidence and unavailable parents,
Workspace exclusion, duplicate names, three directory levels through real Context/Analyzer/
Operations, empty and 1 MiB binary files, immediate parent refresh signals and reopening Context
without new executions or baseline writes. Loopback execution tests inject a collision during
media transfer, a remote version change, invalid hash and invalid size; none confirms a baseline
or overwrites concurrent local content. The production media writer is tested against `/dev/full`,
which returns the real `ENOSPC` error without filling a shared filesystem.

The final workspace run passed 644 tests, with 9 ignored by default. The real initial-download
opt-in test was executed separately in multiple processes as described below. The other eight
opt-in tests were not rerun for this stage. Formatting, Clippy, private API documentation and
doctests also passed; there are currently no executable doctest examples.

Existing Stage 3 tests continue to cover Pending/Running, staged publication, effect-before-
receipt, receipt-before-baseline, revision conflicts and new Engine startup recovery. The existing
unavailable-trash test verifies preservation of source bytes. These lower-level failure checks do
not claim that a real storage device was filled or the Google service was interrupted.

## Real validation on 2026-11-22

The authorized `rtosta.coc@gmail.com` account was exercised through the public Engine API inside
the Devel Flatpak build sandbox, using its existing Accounts/Secret Service session. The opt-in test
`connection::live_initial_download_tests::real_remote_tree_converges_and_reopens_without_duplicate_downloads`
started with an empty local directory and ordinary synchronization, without adoption permission.
The [isolated remote fixture](https://drive.google.com/drive/folders/129p5V9ZMEOrXQrvxwqT1b9rcY8uP21PD)
contains six items:

```text
root.bin                 31 bytes
a/
  empty.bin               0 bytes
  b/
    c/
      large.bin     8388608 bytes
```

The plugin created the folders and nonempty sources. Its file transport rejected zero-byte
uploads, so the test created only the empty source through the authenticated provider before
starting synchronization. All six items reached Synced baselines with exactly one confirmed
material operation each. Large-file bytes matched the deterministic source; SHA-256:
`7d212b9c884f5c77896de960ae17cc341cda43b14d6a971f34ca29ebd4badf7f`.
The resulting public status was Idle with no issue. Another process reopened the Engine and
preserved every scoped Inventory row, identity, baseline, execution and revision. Independent
plugin listings of all four remote folders confirmed the same six items and no duplicates.

A final fresh-root download using the strengthened parent checks also converged all six items;
its subsequent verification process checked the same persistence invariants.

The preserved local fixtures are under
`/home/tosta/GoogleDrive/DriveSync-stage4-validation-2026-11-22` and its `-final` and `-strict` siblings.
No unrelated local or remote content was changed. This exercised the public Engine, Accounts,
real Google observation and real transfer; it did not drive the GTK window by clicking or test
installation/update of a distributed Flatpak package.
