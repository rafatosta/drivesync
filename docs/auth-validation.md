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
