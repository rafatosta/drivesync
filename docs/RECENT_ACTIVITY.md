# Recent activity

`src/ui/components/recent_activity/` is a presentation-only component. The home
preview shows the latest three completed successful activities with operation,
relative time, optional size and completion icon. `View all` opens the full
supplied snapshot on an `adw::NavigationView` page in the same window, with native
back navigation. History adds account, path and absolute local date/time.

`ActivityItem` contains an account-qualified event ID, name, path, account label,
kind (uploaded, downloaded, updated), optional bytes and UTC Unix completion time.
Running/queued operations and failures do not belong in this contract. Completion
of an activity never implies overall synchronization. No engine queries, database,
background task or file operations are introduced.

`RecentActivity::update(items, now)` replaces both lists, including an open
history. Snapshots are ordered newest-first, with IDs breaking timestamp ties;
producers supply unique IDs and a bounded history suitable for rendering. The
preview retains three rows; the detail page renders every supplied entry in a
scrollable list. This is not a durable history or pagination API.

Relative times use the supplied snapshot time, with localized minute/hour/day
plurals, a just-now state, and future timestamps clamped to just now. Producers
refresh snapshots as appropriate; the component does not create a timer. Missing
sizes display a dash, invalid absolute timestamps show an unavailable label,
and file/account/path text is never interpreted as markup.

Developer Overview uses five synthetic activities and includes populated/empty
simulation controls. Activity history is independent of alert adjustments and
current SyncStatus. The home page now scrolls to accommodate all components.
