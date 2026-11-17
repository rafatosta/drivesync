# SyncStatus

`src/ui/components/sync_status/` contains the GTK4/libadwaita presentation card.
It has no engine dependency, polling, operations, inventory copy or file list.
The Developer Console Overview provides isolated, button-driven scenarios for
all activities, determinate and indeterminate progress, and multiple accounts.
Overview now acts as a home-page preview, opening with the attention example;
simulation controls are kept in a collapsed expander below the card.

A presentation adapter calls `SyncStatus::update(&SyncStatusData)` on the GTK
main thread whenever a state notification arrives. The account page subscribes
to the selected account through `Engine::observe_synchronization`; replacing the
selection aborts the previous subscription. The adapter maps engine activity
and byte progress to the presentation model. The card itself owns no
subscription or timer. `SyncStatusData::aggregate` remains available to
multi-account producers and consumes one current snapshot per configured
account in stable order. Attention counts are summed with saturation and take
precedence; otherwise the first executing activity wins. Its counters are never
combined with unrelated operations.

Byte progress takes priority over item counts. Missing or zero totals do not
produce percentages; a spinner indicates ongoing work. Counts are clamped to
their totals. Idle snapshots hide progress and the spinner. Only explicit
`synchronized: true` with no activity or attention shows the success message.
Unknown idle state shows a neutral status message and never claims completion.
The account page hides the card only when no account is selected. Empty account
sets do not imply synchronization.

The engine subscription is account scoped and read-only. The current engine
does not provide an authoritative whole-account completion proof, so the
account page never presents confirmed success from idle or preparation
readiness. It also does not provide attention counts; those remain available to
other producers but are not fabricated by this adapter. The Developer Console
continues to provide isolated, button-driven presentation scenarios.

## Attention state

Nonzero `conflicts` or `problems` display a warning icon and localized summary,
without progress, file lists, technical errors or actions. This explicit product
state supersedes the initial restriction against conflict summaries. Zero-count
categories are omitted and singular/plural clauses are localized independently.
Attention overrides active and confirmed-success snapshots; clearing both counts
restores the activity or idle presentation. Counts remain simulated in Developer;
no engine integration is introduced.
