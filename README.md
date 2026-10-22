# DriveSync

**Google Drive for the GNOME desktop.**

DriveSync is an open-source Linux application designed to make it easy to access your Google Drive files from your computer.

The goal is to let you choose which Google Drive folders to keep on your computer, with automatic, **download-only** synchronization. Your local changes won't be uploaded to Google Drive.

> **Early development:** DriveSync is a technical preview. File downloads and synchronization are **not working yet**. Do not rely on it for important files.

## What works today

- Connect a Google account through your web browser (for authorized testers using a configured build).
- Use more than one Google account.
- Restore saved accounts when reopening the app and see connection problems.
- Browse folders in Google Drive.
- Choose a Google Drive folder and a local destination.
- Save folder settings and view them in the dashboard.
- Disconnect an account without deleting files from your computer or Google Drive.

## What's coming

- Download files from selected Google Drive folders.
- Keep downloaded files up to date automatically.
- Make downloaded files available offline.
- Show transfer progress and errors.
- Handle interrupted downloads safely.
- Run in the background and integrate more closely with GNOME.

**Not planned for the first release:** uploading local changes, two-way synchronization, or automatically deleting local files when something is deleted from Google Drive.

## Try DriveSync

DriveSync is not ready for everyday use, but you can explore its current features with **GNOME Builder**:

1. Clone this repository: `https://github.com/rafatosta/drivesync.git`.
2. Open the project folder in **GNOME Builder**.
3. Click **Run** to build and launch the development app.

**Want to test Google sign-in?** The Google OAuth integration is currently limited to authorized test accounts. [Open a GitHub issue](https://github.com/rafatosta/drivesync/issues/new) requesting tester access and provide the Google account email you'd like to use. The maintainer will add eligible accounts to the test-user list.

GitHub issues are public: only share an email address you're comfortable making public. A development build without the testing OAuth configuration can still show the interface, but Google login will be unavailable until configured. See [build profiles](docs/build-profiles.md) for details.

## Development philosophy

DriveSync starts with a problem to solve, not a preferred programming language. The maintainer defines the goals, discusses technical options, plans the architecture, and checks the results. AI agents help with those discussions and write the Rust code—similar to delegating implementation tasks within a development team.

The focus is on **engineering a reliable, maintainable product**, rather than simply generating code. AI-generated work still needs review and testing, and the maintainer remains responsible for the project. Read more in the [development philosophy](docs/DEVELOPMENT_PHILOSOPHY.md).

## Feedback

Found a problem or have a suggestion? [Open an issue](https://github.com/rafatosta/drivesync/issues).

DriveSync is an independent project and is not affiliated with Google.
