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

### Planned (validation and release)

- Update the welcome screen and onboarding text.
- Validate the full workflow in GNOME and the development Flatpak.
- Make `.desktop` links work reliably from GNOME Files.
- Improve recovery for existing folders, missing state, and conflicts.
- Define safe handling for edits to existing remote files and deletions.
- Prepare OAuth and Flatpak packaging for public distribution.

### Future features (post-MVP)

The MVP has established the core application foundation: Google account connection, local/remote folder pairing, basic downloads and uploads, and synchronization. The following items are **candidate enhancements and design decisions**, not unfinished prerequisites for building that foundation. They will be specified and prioritized separately; some extend safeguards or capabilities already present in the technical preview.

| Priority | Capability | Intended direction |
| --- | --- | --- |
| P0 | Complete conflict resolution | Provide a clear flow for local/remote edit conflicts without silently overwriting either version. |
| P0 | Safeguards for deletions | Define confirmation/authorization for destructive remote changes, protections for bulk deletions, and recovery options. |
| P0 | Robust recovery and consistency | Resume safely after interruptions, disconnections, missing state, and partial operations. |
| P1 | Selective synchronization | Let users include/exclude individual folders within a configured account's synchronization scope. |
| P1 | Efficient background synchronization | Improve existing periodic checks with local filesystem events and incremental remote change detection where appropriate. |
| P1 | Sync status and history | Expose pending transfers, errors, last successful sync, and a useful activity log. |
| P1 | Enhanced multi-account experience | Improve status visibility and management across accounts; basic multi-account support already exists. |
| P1 | Shared content | Evaluate syncing folders shared with the user and support for Shared Drives, subject to Google Drive API constraints. |
| P2 | GNOME Files (Nautilus) integration | Explore file status emblems and relevant actions in the file manager. |
| P2 | Files on demand | Evaluate access to remote content without downloading every file locally. |
| P2 | Performance controls | Consider bandwidth limits and network/battery-aware behavior. |

P0/P1/P2 are preliminary planning priorities, **not** a release schedule or a claim that every item is unimplemented. Data safety and correctness remain requirements for enabling more destructive or fully bidirectional operations.

## Current limits

- DriveSync is under active development and has no stable release. The full workflow still needs continued validation in GNOME and the development Flatpak.
- Each account has one configured remote/local folder pair.
- Upload is opt-in and only creates new remote files and folders. Editing an existing remote file and propagating deletions are not supported.
- Conflict detection preserves data and reports attention; there is no complete UI for choosing between local and remote versions.
- Google Workspace documents are local `.desktop` links. Opening those links from GNOME Files, including GNOME's first-open trust behavior, still needs validation.
- Shared Drives and several other special or unsupported Drive item types are not synchronized as regular files.
- OAuth testing builds may be limited to accounts approved in the configured Google Cloud project. Public distribution also requires completing OAuth and Flatpak release preparation.

## Project approach

DriveSync is being developed around explicit contracts, conservative filesystem changes, and account-isolated state. Its MVP foundation is in place; subsequent features primarily involve application-level product decisions, UX, resilience, and edge cases rather than rebuilding the Google Drive integration. This does **not** mean the app is production-ready. The [MVP plan](docs/MVP.md) documents the intended acceptance scope; implementation and live GNOME/Flatpak validation may not yet cover every planned item.

## Feedback

Found a problem or have a suggestion? [Open an issue](https://github.com/rafatosta/drivesync/issues).

DriveSync is an independent project and is not affiliated with Google.
