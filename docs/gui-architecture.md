# GUI architecture

The GTK4/Libadwaita application composes account and folder features through
the public `drivesync-engine` facade. The engine does not depend on GTK.
Network, OAuth and credential-vault work runs outside the GTK main thread.

`src/app` owns application lifecycle, navigation, restoration and the main
menu. `src/features/accounts` owns connection, folder selection and sign-out;
`src/features/dashboard` displays the selected account and its folders;
`src/features/preferences` owns global appearance and background settings.
Shared presentation helpers and desktop integration live in `src/ui` and
`src/core`.

The dashboard exposes account-specific actions and folder settings. It has no
controls for transfers, synchronization or conflict resolution. Background
execution keeps the application available and local observers running, but does
not synchronize files.

Each account page owns a temporary local activity list (names only, newest first,
200 entries). Filesystem work runs on cancellable workers; bounded channels apply
backpressure rather than silently losing batches. Hidden pages keep observing;
successfully saved folder changes restart only that account's session and clear
its list. Discarded pages and application shutdown cancel their workers. Coverage
warnings remain separate from file names. The list is session feedback, not a
durable history or a classification of synchronization operations.

GTK fixtures require a display and isolated profile. Their results do not prove
real Google Drive integration or installed Flatpak behavior. See the
[screenshot fixture instructions](../data/screenshots/README.md).
