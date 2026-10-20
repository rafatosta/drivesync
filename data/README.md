# Desktop and Flatpak integration (development)

This folder holds **preliminary integration assets** for the existing application ID `com.rtosta.DriveSync.Devel` from `src/main.rs`.

- `icons/hicolor/scalable/apps/com.rtosta.DriveSync.Devel.svg`: original **SVG reconstruction** of the chosen blue-folder/cloud concept, **not** an SVG directly exported from the generated bitmap.
- `com.rtosta.DriveSync.Devel.desktop`: launcher metadata; `Exec=drivesync` matches the Cargo binary.
- `com.rtosta.DriveSync.Devel.metainfo.xml`: initial AppStream metadata; licensing is **placeholder only**, must be resolved before publishing. No release or screenshots are claimed.
- `../flatpak/com.rtosta.DriveSync.Devel.yml`: preliminary Flatpak development manifest, **not tested or release-ready**.

## First Builder run checklist

1. Confirm Rust/GTK/Libadwaita versions compile in the target GNOME SDK and choose a supported runtime.
2. Generate and lock Rust dependencies and configure an **offline/vendor** build for Flatpak Builder; the draft manifest deliberately does **not** silently download dependencies.
3. Verify local build/test (`cargo check`, `cargo run`) before claiming the Flatpak build is functional.
4. Validate `desktop-file-validate`, `appstreamcli validate`, and render the SVG with `rsvg-convert` or GNOME image tooling.
5. Verify icons, launcher, application ID, metadata and session behavior inside the sandbox.
6. Establish a project source license and replace the AppStream placeholder. Add non-placeholder release metadata and actual screenshots only after testing.
7. Add filesystem and background portal permissions only after testing user flows; avoid blanket home-directory access.

The current manifest is a **scaffold** and will require dependency vendoring before offline `flatpak-builder` succeeds.
