# Desktop and Flatpak integration (development)

This folder holds preliminary assets for application ID `com.rtosta.DriveSync.Devel` in `src/main.rs`.

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

Closing the window requests the desktop Background portal permission without
requesting login autostart. Only an explicit grant hides the window. A native
notification offers Open and Quit; the account menu and Ctrl+Q also quit.
Denied or unavailable permission leaves the window visible with a Keep Open / Quit
dialog. GNOME background status distinguishes active manual cycles from waiting.
Closing the window does not schedule new synchronization cycles.

The launcher and notifications require installing the Flatpak, not merely running
its build directory in Builder. Installed desktop identity must match the binary's
`DRIVESYNC_APP_ID`. Reopening the launcher activates the existing window.

The opt-in `background::tests::desktop_close_and_reopen` test must run inside a
sandbox with the matching app identity, session bus and desktop display, with
`DRIVESYNC_BACKGROUND_TEST=1`. It creates only a test window and never initializes
the engine or reads account data. It requires background permission; denial must
leave the test window visible and therefore fails its success-path assertion.
