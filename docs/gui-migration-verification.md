# GUI architectural migration verification

Verification scope: relocation from main `72fa6bbcb5ad4b3413a5b649cd7d106521f8688a`
on branch `refactor/gui-hybrid-architecture`. See the
[ownership and relocation map](gui-architecture.md). This report records the
requested migration acceptance evidence, not real Google Drive certification.

## Environment and commands

All compilation, Rust tooling, GUI fixtures and pixel comparison ran inside the
GNOME SDK 50 from Flathub, with its `rust-stable` extension and GTK/libadwaita.
No host Cargo, rustc, rustfmt or Clippy was used. The SDK and environment follow
`flatpak/com.rtosta.DriveSync.Devel.yml`; no SDK was installed or updated.

The temporary build environment was initialized with:

```sh
flatpak build-init /tmp/drivesync-architecture-sdk \
  com.rtosta.DriveSync.Devel org.gnome.Sdk org.gnome.Platform 50
```

Commands below ran through `flatpak build`, granting the project and Cargo cache
filesystem access and selecting
`PATH=/usr/lib/sdk/rust-stable/bin:/usr/bin`. GUI runs also used the desktop display,
IPC, `GSK_RENDERER=cairo` and temporary profiles under `/tmp/drivesync-ui-*` and
`/tmp/drivesync-preferences-ui-*`. The screenshot application was granted its
`com.rtosta.DriveSync.Screenshots` bus name. Fixtures were run individually to
keep GTK initialization in separate processes.

| Verification | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo build --workspace --locked --offline` | Passed |
| `cargo clippy --workspace --all-targets --offline -- -D warnings` | Passed |
| `cargo test --workspace --locked --offline` | Passed: 36 GUI tests, 416 engine tests, two integration tests and one doctest; 33 opt-in tests ignored by default |
| `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --workspace --no-deps --document-private-items --offline` | Passed |
| `cargo test --workspace --doc --offline` | Passed: one doctest |
| 16 opt-in GUI fixtures via `cargo test -p drivesync --offline <fixture> -- --ignored --test-threads=1` | 15 passed; one existing main failure described below |
| 48 layout captures compared with main using GdkPixbuf inside the SDK | 44 pixel-identical; four reviewed differences described below |
| Engine, Cargo manifests, lockfile and build script diff against main | Unchanged |
| Relocation review | Existing implementations retained; imports, module visibility, asset path and formatting adjusted |

The GUI fixtures cover account selection and action targeting, idle pause,
folder configuration and context, observer disposal, preferences navigation,
account editor reuse, conflict choices and literal rows, merge review, progress
states, dashboard layouts and demonstration screenshots. Background/autostart
portal opt-in tests and engine tests requiring external services remain unrun.

## Existing failure and visual limits

`comparison_windows_render_text_png_and_fallback_at_narrow_width` fails because
the text comparison window is 377 px wide while the assertion requires at most
360 px. The same failure, same size and same assertion were reproduced against
a clean detached checkout of main `72fa6bb` in the same SDK/display environment.
The dialog and test implementations are preserved. Fixing its layout is separate
work; this migration does not claim a completely passing visual suite.

Three changed images contain the build revision text (dashboard and two welcome
sizes). The fourth, account preferences, contains the randomized temporary folder
path and transient scrollbar visibility. Manual inspection found no layout change
in those four captures. This comparison does not prove every interactive state,
rendering backend or desktop configuration is equivalent.

Raw execution logs, captures, comparison output and the local relocation audit
are retained under `target/architecture/` in the working checkout; they are build
artifacts and are not versioned. No real Google Drive operation, installed Flatpak
package, background portal approval or autostart setting was exercised. No merge
or push was performed.
