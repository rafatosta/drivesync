# Desktop and Flatpak integration

This folder holds desktop integration assets for development (`com.rtosta.DriveSync.Devel`) and production (`com.rtosta.DriveSync`). Each Flatpak manifest installs its matching `.desktop` and `.metainfo.xml` files directly from this folder.

- `icons/hicolor/scalable/apps/com.rtosta.DriveSync.Devel.svg`: original SVG reconstruction of the blue-folder/cloud concept (not a vector export of the generated bitmap).
- `com.rtosta.DriveSync.Devel.desktop`: launcher metadata, matching the `drivesync` Cargo binary.
- `com.rtosta.DriveSync.Devel.metainfo.xml`: initial AppStream metadata. The source license is still a placeholder and **must be replaced before publication**.
- `../flatpak/com.rtosta.DriveSync.Devel.yml`: **GNOME Builder development-only manifest**. It uses the Rust SDK extension and allows Cargo to download crates *while compiling*.

## First run from GNOME Builder

1. Install the GNOME SDK for the selected runtime and the matching Rust SDK extension. The extension is declared in the manifest but Builder/Flatpak still needs to resolve and install its compatible branch.
2. Reopen/reload the project and rebuild with the selected Flatpak configuration. The manifest adds `/usr/lib/sdk/rust-stable/bin` to `PATH`; it does **not** require Meson.
3. The manifest runs `cargo --version` followed by `cargo build --release --locked`, using the versioned `Cargo.lock`. Dependency updates must be reviewed and committed.
4. The manifest gives the **build sandbox only** temporary network access (`build-options.build-args: --share=network`) so the prototype can fetch crates. This is **not suitable for reproducible release/Flathub packaging**. The application's existing `finish-args: --share=network` is separate and will be needed for the Google Drive API.
5. Before the first reproducible release, generate `cargo-sources.json` from `Cargo.lock` using `flatpak-cargo-generator`, add it to the module's sources, switch to `cargo build --release --offline --locked`, and remove the build network permission.
6. Verify `cargo check --workspace`, `cargo test --workspace`, desktop/metainfo validation, and actual icon rendering. Confirm the launcher and sandbox permissions on a Builder run.
7. Select a genuine project license and replace the placeholder AppStream declaration; add proper releases and screenshots only when available.

See [Flatpak's Rust/Cargo generator documentation](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo).

Google account authentication requires the local Desktop OAuth configuration and Secret Service permission described in [the OAuth guide](../docs/google-oauth.md). The development runtime/SDK is GNOME 50.

## Background execution

Closing the window requests the desktop Background portal permission and preserves
login autostart registration. Only an explicit grant hides the window. A native
notification offers Open and Quit; the primary header menu and Ctrl+Q also quit.
Denied or unavailable permission leaves the window visible with a Keep Open / Quit
dialog. GNOME background status distinguishes active cycles, connection failures,
pending issues and automatic checks. Saved configured accounts are checked
automatically while the application is running, including with its window hidden.

The production profile must be installed before running it. On GNOME 50,
GdkPixbuf uses Glycin to load the welcome SVG through `flatpak-spawn --sandbox`.
That helper requires an installed application; running an uninstalled production
build can fail with “app … not installed”. Glycin skips its loader sandbox for
application IDs ending in `Devel`, so use the development manifest for direct
GNOME Builder runs. Export and install the production Flatpak to test that profile.

The launcher and notifications also require installing the Flatpak, not merely running
its build directory in Builder. Installed desktop identity must match the binary's
`DRIVESYNC_APP_ID`. Reopening the launcher activates the existing window.

The opt-in `background::tests::desktop_close_and_reopen` test must run inside a
sandbox with the matching app identity, session bus and desktop display, with
`DRIVESYNC_BACKGROUND_TEST=1`. It creates only a test window and never initializes
the engine or reads account data. It requires background permission; denial must
leave the test window visible and therefore fails its success-path assertion.

## Session startup

The primary instance requests Background portal authorization once per launch,
with `autostart=true` and the argument array `drivesync --background`. The portal
owns the session launcher; DriveSync never writes host autostart files directly.
Closing a window uses the same request so it cannot accidentally remove that
registration. Concurrent startup/close requests share one in-flight portal call.

The response's `background` and `autostart` results are checked separately. Denial,
malformed responses, unavailable services and registration failures expose the
window with a native toast and leave manual use available. Success is never
inferred merely from receiving a request handle.

A session launch keeps the initial window hidden. A normal launcher invocation
shows the existing primary instance; a repeated `--background` invocation never
hides a visible window or constructs another engine. Quit ends the current run
without disabling the next session's startup. Login autostart remains subject to
the desktop's permission and session settings.

The ignored `background::startup::tests` native fixtures cover permission denial,
late grants, hidden startup and activation without engine data. The real-portal
fixture requires `DRIVESYNC_BACKGROUND_TEST=1` and registers the development login
command. An actual logout/login must be verified separately; it is not simulated
by a unit test and must not interrupt an active user session during testing.
