# Credential vault diagnostics

DriveSync reports credential-vault failures with a fixed operation and category,
for example `Read/Crypto` or `Decode/InvalidData`. The same identifiers appear
on stderr when the application is launched from a terminal. Raw provider error
messages, account attributes and secret bytes are never logged.

Only `Locked` recommends unlocking the keyring. A crypto failure during a read
opens a fresh session and repeats that read once. This recovery does not replay
writes, deletions, JSON decoding or remote operations. A second failure blocks
synchronization and remains visible. `recovery=FreshSession` on stderr means the
second read succeeded; it does not mean the external backend has been repaired.

Do not reset synchronization state or delete keyring entries to investigate a
vault access failure. An unlocked default collection and a Flatpak permission
for `org.freedesktop.secrets` rule out those specific configuration problems;
they do not prove that every encrypted operation succeeds.

## Isolated oo7 contract test

Requirements: the SDK and Rust extension specified by
`flatpak/com.rtosta.DriveSync.Devel.yml` already installed from Flathub,
`dbus-daemon`, `busctl`, Python 3, and `/usr/libexec/oo7-daemon` on the host.
The runner never installs or updates these components.

Build the engine test executable inside that SDK. From the repository root:

```sh
mkdir -p target/vault-tmp
flatpak run --devel --command=sh --filesystem="$PWD" org.gnome.Sdk//50 -c '
  export PATH=/usr/lib/sdk/rust-stable/bin:$PATH
  export TMPDIR="$PWD/target/vault-tmp"
  cargo test --locked -p drivesync-engine --lib --no-run
'
```

If Cargo needs an existing host cache, explicitly grant and configure its
`CARGO_HOME` inside the sandbox. Cargo prints the test executable path; pass
that exact executable (not the `.d` file) to:

```sh
python3 scripts/check-vault-isolated.py --binary /absolute/path/to/drivesync_engine-HASH
```

The runner creates a temporary D-Bus, XDG directories and an unlocked disposable
oo7 keyring, executes the test in the project SDK, then stops its processes and
removes the temporary data. The test checks its isolation markers before accessing
credentials. It performs 500 reads, periodic writes, corrupt-JSON preservation,
delete idempotency and confirmed absence, using synthetic credentials only.
The test is ignored in the ordinary suite and refuses execution without the
runner's environment. A wrong executable is rejected rather than reporting a
successful run with zero matching tests.

This tests the real oo7 service with the engine adapter in an SDK sandbox.
It does not validate Google Drive access, production Flatpak permissions or
an end-to-end synchronization cycle. Intermittent failures in other operations
can still fail the test and must not be hidden by rerunning until it passes.
