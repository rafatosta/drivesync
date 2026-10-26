# DriveSync

**Google Drive for the GNOME desktop.**

DriveSync is an early-stage Linux application for keeping selected Google Drive folders available on this computer. It is a technical preview, not a stable release. Test with files you can safely replace or recreate; DriveSync is not a backup service.

## What works today

- Connect Google accounts through the system browser, restore saved accounts, and manage more than one account.
- Choose one Google Drive folder and one local folder for each account.
- Download regular files and nested folders, check for remote changes automatically, and run a cycle on demand.
- Pause and resume synchronization per account. A pause waits for the current operation to stop at a safe point and preserves completed work.
- Keep existing local data safe: conflicting or unsupported items are reported for attention, and remote deletions are not propagated to the local folder.
- Keep synchronization state and the folder identity marker inside the selected local folder's `.drivesync/` directory.
- Optionally enable **Upload new files and folders** for an account. This is create-only: existing remote files are not replaced, and local deletions are not sent.
- Represent Google Docs, Sheets, and Slides as local `.desktop` links to the document in Google Drive.
- Continue scheduled checks while the application runs. Hiding the window can keep the process active when the desktop's background permission is available.

## Current limits

- DriveSync is under active development and has no stable release. The full workflow still needs continued validation in GNOME and the development Flatpak.
- Each account has one configured remote/local folder pair.
- Upload is opt-in and only creates new remote files and folders. Editing an existing remote file and propagating deletions are not supported.
- Conflict detection preserves data and reports attention; there is no complete UI for choosing between local and remote versions.
- Google Workspace documents are local `.desktop` links. Opening those links from GNOME Files, including GNOME's first-open trust behavior, still needs validation.
- Shared Drives and several other special or unsupported Drive item types are not synchronized as regular files.
- OAuth testing builds may be limited to accounts approved in the configured Google Cloud project. Public distribution also requires completing OAuth and Flatpak release preparation.

## Next updates

1. Update the welcome screen and onboarding text so they describe the current download behavior and the optional create-only upload accurately.
2. Validate the complete current workflow in GNOME and the development Flatpak: account setup, multiple accounts, initial and incremental downloads, upload opt-in, pause/resume, restart recovery, and background behavior.
3. Verify how `.desktop` links behave in GNOME Files and document the supported opening flow.
4. Improve recovery and guidance for existing local folders, missing `.drivesync/` metadata, and conflicts, while preserving local data.
5. Define and implement any future updates to existing remote files or deletion handling only after the conflict and recovery policies are specified.
6. Finish OAuth and reproducible Flatpak packaging work before considering a public release.

## Project approach

DriveSync is being developed around explicit contracts, conservative filesystem changes, and account-isolated state. The [MVP plan](docs/MVP.md) describes the intended acceptance scope; implementation and live GNOME/Flatpak validation may not yet cover every planned item.

## Feedback

Found a problem or have a suggestion? [Open an issue](https://github.com/rafatosta/drivesync/issues).

DriveSync is an independent project and is not affiliated with Google.
