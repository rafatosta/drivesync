# DriveSync — GNOME icon design sources

This directory is reserved for the **editable GNOME App Icon Preview source SVG**. No final DriveSync artwork has been approved yet.

## Start from GNOME's *original* template

GNOME publishes the authoritative SVG at:

https://gitlab.gnome.org/Teams/Design/HIG-app-icons/-/blob/master/template.svg

Download it via the helper in this directory:

```sh
sh data/icons/source/fetch-gnome-template.sh
```

The download is intentionally **not represented as an official template until successfully fetched from GNOME**. Open the downloaded `gnome-app-icon-template.svg` in Inkscape, or create a new design in GNOME App Icon Preview.

The GNOME application icon source includes areas for both the full-color 128×128 icon and its symbolic counterpart; do not confuse the symbolic 16×16 design with the colored app icon. Use App Icon Preview to inspect/export the results.

## Intended outputs

| Channel | App ID | Installed icon |
| --- | --- | --- |
| Stable | `com.rtosta.DriveSync` | `data/icons/hicolor/scalable/apps/com.rtosta.DriveSync.svg` |
| Development | `com.rtosta.DriveSync.Devel` | `data/icons/hicolor/scalable/apps/com.rtosta.DriveSync.Devel.svg` |
| Stable symbolic | `com.rtosta.DriveSync` | `data/icons/hicolor/symbolic/apps/com.rtosta.DriveSync-symbolic.svg` |
| Development symbolic | `com.rtosta.DriveSync.Devel` | `data/icons/hicolor/symbolic/apps/com.rtosta.DriveSync.Devel-symbolic.svg` |

GNOME recommends a distinctive Nightly/Devel version, which **App Icon Preview can generate automatically**. Maintain one editable master design, not four independently designed symbols.

## Planned variants

Keep the editable master at `data/icons/source/drivesync.svg`. Treat
`docs/mockups/icons/` as the location for standalone visual references and
explorations; it is not an installable icon directory. Export the approved
full-color artwork to the stable and development paths in the table above, and
export the matching symbolic artwork to the two `hicolor/symbolic/apps/` paths.
The development full-color variant should be generated from the same master
with App Icon Preview's Devel/Nightly treatment, so stable and development
remain recognizable as one app. Keep symbolic icons monochrome and suitable for
small sizes; do not add the colored development treatment to them.

The current files in the installable paths are working assets, not approved
final variants. Update them only when the master artwork is approved and its
exports are ready together. This plan does not require changes to application
IDs, desktop files, or Flatpak manifests.

Do **not** overwrite the current development SVG or change Flatpak manifests until an actual icon is designed and exported, so existing Builder builds keep working. The current development SVG is temporary artwork, not a GNOME template.

## References

- [GNOME HIG — App Icons](https://developer.gnome.org/hig/guidelines/app-icons.html)
- [GNOME HIG — UI Icons](https://developer.gnome.org/hig/guidelines/ui-icons.html)
- [GNOME design resources](https://developer.gnome.org/hig/resources.html)
- [App Icon Preview](https://flathub.org/apps/org.gnome.design.AppIconPreview)
