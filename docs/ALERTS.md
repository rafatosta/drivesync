# Alerts

`src/ui/components/alerts/` provides summary cards, category lists and item
review pages within a dedicated caller-owned `adw::NavigationView`. All pages
remain inside the existing window. Hidden categories release their width to the
other card. Empty summaries are hidden; open empty lists show an empty state.

## Conflicts

A conflict requires a decision between file versions. Activating a list row opens
an account-qualified detail page with the filename, explanation, local and Drive
modification times and sizes. Unknown metadata is explicitly unavailable.
Version cards wrap on narrow displays. Radio choices explain each consequence:
keep both with different names (default), replace Drive from local, or replace
local from Drive. Only Confirm emits an intent; Cancel, back and close do not.

## Problems

Only persistent impediments requiring user intervention belong here. Typed
categories are authentication, permission, storage and local-folder availability.
Their pages explain the required adjustment and offer reconnection, permission
review, freeing space or selecting a folder. Transient network/retry states do
not belong in these lists. Problems never display version-resolution controls.

## Presentation contract

`Alerts::update(&[AlertItem])` replaces counts and lists. `AlertDetails` distinguishes
conflict metadata from a typed problem; IDs must be account-qualified and unique.
Titles and descriptions are literal text. The callback emits `(item_id, action_id)`
on GTK's main thread. A fresh update invalidates any open detail page and returns
to the list; retained stale controls and repeated confirmations cannot emit again.
Producers must still revalidate the intent before executing actual operations.

Conflict action IDs are `both`, `local`, `remote`; problem IDs are `reconnect`,
`permissions`, `storage`, `folder`. No file content or engine command is retained.

Developer Overview uses synthetic files, account labels and metadata. Its
simulation controls include every problem category. Confirmed preview intents
remove synthetic items and update SyncStatus and alert counts without touching
real files or asserting synchronization. Engine integration remains separate.
