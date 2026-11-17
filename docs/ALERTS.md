# Alerts

`src/ui/components/alerts/` provides a single pair of summary buttons and two
review pages installed on a caller-owned `adw::NavigationView`. These pages
remain inside the existing window and use native back navigation. The component
must be installed once on a dedicated view, with unique page tags.

`Alerts::update(&[AlertItem])` replaces both lists and their counts, including an
open review page. Categories with no items are hidden; GtkBox homogeneous layout
shares the width equally when both exist and gives the visible button all space
when only one exists. With no alerts, the summary is hidden and an open review
page displays an empty state. Returning home preserves the current preview.

Each item has a qualified ID, category, display-safe title/detail and supplied
actions. Expanding a row exposes its actions. Titles and details are literal
text, not markup. A click emits `(item_id, action_id)` on the GTK thread. The
component performs no IO, resolves no conflicts and does not infer success.
A future engine adapter must revalidate identities and available actions before
executing an intent; the component is not authorization to replace files.

Developer Overview is the simulated home page. Its scenarios create synthetic
items alongside SyncStatus. Review choices remove only those synthetic items
and update both components. Clearing all alerts does not assert synchronized
state. Conflict choices and retry buttons are explicitly marked as simulations.
There is no engine integration or real file modification.
