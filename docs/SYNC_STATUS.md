# SyncStatus

`src/ui/components/sync_status/` contains the GTK4/libadwaita presentation card.
It has no engine dependency, polling, operations, inventory copy or file list.
The Developer Console Overview provides isolated, button-driven scenarios for
all activities, determinate and indeterminate progress, and multiple accounts.
Overview now acts as a home-page preview, opening with the attention example;
simulation controls are kept in a collapsed expander below the card.

A presentation adapter calls `SyncStatus::update(&SyncStatusData)` on the GTK
main thread whenever a state notification arrives. Background producers must
marshal their events onto that thread before updating GTK. The card itself
owns no subscription or timer. `SyncStatusData::aggregate` consumes one current
snapshot per configured account in stable order. Attention counts are summed
with saturation and take precedence; otherwise the first executing activity wins. Its counters are never combined with unrelated operations. Producers must
include unknown accounts, remove disconnected accounts and invalidate old
confirmations when activity or knowledge changes. The adapter owns event ordering
and rejects notifications from replaced sessions.

Byte progress takes priority over item counts. Missing or zero totals do not
produce percentages; a spinner indicates ongoing work. Counts are clamped to
their totals. Idle snapshots hide progress and the spinner. Only explicit
`synchronized: true` with no activity or attention shows the success message. Unknown idle
state hides the card. Empty account sets do not imply synchronization.

The production engine currently exposes execution/preparation diagnostics but
no public global-synchronization confirmation or unified activity subscription.
This change intentionally does not infer synchronization from those diagnostics
or poll them. The card is exercised in Developer only; connecting real account
activity requires a producer with the above guarantees. No internal engine
contracts were changed.

## Attention state

Nonzero `conflicts` or `problems` display a warning icon and localized summary,
without progress, file lists, technical errors or actions. This explicit product
state supersedes the initial restriction against conflict summaries. Zero-count
categories are omitted and singular/plural clauses are localized independently.
Attention overrides active and confirmed-success snapshots; clearing both counts
restores the activity or idle presentation. Counts remain simulated in Developer;
no engine integration is introduced.
