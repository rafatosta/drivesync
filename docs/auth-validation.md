# Account persistence / startup restoration — 2026-10-21

Implemented ACCT-003 and the account-restoration portion of UI-001; UI-003 keeps
using the same engine/window. Reviewed the lifecycle and configuration algorithms
before implementation. No sync roots, state SQLite, autostart, background journal
or unlink implementation is included.

Profiles are atomically persisted in versioned `accounts.json` under the XDG data
directory, namespaced by Desktop client ID, with private file/directory modes.
Only subject, internal ID and optional email are stored; auth tokens remain in
Secret Service. Startup loads the index in a worker and verifies each identity
through existing AUTH-002, renewing expired credentials as required. Cached data
never counts as authenticated. Failures preserve the profile and remain isolated
per account, with UI retry/reconnect actions. Unknown/corrupt index versions fail
closed and are preserved.

Verification actually executed on this change:

- GNOME SDK 50: `cargo check --workspace --offline --locked`,
  `cargo clippy --workspace --all-targets --offline --locked -- -D warnings`,
  `cargo test --workspace --offline --locked`, and
  `cargo build --workspace --offline --locked`: passed.
- **24 tests passed**, including all existing auth tests and six account tests:
  disk restart/update/client isolation; corrupt/future/duplicate/foreign identity
  preservation; write failure; startup per-account failures/retry; concurrent
  writes and interrupted temp files; invalid profiles and oversized indexes.
- `cargo fmt --all --check` and `git diff --check`: passed.
- Rust 1.99.1 tools were installed only under `/tmp/drivesync-persistence-tools`;
  dependencies were downloaded before the final offline checks. No system packages
  or shell configuration were changed.

The user reported a successful real browser login with screenshots before this
change. **This new startup restoration has not been exercised with their real
Google account or actual vault**; automated provider/vault tests use doubles and
loopback tests use real local sockets. The application UI has compiled, but no
new visual or manifest-package acceptance run is claimed.

Manual acceptance: rebuild, reconnect an old account once to create its profile,
choose Quit / Ctrl+Q (closing only hides the window), reopen and confirm automatic
Account Information without browser login. Repeat with two accounts, offline,
locked keyring and revoked access; failures must preserve profiles without a
connected status, and retry must work after recovery. Old vault entries without
an index are deliberately not enumerated; reconnection repairs discovery.

The entries below record earlier stages and their then-pending limitations.

---

# Distribution OAuth configuration follow-up — 2026-10-20

The official Desktop client is now a build input (`data/google-oauth-client.json`
or `DRIVESYNC_BUILD_GOOGLE_OAUTH_FILE`), embedded after shared validation and field
filtering. Optional explicit runtime overrides remain for developers. Implicit
per-user XDG configuration is removed. `DRIVESYNC_REQUIRE_GOOGLE_OAUTH=1` rejects
unconfigured publication builds. Desktop `client_secret` is public compatibility
metadata; no user tokens are build inputs or logged.

Verification executed for this change:

- `cargo fmt --all --check` and `git diff --check`: passed.
- GNOME SDK 50: workspace check, clippy with `-D warnings`, tests and binary build,
  all offline/locked: passed. **19 tests passed**, including the existing 17 and
  two configuration tests covering precedence, invalid overrides, missing metadata,
  Desktop vs web JSON, validation and filtering of extra downloaded fields.
- `python3 scripts/check-oauth-build.py`: passed using synthetic, nonfunctional
  test metadata. Checked configured build, changed/invalid file, missing explicit
  file, permissive development build and strict publication failure; restored the
  ordinary development build afterward. No provider/vault calls were made.

The first restricted host test run passed 15 tests and failed four loopback tests
because sockets were blocked. Flatpak also initially could not create its network
namespace. Repeating in the GNOME SDK with the required sandbox permission passed
all tests. This was SDK validation, not a Flatpak manifest package build or a
GNOME Builder UI acceptance run; those remain manual checks.

No official client metadata was supplied. **No real Google login, consent,
browser return or actual vault persistence was validated.** The maintainer must
register/configure the official Desktop client once, meet consent/publication
requirements and stage its metadata before producing configured packages. Follow
[the maintainer and manual acceptance guide](google-oauth.md).

---

# Authentication implementation evidence — 2026-10-20

## Scope and maturity

The explicitly requested integration implements AUTH-001/002 and the minimal
account-profile registry needed for Welcome → Account Information. It does not
approve earlier MVP gates, implement synchronization, or claim a completed real
Google sign-in. F04/F05 and UI acceptance remain open.

- [x] Desktop OAuth code with PKCE S256, random state and strict loopback callback.
- [x] Google token exchange/UserInfo adapter with bounded, sanitized failures.
- [x] Per-account Secret Service backend; no plaintext production fallback.
- [x] Per-account refresh locking and typed reauthorization/network failures.
- [x] One engine and AdwNavigationView; worker, spinner, cancellation and retry.
- [x] Account Information with provider subject, optional verified email and state.
- [x] Automated unit/loopback tests and workspace compilation/linting.
- [ ] Real Google Desktop client configured and consent completed.
- [ ] Browser portal, loopback return and vault exercised together in the app Flatpak.
- [ ] Desktop vault restart/renewal validated with an actual test account.
- [ ] Two real Google accounts, denied consent, locked vault and offline UI checks.
- [ ] Visual/keyboard/adaptive layout review in GNOME Builder.
- [ ] Existing-folder Drive scope feasibility and distribution review (G04).

## Environment and commands actually executed

Checkout: `/home/tosta/Projetos/drivesync`, starting from `d32ee1c`.
Rust/Cargo installed in `/tmp/drivesync-auth-tools`, without changing the user's
shell configuration or installing system packages:

- rustc 1.99.1 (58cf8f9c3 2026-10-13)
- cargo 1.99.1 (5f94df478 2026-08-27)
- GNOME SDK 50, GTK 4.22.5, Libadwaita 1.9.4

Executed successfully:

```sh
cargo fmt --all
cargo fmt --all --check
cargo check --workspace --offline --locked
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo test --workspace --offline --locked
cargo build --workspace --offline --locked
git diff --check
```

The workspace check, clippy, tests and binary build ran inside the installed
GNOME SDK 50, with the checkout and isolated toolchain mounted and
`CARGO_TARGET_DIR=target/gnome-sdk`. The test result was **17 passed, 0 failed**;
the application and doc-test harnesses contained no tests. The tests use doubles
for Google/vault and actual 127.0.0.1 sockets for the callback integration.

The initial host-only workspace check could not find GTK/GLib development
packages; SDK execution resolved that environment issue. Callback tests also
require local sockets, which the outer restricted command sandbox blocks;
they passed with that permission available and inside the SDK.

A five-second application startup probe ran in an isolated D-Bus session with
`GTK_A11Y=none GSK_RENDERER=cairo`. It stayed alive until the deliberate timeout
(exit 124), with a warning about the absent settings portal in that isolated
session. This confirms initialization only; it does not prove visual layout,
portal browser launch or Google authentication.

For a comparable SDK check after installing Rust in a locally available path:

```sh
flatpak run --command=sh \
  --filesystem=/home/tosta/Projetos/drivesync \
  --filesystem=/tmp/drivesync-auth-tools --share=network \
  org.gnome.Sdk//50 -c '
    . /tmp/drivesync-auth-tools/env.sh
    cd /home/tosta/Projetos/drivesync
    export CARGO_TARGET_DIR=target/gnome-sdk
    cargo check --workspace --offline --locked &&
    cargo clippy --workspace --all-targets --offline --locked -- -D warnings &&
    cargo test --workspace --offline --locked
  '
```

These SDK checks use the runtime libraries, but are not an application build via
the development manifest. The latter still needs the Rust SDK extension and the
manual GNOME Builder procedure.

## Remaining acceptance dependency

No OAuth client was provided, and neither expected native nor Flatpak local
`oauth-client.json` existed when checked. No real credentials were fabricated,
no real Google session was used, and no user token was stored by these tests.
Follow [the configuration and manual test guide](google-oauth.md) before accepting
the primary end-to-end criterion. Automatic discovery of saved profiles on startup
is deferred to F07/F08; known IDs can already be verified using persisted vault
credentials through the facade.

## Changed files

- `.gitignore`
- `Cargo.lock`
- `README.md`
- `data/README.md`
- `docs/MVP.md`
- `docs/architecture.md`
- `docs/auth-validation.md`
- `docs/google-oauth.md`
- `docs/public-engine-api.md`
- `docs/welcome-screen.md`
- `engine/Cargo.toml`
- `engine/src/accounts/README.md`
- `engine/src/accounts/contract.md`
- `engine/src/accounts/mod.rs`
- `engine/src/auth/README.md`
- `engine/src/auth/algorithms.md`
- `engine/src/auth/callback.rs`
- `engine/src/auth/contract.md`
- `engine/src/auth/google.rs`
- `engine/src/auth/mod.rs`
- `engine/src/auth/storage.rs`
- `engine/src/auth/tests.rs`
- `engine/src/auth/tests/test-cases.md`
- `engine/src/lib.rs`
- `flatpak/com.rtosta.DriveSync.Devel.yml`
- `src/account.rs`
- `src/main.rs`
- `src/welcome.rs`
