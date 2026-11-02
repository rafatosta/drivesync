# Controlled live synchronization checks

## Contract and oracles

`sync-cycle` is an opt-in developer example, not a scheduler. It calls the public
`download_once` facade using the existing desktop vault and saved configuration.
It never exports credentials, configures folders or changes remote files.
Require `DRIVESYNC_LIVE_GOOGLE_TEST=1`, an exact account email, a non-root remote
ID and an absolute local directory matching the saved pair. The saved remote
name must start with `DriveSync — Ensaio controlado`. Mismatches fail before sync.
Use a dedicated test pair and do not start the GUI's cycle concurrently.

Each invocation is a new process, exercising durable state. JSON results expose
counts/media attempts, not credentials. Expected counts fail the command when
results differ. The six counts are `downloaded,updated,recovered,unchanged,
issues,preserved_copies`. This tool makes no changes to normal UI behavior.

Cancellation is injected after streamed bytes. The hold option prints a flushed
`ready_for_install` marker at Installing, after Ready is durable and before
rename. It then waits up to 30 seconds so a supervisor can kill the worker. The
application never enables this test-only option.

`scripts/check-live-sync.py` validates a baseline cycle and preserves an explicit
binary fixture outside the sync directory using a same-filesystem rename. It
runs cancel/retry and kill/recovery sequences in independent processes. SHA-256
checks verify the file after each recovery. On failure, it restores a missing
destination without replacing existing data. Preserved copies and evidence are
retained; the engine's partial staging is not deleted. An exclusive evidence
directory and root identity checks guard test-side renames. Do not concurrently
edit or move the test root; a cooperating desktop is assumed.

Oracles: unchanged baseline transfers zero bytes; cancellation leaves no partial
final file; retry installs identical bytes; SIGKILL after Ready recovers without
a new media request; a new final process reports unchanged. Unexpected counts,
timeouts, symlinks or differing bytes fail. This tests process crashes, not power
loss. Midstream network failures are covered by fakes; a separate real offline
check can run the binary in a sandbox without network access.

## Use

Build using the project's GNOME SDK/Rust environment, with the same packaged
OAuth client as the authorized app:

```sh
cargo build -p drivesync-engine --example sync-cycle --locked
```

Run with the app's `XDG_DATA_HOME`, portal filesystem access and permission to
contact `org.freedesktop.secrets`. Supply the configured test pair explicitly:

```sh
DRIVESYNC_LIVE_GOOGLE_TEST=1 target/debug/examples/sync-cycle \
  test@example.com REMOTE_TEST_FOLDER_ID /absolute/local/test-folder \
  --expect 0,0,0,8,4,0

DRIVESYNC_LIVE_GOOGLE_TEST=1 python3 scripts/check-live-sync.py \
  --binary target/debug/examples/sync-cycle \
  --account test@example.com --remote-root REMOTE_TEST_FOLDER_ID \
  --local-root /absolute/local/test-folder --transfer-file maior-8MiB.dat \
  --items 8 --issues 4 --evidence-dir /absolute/new/evidence-directory
```

A real metadata-only test may rename a controlled remote file and rename it back
before running a cycle. Identity/path/bytes remain unchanged while its server
revision grows. Expect unchanged counts, no media requests and no new backup.
Change remote fixtures through connected Drive tools or Drive UI; the production
engine requests the full Drive OAuth scope at account connection; uploads remain
disabled unless explicitly enabled.

## Flatpak document-portal identity

Portal paths are views, not interchangeable aliases for inode evidence. A host
process, an SDK sandbox and the application's `by-app/<APP_ID>` view can expose
different inode identities for the same underlying bytes. Keep the existing
journal checks; do not rewrite stored identities to make a test pass.

Prefer an installed-runtime check before concluding that packaging works. It
retains the native `/run/user/UID/doc` alias instead of replacing that mount:

```sh
flatpak run --filesystem=/absolute/repository:ro \
  --filesystem=/absolute/local/test-folder:ro \
  --env=DRIVESYNC_LIVE_GOOGLE_TEST=1 \
  --command=/absolute/repository/target/debug/examples/sync-cycle \
  APP_ID test@example.com REMOTE_TEST_FOLDER_ID /absolute/local/test-folder \
  --expect 0,0,0,8,4,0
```

The direct host-folder grant above is only for checking the supplied display-path
oracle; engine access still uses the saved portal path. Its native mount alias
must preserve the same grant/inode evidence while rejecting ordinary symlinks.
A custom SDK bind mount can hide this runtime difference and cannot replace this
packaging check. The ignored `runtime_portal_alias_preserves_host_origin_and_inode`
fixture also checks alias/canonical host origins using explicit portal test paths
without reading file contents or credentials.

For a Builder development checkout, `flatpak build --with-appdir` uses its build
directory, but a developer invocation may also need an explicit scoped portal
mount. Use the app's existing grant view as the source, for example:

```sh
flatpak build --with-appdir \
  --bind-mount=/run/user/UID/doc=/run/user/UID/doc/by-app/APP_ID \
  --share=network --talk-name=org.freedesktop.secrets \
  --filesystem=/absolute/repository \
  --filesystem=/absolute/app-data \
  --filesystem=/absolute/shared-test-parent \
  --env=XDG_DATA_HOME=/absolute/app-data \
  --env=DRIVESYNC_LIVE_GOOGLE_TEST=1 \
  build /absolute/repository/target/debug/examples/sync-cycle \
  test@example.com REMOTE_TEST_FOLDER_ID /absolute/local/test-folder \
  --expect 0,0,0,8,4,0
```

Replace UID, APP_ID and paths with the actual authorized development context.
The shared test parent must contain both the local fixture root and evidence
folder in a single filesystem mount. Separate bind mounts can make rename return
EXDEV even when the underlying host directories share a device. The suite aborts
before moving the fixture in that case. Do not grant access to unrelated paths
when a narrower common test parent is available.

### Isolated download fixture and installed profile

The ignored `facade::live_google_tests::prepare_isolated_download_fixture` test
requires `DRIVESYNC_LIVE_GOOGLE_TEST=1`, an exact `DRIVESYNC_TEST_ACCOUNT` email
and a new absolute `DRIVESYNC_DOWNLOAD_EVIDENCE` directory. It reads the existing
account metadata and vault in their original OAuth namespace, creates a generated
remote folder with text, empty and 10 MiB binary files, and configures a separate
`profile/` beneath that evidence directory. It never reconfigures the original
account index or enables engine uploads. The fixture manifest and originals are
retained there. `update_isolated_download_fixture` updates only the authenticated
`simple.txt` child in that isolated pair, preserving the original local fixture.
Both helpers are test-only, ignored by default, and require explicit opt-in.

To exercise the document portal itself, supply `DRIVESYNC_DOWNLOAD_PORTAL_ROOT`
as an explicitly exported, empty disposable directory outside the granted evidence
tree. Grant the application only the evidence directory and portal export; do not
grant the host origin. The helper asserts that the configured access path remains
the portal path and records both access and display paths in `fixture.json`.
`sync-cycle` accepts either exact saved path as its local scope argument, allowing
the recovery supervisor to operate on the writable portal view. Unrelated paths
remain rejected. Use the same portal view and persisted profile after restarting:
never rewrite inode evidence or change a direct fixture into a portal fixture.

Portal-only acceptance must record that the host origin is inaccessible while
regular-file flock actually returns ENOSYS. Keep root exclusion active. Verify
initial hashes, repeat and restart with the same journal, cancellation/retry,
Ready/SIGKILL recovery without media, remote-only update with backup, offline
recovery and preservation of local divergence. Retain the original root marker
and compare it after the suite. Put supervisor evidence in a sibling directory
inside the same exported parent, outside the configured root: moving between
the portal mount and a directly granted evidence tree can fail with EXDEV.

For installed apps, Flatpak can override `--env=XDG_DATA_HOME=...` during startup.
Set the isolated profile **after** startup through `/usr/bin/env`, and retain the
exact pair checks in `sync-cycle`. Do not change the normal app profile to work
around a scope mismatch:

```sh
flatpak run --filesystem=/absolute/evidence-directory \
  --command=/usr/bin/env com.rtosta.DriveSync.Devel \
  DRIVESYNC_LIVE_GOOGLE_TEST=1 \
  XDG_DATA_HOME=/absolute/evidence-directory/profile \
  /app/bin/sync-cycle test@example.com REMOTE_TEST_FOLDER_ID \
  /absolute/evidence-directory/files --expect 3,0,0,0,0,0
```

The locally installed validation package must contain the freshly built
`sync-cycle` executable; an SDK executable is not proof of installed behavior.
Run the Python recovery supervisor inside that same installed runtime so SIGKILL
targets the actual worker rather than a host Flatpak launcher. Start each command
only after the preceding process has exited. Use `--unshare=network` for the
offline cycle and restore normal networking for recovery. Coordinator Offline
suspensions and lower-level network errors both satisfy `--expect-error network`.

For the generated three-file fixture, the count oracles are:

| Scenario | Counts or error | Additional oracle |
| --- | --- | --- |
| Empty local root | `3,0,0,0,0,0` | Text, empty and binary SHA-256 match originals. |
| Repeat/new process | `0,0,0,3,0,0` | Zero media attempts and identical bytes. |
| Remote-only simple-file edit | `0,1,0,2,0,1` | New bytes installed; the last count includes the required preserved backup of the old bytes. |
| Offline start | `--expect-error network` | Zero media attempts; persistent state remains retryable. |
| Reconnection | `0,0,0,3,0,0` | Zero media attempts. |
| Local-only simple-file edit | `0,0,0,2,1,0` | Zero media attempts; divergent local bytes and download journal records remain unchanged. |

Run cancellation/SIGKILL recovery before editing the simple file, passing
`--items 3 --issues 0 --transfer-file binary.dat` to the existing supervisor.
All cycles must use the isolated profile. Preserve failed command outputs as
well as successful results; a failed count assertion does not undo a completed
installation. Inspect bytes, backups and journal evidence before deciding
whether the implementation or the test input was incorrect. Do not reset state
to rerun an initial-download oracle against an already installed file.

For an offline-start check, use `--unshare=network` instead of `--share=network`
and pass `--expect-error network`. It must fail before requesting media. Follow
it with a normal unchanged cycle to verify that observation state is retryable.
This does not simulate losing the connection midway through a real response.
