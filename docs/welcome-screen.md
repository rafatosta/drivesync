# Welcome screen

The first-run screen is implemented in `src/welcome.rs`, using GTK4 and
Libadwaita widgets. It follows the supplied visual reference with the existing
DriveSync folder/cloud artwork, native header bar, centered introduction,
primary connection action, Learn More disclosure, and a browser OAuth note. Theme colors and typography come from GNOME.
A scrolled, clamped layout centers the content horizontally and vertically,
with equal top and bottom margins, and supports narrow windows and large text.

## Behavior and boundaries

- UI calls only the public `DriveSyncEngine` facade. No engine contracts change.
- Connect calls `add_account`. Its current `NotImplemented` result is shown
  inline and disables further attempts for this process. No browser is opened,
  credentials collected, account fabricated, or synchronization claimed.
- Learn More expands an inline explanation, without creating another window.
- Copy explicitly describes a download-only technical preview; sign-in and
  transfers are not functional yet. Setup step cards are omitted from this screen.
- Startup creates one engine and one window. Repeated activation presents that
  window. Close hides it; the menu's Quit action or Ctrl+Q exits the application.
- No background portal or autostart support is added by this UI change.

## Manual acceptance checks (GNOME / Builder)

1. Launch: native header, own app artwork, introduction, actions,
   and browser OAuth note are visible; no Google logo is introduced.
2. Resize from 960 to 360 pixels wide: labels wrap, vertical
   scrolling exposes all content, and actions remain reachable.
3. Enable dark style and large text: native theme remains legible; no clipping.
4. Activate Learn More with keyboard: explanatory text expands/collapses inline.
5. Activate Connect: an honest unavailable message appears, the action becomes
   insensitive, and neither browser nor fake account/dashboard appears.
6. Close and activate again: the same window and connection feedback reappear.
   Quit from the menu and Ctrl+Q terminate the process.

These checks do not validate OAuth, transfers, background permissions, or any MVP
engine gate. Run them on the target GNOME desktop before declaring visual QA complete.

## Verification for this implementation

On 2026-10-20, using an isolated Rust toolchain with GNOME SDK 50 (GTK 4.22.5,
Libadwaita 1.9.4): application build passed; `cargo fmt --package drivesync
--check`, `cargo test --workspace` (one existing engine test), and
`cargo clippy --all-targets -- -D warnings` passed. No engine implementation or
contracts were changed. Libadwaita's `v1_4` feature is enabled for ToolbarView.

An initial launch probe exposed an SVG loader failure without the Flatpak portal.
The screen now falls back to GNOME's folder-download symbolic icon if artwork
cannot be decoded. A subsequent five-second launch probe with an isolated D-Bus
session stayed running until the deliberate timeout, with environment warnings
about missing portals/graphics drivers. This is an initialization check, not a
visual review or interaction test. The manual checks above remain outstanding.
