# DriveSync

**Google Drive for the GNOME desktop.**

DriveSync is an early-stage Linux application for keeping selected Google Drive folders available on this computer. It is a technical preview, not a stable release. Test with files you can safely replace or recreate; DriveSync is not a backup service.

## Screenshots

Manage multiple accounts and configure a separate Google Drive and local folder for each one.

![DriveSync dashboard with three fictional accounts and personal folder settings](data/screenshots/04-dashboard-pessoal-1200.png)

| Choose a Google Drive folder | Review synchronization activity |
| --- | --- |
| ![Google Drive folder chooser with fictional folders](data/screenshots/10-selecionar-pasta-drive-1200.png) | ![Synchronization activity showing remote moves and deletions while preserving local copies](data/screenshots/download-history-expanded-720.png) |

These screenshots use fictional data. Synchronization states are simulated using the current interface components.

[View all screenshots, including welcome, connection, transfers, and compact layouts](data/screenshots/README.md) · [Regenerate screenshots](data/screenshots/README.md#gerar-novamente)

## What you can do today

DriveSync focuses on keeping local copies up to date with Google Drive. It does not yet offer complete two-way synchronization.

| Feature | Current availability and limits |
| --- | --- |
| Google accounts | Connect and manage multiple accounts, each with its own settings and credentials. |
| Folder selection | Choose one Google Drive folder and one local folder per account. Local folders for different accounts cannot overlap. |
| Downloads | Download files and subfolders, update managed local copies when remote files change, and keep previous versions. |
| Uploads | Optionally upload new files and folders. Changes to existing files and deletions are not sent to Google Drive. |
| Automatic checks | Check for remote changes periodically, or sync on demand. |
| Pause and resume | Pause a sync and resume later, preserving completed work. |
| Background operation | Continue checking while the app is allowed to run in the background. Session startup depends on desktop permission. |
| Conflicts and deletions | Preserve local files when changes conflict or remote files are removed. Conflicts need manual attention; there is no complete resolution interface yet. |
| Recovery | Save progress for later runs and check interrupted work. Recovery after failures or missing sync data still needs broader testing. |
| Google Docs, Sheets, and Slides | Create local links that open documents in the browser. Opening these links from GNOME Files still needs validation. |
| Status and activity | Show progress, account status, and sync activity. Preferences and notifications remain limited. |
| Shared Drives and special items | Shared Drives and some special Google Drive item types are not supported. |

## Preview availability

The full workflow still needs further testing in GNOME and the development Flatpak before a stable release. Google sign-in in testing builds may be restricted to approved test accounts. Public distribution requires further sign-in approval and packaging work.

## What's next

The immediate focus is clearer first-time setup, more reliable recovery, better conflict handling, and validation of background operation and document links on GNOME. Updating existing files on Google Drive and propagating deletions require additional safeguards before they can be enabled.

Other possible improvements include selecting individual subfolders, faster detection of local changes, richer activity history and notifications, improved account management, support for shared content, file status indicators in GNOME Files, files on demand, and bandwidth or battery controls. These are candidate improvements, with no committed release schedule.

## Feedback

Found a problem or have a suggestion? [Open an issue](https://github.com/rafatosta/drivesync/issues).

DriveSync is an independent project and is not affiliated with Google.
