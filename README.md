# DriveSync

**Google Drive for the GNOME desktop.**

DriveSync is an early-stage Linux application for keeping selected Google Drive folders available on this computer. It is a technical preview, not a stable release. Test with files you can safely replace or recreate; DriveSync is not a backup service.

## Features

### Implemented

- Connect and manage multiple Google accounts.
- Choose one Google Drive folder and one local folder per account.
- Download files and folders, check for changes, and sync on demand.
- Pause and resume safely. Completed work is preserved.
- Preserve local data when conflicts or remote deletions occur.
- Store folder identity and sync state in `.drivesync/`.
- Optionally upload new files and folders. Existing remote files are not replaced.
- Save Google Docs, Sheets, and Slides as local `.desktop` links.
- Run automatic checks while the app is open or allowed to run in the background.

### Planned

- Update the welcome screen and onboarding text.
- Validate the full workflow in GNOME and the development Flatpak.
- Make `.desktop` links work reliably from GNOME Files.
- Improve recovery for existing folders, missing state, and conflicts.
- Define safe handling for edits to existing remote files and deletions.
- Prepare OAuth and Flatpak packaging for public distribution.

## Current limits

- DriveSync is under active development and has no stable release. The full workflow still needs continued validation in GNOME and the development Flatpak.
- Each account has one configured remote/local folder pair.
- Upload is opt-in and only creates new remote files and folders. Editing an existing remote file and propagating deletions are not supported.
- Conflict detection preserves data and reports attention; there is no complete UI for choosing between local and remote versions.
- Google Workspace documents are local `.desktop` links. Opening those links from GNOME Files, including GNOME's first-open trust behavior, still needs validation.
- Shared Drives and several other special or unsupported Drive item types are not synchronized as regular files.
- OAuth testing builds may be limited to accounts approved in the configured Google Cloud project. Public distribution also requires completing OAuth and Flatpak release preparation.

## Project approach

DriveSync is being developed around explicit contracts, conservative filesystem changes, and account-isolated state. The [MVP plan](docs/MVP.md) describes the intended acceptance scope; implementation and live GNOME/Flatpak validation may not yet cover every planned item.

## Feedback

Found a problem or have a suggestion? [Open an issue](https://github.com/rafatosta/drivesync/issues).

DriveSync is an independent project and is not affiliated with Google.
