# Functional correction — Stage 11: integrated acceptance and limits

Date: 2026-11-23 (America/Bahia). This matrix distinguishes installed application behavior,
real Google operations, native-filesystem experiments and controlled tests. A passing domain
or loopback test is not evidence of a real provider or portal effect.

## Result and release boundary

The native-root synchronization path passed installed Devel acceptance with the authorized
`rtosta.coc@gmail.com` account. Four native items, then a mixed five-item tree including a
Workspace projection, converged. Fresh provider listings, file SHA-256, unique provider IDs,
local paths and Inventory baselines agreed. Normal GUI restoration and repeated starts were
exercised; screenshot and structured installed-window evidence are retained separately.

This is **not unrestricted release approval**. The installed Engine's default local trash
refused the real attempted move while preserving the original bytes and baseline. The GIO
portal independently succeeded and retained a host-trash receipt, but that receipt does not
establish recoverability through the Engine's private trash implementation. The existing
saved document-portal FUSE root is readable but fails writer admission; a native sync root
is required. Two real Google accounts and bind-mount aliases were not exercised.

## Implementation completed during acceptance

- Kernel directory leases replace path-only overlap assumptions. Each ancestor has a shared
  lease and the selected root an exclusive lease. Siblings for independent accounts remain
  usable; nested roots and aliases of the same directory cannot become simultaneous writers.
  Symlink components and filesystems without the required identity/locking support refuse
  startup. Ordinary lock contention is distinct from an unavailable/unsupported root.
- One abstract Unix-socket lease per framed provider/subject identity prevents simultaneous
  writers for the same provider account from different local roots/profiles. This applies
  in the manifest's shared Linux network namespace, not across arbitrary namespaces or
  other machines. No global distributed lock against external Drive clients is claimed.
- The worker retains the leases through effect recovery and task termination, including
  Engine drop. A stopped/replaced session releases its lease before a new session starts.
  The executor verifies its pinned root against the admitted root identity.
- A create-only, private, fsynced `scope-<scope-id>.json` records device, inode and birth
  identity before observation/recovery starts. Reopening the same scope checks that proof.
  Root replacement, malformed proof or unsafe permissions refuses startup without rewriting
  the proof or touching either tree. Existing Inventory without a root proof also refuses startup; it cannot retroactively
  prove ownership of a possibly replaced root. An explicit new scope or manual review is
  required. No migration or automatic rebinding was introduced.
- `Engine::synchronization_audit` attests convergence at a captured effect epoch. It requires
  complete local and remote rounds at that epoch, the exact returned configured scope, a matching root, one proven owner for
  every observed item, valid representations/digests, confirmed baselines and no unconfirmed
  effects or unresolved conflicts. Empty roots can complete only after complete observations.
  An idle queue alone never supplies this proof. Audit uses indexed ownership sets, not
  quadratic per-item inventory searches, and excludes timestamp-only content assumptions.
- Explicit Devel-only installed-GUI instrumentation records a real window render, visible
  labels, selected test account, public audit and worker status. It requires an opt-in
  environment variable and exactly the authorized test account. Normal launches and the
  production application ID do not activate it. New OAuth login, folder-picker interaction
  and conflict-dialog click flows were not re-homologated in this installed run. It is not a simulated preview page.

Restart checks allow verified metadata-only baseline refresh (observed for a Workspace
version advance) while requiring unchanged identities, paths, content and material-operation
histories. Baseline revision immutability is not used as a substitute for no-replay proof.

An audit is evidence for its captured horizon, not a transaction against arbitrary external
filesystem/Google writers or a promise that data cannot change after observation. Busy or
unreadable storage yields no completion proof. Historical failed operations can remain in
the audit trail; unconfirmed effects and divergent current replicas cannot be dismissed.

## Acceptance matrix

| Scenario | Level and outcome | Evidence / limit |
| --- | --- | --- |
| Native installed GUI and account restoration | Installed Devel: passed | 960×720 live GTK window; real account; isolated saved configuration; initial four-item audit complete |
| Empty file, ordinary file, nested folder and 4 MiB binary | Installed Engine + real Google: passed | Unique IDs/paths, SHA-256 and baselines independently checked |
| Existing native content edit | Installed Engine + real Google: passed | Local write propagated; fresh provider digest matched resulting bytes |
| Remote rename | Installed Engine + real Google: passed | Local rename retained inode and provider identity |
| Local rename/move to a different managed parent | Installed Engine + real Google: passed | Same provider ID; resulting tree and baseline agreed; no content retransmission required by the plan |
| Workspace document | Installed Engine + real Google: passed | Derived non-executable desktop link, canonical ID URL, own projection baseline; five-item audit complete |
| Restart / repeated invocation | Installed GUI + Engine: passed | Second GUI invocation reused the running app; source restart retained identities and effect histories |
| Independent completion oracle | Installed Engine + fresh provider reads: passed | Two listing pages per tree; binary digests, IDs and local paths compared independently of GUI status |
| Default Engine local trash | Installed Engine + real Google: safe refusal | Actual `RemoveLocal` failure audited; bytes and prior baseline retained; no permanent-delete fallback |
| GIO portal trash | Installed runtime + actual desktop portal: passed | Only the isolated proof file moved; bytes and original-path receipt verified in host trash |
| Saved document-portal root | Installed admission boundary: refused | Real `fuse.portal` directory was readable; writer probe refused before sources/mutations |
| Multiple accounts / overlapping roots | Public Engine + controlled identities: passed | Independent account scopes allowed; nested roots denied; this is not a second real Google-account test |
| Aliases and multiple processes | Real temporary filesystem + OS process tests: passed | Lexical aliases and symlinks rejected/excluded; another process could not claim the same provider writer; bind mounts untested |
| Replaced root across stop/reopen | Public Engine + real temporary filesystem: passed | Same scope reopened; replacement refused; retained original bytes and new tree unchanged |
| Explicit configuration change | Public Engine + controlled sources: passed | Active changes refused; stopped session accepts an explicit new scope without deleting old data |
| 100,001-entry tree | Native SDK scanner: passed | First read Partial at 100,000; expanded budget then Complete; timings/memory below |
| Provider pagination and soft-budget recovery | Loopback provider: passed | Page continuations, incomplete search, cycles and soft item-limit growth covered; not a large real Drive benchmark |
| Inaccessible / outside-scope item | Engine integration and prior real Stage 10 evidence | Never converted to deletion; explicit per-item trash evidence can progress independently |
| Interrupted effects / durable recovery | Filesystem + loopback and Stages 3–10 evidence | Existing crash-window suite rerun; no claim that every crash window was re-injected against real Google |

## Measured scale and bounds

The opt-in scanner experiment created 100,001 zero-byte local files in the SDK temporary
filesystem. Creation took 0.334 s. The first scan returned explicit Partial with 100,000
entries in 1.518 s. After `expand_limit`, the same tree returned Complete with 100,001 entries
in 1.453 s. Process peak resident memory (`VmHWM`) was 121,228 kB, approximately 118.4 MiB.
The fixture was removed by its isolated test cleanup. This measures the scanner on cached
SDK temporary storage, not a 100,001-item upload, complete Engine memory, user Btrfs throughput,
or real Drive latency. Those larger end-to-end measurements remain unperformed.

Current collection soft budget is 100,000 entries and expands to a hard 1,600,000 ceiling;
local depth is bounded at 128. Remote per-call page budget defaults to 16 and is capped at
64. Exceeding a budget, incomplete pages or uncertain evidence cannot become confirmed
absence or initial completion. No million-item or hard-cap exhaustion certification is made.

## Installed environment and retained evidence

- Application: `com.rtosta.DriveSync.Devel`, user installation, local `stage11` branch;
  GNOME Platform/SDK 50 and manifest `rust-stable`. No SDK/runtime install or update occurred.
- Package permissions: network, IPC, Wayland/fallback X11, DRI, `~/GoogleDrive` and Secret
  Service access. An existing user override grants read-only access to
  `/home/tosta/Documentos/Google Drive/rtosta`; it was recorded and preserved, not added here.
- The test package additionally contains an Engine acceptance executable under `/app/libexec`.
  Main binary and permission paths are the actual installed application; the sidecar is
  explicitly test instrumentation in a local package, not a published release artifact.
- Native fixture: `/home/tosta/GoogleDrive/DriveSync-stage11-validation-2026-11-23`;
  [Google root](https://drive.google.com/drive/folders/1LS1-0Q7nsEma0sF98o02HUXCMeecmEUI).
- Trash fixture: `/home/tosta/GoogleDrive/DriveSync-stage11-validation-trash-2026-11-23`;
  [Google root](https://drive.google.com/drive/folders/1Nl0ATF4Zj3D8AvU-bnaak7F1yrfzEVhI).
- GUI used a separate `XDG_CONFIG_HOME` beneath the Devel private configuration directory.
  The previous saved account choices and existing native/portal data were not replaced.
- Logs, independent checks and PNGs:
  `/home/tosta/Documentos/ChatGPT/drivesync 2/relatorios/etapa-11-20261123`.
  Installed GUI report also remains under the Devel data directory at
  `drivesync/stage11-acceptance`. Real fixtures and trash receipts remain available.

The portal file exists only in host trash, with its receipt, and is not a remotely synchronized
item. The Engine trash fixture retains its original native bytes despite the remote item being
trashed. No unrelated user files, credentials or configurations were deleted or reset.

## Validation commands

All Rust tooling ran inside GNOME SDK 50 with the manifest's rust-stable extension:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps --document-private-items
cargo test --workspace --doc
cargo build --release --locked
```

Default workspace suite: 699 passed, zero failed, 21 opt-in tests ignored. The subprocess
writer test runs separately to avoid temporarily inheriting parallel account-test leases
between fork and exec; its child process also ran the lease probe. The 100,001-file experiment, installed real-account
phases, installed portal-root probe and installed GUI runs were executed separately.

Installation used a local export/repository and `flatpak install --user --no-deps --no-related`.
No push, PR, remote release or SDK update was performed. Test screenshots use the Cairo GTK
renderer; default GPU renderer paths and every desktop/theme combination are not certified.
