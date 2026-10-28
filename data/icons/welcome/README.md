# Welcome artwork

`com.rtosta.DriveSync.png` is a 224×224 transparent PNG export of
`../hicolor/scalable/apps/com.rtosta.DriveSync.svg`. The welcome view embeds
this export so it can decode the application artwork without an SVG loader
or desktop image-loading portal. Refresh it when the production SVG changes.

Regenerate from the repository root with ImageMagick:

```sh
magick -background none data/icons/hicolor/scalable/apps/com.rtosta.DriveSync.svg -resize 224x224 data/icons/welcome/com.rtosta.DriveSync.png
```
