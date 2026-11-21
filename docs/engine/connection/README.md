# Connection — account access exposed to the application

The application uses the `drivesync-engine` crate to open the account store, start or finish Google OAuth, list accounts, update a display name, disconnect an account explicitly, and browse remote folders. GTK never receives credential material or accesses Engine internals.

The connection layer composes the existing Accounts and Google Provider APIs. It owns the private Tokio executor, OAuth attempt lifecycle, cancellation, and the account removal guard. Accounts persist public metadata and credentials in their existing stores.

Folder setup and preferences remain application services. Remote folder selection uses the existing account-scoped Drive reads. These flows do not start background file synchronization.

A missing OAuth configuration prevents sign-in but does not reset or remove saved accounts. Closing setup preserves registered accounts; account removal requires the explicit confirmed sign-out action.

[Interfaces](INTERFACES.md) · [Tests](TESTS.md) · [Orientações](AGENTS.md)
