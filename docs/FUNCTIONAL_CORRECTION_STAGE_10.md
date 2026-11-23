# Functional correction — Stage 10

## Explicit policy

Normal `Engine::start_synchronization` and initial-association startup keep trash
propagation disabled. `start_synchronization_with_trash_propagation` authorizes it for
one exact worker session. Switching policy requires stopping the active session; normal
restart drops authorization. The worker and executor both enforce the gate. A historical
baseline, a watcher removal event or missing provider access grants no removal permission.

Automatic propagation applies only to confirmed native replicas. Workspace projections
remain read-only derived artifacts: deleting a local link never trashes its document.
Remote shortcuts remain unsupported. Permanent deletion remains prohibited.

| Evidence | Result under explicit trash authorization |
| --- | --- |
| Local slot missing, same incarnation retained in local trash with original-path receipt, complete scan excludes aliases; remote unchanged | Conditional `TrashRemote` |
| Provider explicitly reports `trashed=true`, current native local replica matches the baseline | Recoverable `RemoveLocal` |
| 403, ambiguous 404, Unknown/Unavailable, partial capture, or OutsideScope | Preserve data; no trash command |
| Missing local file without recoverable trash proof, including a move outside the root | Refuse dispatch; retain baseline |
| Concurrent peer edits, rename, move or type change | Conflict/defer; no automatic trash |
| Both sides have a confirmed absent/trashed baseline | Tombstone NoOp; never recreate or replay |

Later remote metadata revisions may remain comparable only with identical proven native
content, name, parent and type and monotonic version. Dispatch always uses the current
observed version and fresh provider ETag. A changed hash is not a metadata-only revision.

## Conservative folder policy

Only completely observed empty native folders may be trashed automatically. Context
checks local entries, remote children and registered descendants; a missing descendant
probe or partial capture is not empty-tree evidence. Operations independently checks the
Inventory and current filesystem. Provider queries include trashed children and reject
incomplete or paginated results. Nonempty trees are preserved even if all known children
appear unchanged. Recursive tree trash is deliberately not enabled in this stage.

Folder children can change independently of their parent's provider version. Descendant
checks occur before effects and again while confirming effects. A race that prevents
confirmation remains uncertain and recoverable; no destructive fallback is attempted.
Filesystem/provider reads cannot form a transaction against arbitrary external writers.

## Execution and recovery

Local trash uses private freedesktop `Trash/files` and `Trash/info` directories with a
restore receipt. The destination must be on the source filesystem. Missing trash roots,
unsafe ownership/permissions, unavailable storage and cross-device destinations refuse
execution while preserving the source. No copy/unlink or permanent-delete fallback exists.
Symlinks are not followed. Automatic trash propagation requires a known local birth identity. Local-to-remote propagation also requires
a unique regular-file link (or empty directory), retained bytes and the original-path
receipt. Search is bounded; exceeding the bound refuses proof.

Before local rename into trash, Inventory durably checkpoints destination path, destination
parent identity and receipt SHA-256. Source identity/content is rechecked; directory writes
are fsynced. Recovery verifies original absence, retained inode/kind/content, pinned trash
parent and unchanged receipt. Missing, edited, moved or replaced destinations remain
inconclusive. A crash before rename can be classified as not dispatched; recovery never
blindly replays trash.

Remote trash requires current `canTrash` and `canUntrash`, a conditional metadata update,
and the existing per-execution private marker. Recovery requires that marker, matching
identity/content/location, an advanced version, intact local trash proof and current
scope/ancestor checks. Provider and local effects are confirmed independently of baseline
writes. Verified absence on both sides records a native tombstone with stable identities;
unconfirmed effects retain durable pending records for later recovery.

Tombstones conservatively retain path ownership; automatic path reuse and restoration
policy are not introduced here. Read-only recovery of previously authorized intent can run
after a restart without enabling new trash propagation.

## Validation

Automated checks cover explicit session authorization and reset, unchanged/metadata-only
peers, denied/unknown/outside/partial evidence, divergent peers, empty-tree evidence and
registered missing descendants, unavailable and cross-filesystem trash, retained bytes and
receipts, pinned destination replacement, crash after local rename before effect persistence,
root access loss during recovery, absence confirmation and recovery without replay.

### Real account — 2026-11-23

The opt-in public Engine test `real_bidirectional_trash_scope_exit_and_restart` ran in the
existing Devel Flatpak build sandbox with actual Accounts/Secret Service and Drive API.
It verified exactly one connected account with email `rtosta.coc@gmail.com` before changes.
The connected Drive plugin created and read back the isolated
[remote root](https://drive.google.com/drive/folders/1_CzgBANPG8cfIqjoUv088269laWSU5Wi).
Its matching local root is `/home/tosta/GoogleDrive/DriveSync-stage10-validation-2026-11-23`;
recoverable trash fixtures are retained separately under
`/home/tosta/GoogleDrive/DriveSync-stage10-trash-validation-2026-11-23`.

- Six native records (four binaries, a folder and a nested child) converged initially.
- Local recoverable removal propagated to Drive trash, preserving IDs, bytes and receipts,
  with an absent/trashed baseline and confirmed execution.
- Actual provider trash propagated to same-filesystem local trash with a durable receipt,
  retained bytes and stable identities; no permanent deletion was used.
- Default startup preserved missing local data without propagating trash. Explicit startup
  also refused a local move outside the root without a valid trash receipt.
- Moving a provider item outside the remote root preserved its local replica and Inventory
  record; provider readback confirmed the item was untrashed and in a different parent.
- A remotely trashed nonempty folder retained its independently edited local child. Sources
  eventually recorded the child's conflict; no folder or child trash was dispatched.
- Restart preserved tombstones, identities, baselines and operation histories without replay.
  A freshly observed conflict may be persisted independently of those histories.
- An unavailable local trash root preserved the complete binary bytes and all baselines;
  failed intent was recorded for audit without reporting a successful removal. Restoring
  the test trash location then completed that removal, retained bytes and confirmed a third
  tombstone. A further restart preserved the complete effect history.

The first real local-removal attempt exposed two separate boundaries: GIO in the sandbox
used the host portal's trash, and Drive advanced an uploaded binary from version 1 to 2
without content changes. Only the verified isolated test object and its original-path receipt
were moved into the test trash root. The Engine conservatively refused unmatched proof.
Native source comparison now accepts later metadata-only versions with identical attributes,
while using the current ETag for dispatch. The reverse direction exposed a Context parent
association lost on `trashed=true`; its regression test now retains parent correspondence
without inventing a free destination. A complete direct trash probe retains this per-item
parent evidence even when an unrelated known ID cannot be accessed. Partial captures and
unknown target states still block propagation.

The opt-in fixture preparation uses an explicit isolated freedesktop receipt, not a claim
that the installed GUI/portal routes trash to this location. Production still refuses
cross-filesystem/unavailable home trash rather than using the host portal as a fallback.
Installed package and portal acceptance remain Stage 11 work. No unrelated files were
changed or deleted. Fixtures, outside-scope test folder, retained bytes and logs remain
available for inspection.

### Commands and totals

All Rust tooling ran in `org.gnome.Sdk//50` with the manifest's `rust-stable` extension:
`cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`;
`cargo test --workspace`; `cargo doc --workspace --no-deps --document-private-items`;
`cargo test --workspace --doc`. The workspace suite passed 693 tests with zero failures;
17 opt-in tests were ignored in the default run and real phases ran separately.

Real test variables: `DRIVESYNC_STAGE10_PHASE`, `DRIVESYNC_STAGE10_LOCAL_ROOT`,
`DRIVESYNC_STAGE10_REMOTE_ROOT`, `DRIVESYNC_REAL_ACCOUNTS_DIRECTORY`,
`DRIVESYNC_REAL_OAUTH_FILE` and a strictly isolated `XDG_DATA_HOME`. Flatpak's startup
sets its own XDG variables; the test shell explicitly exports the supplied fixture trash
home before invoking the test binary. `sync` creates a fresh fixture; `local`, `remote`,
`disabled`, `local-outside`, `outside`, `verify`, `unavailable` and `resume` exercise only
that selected fixture. The missing trash root used by `unavailable` must remain missing.
