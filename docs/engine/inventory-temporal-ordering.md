# Inventory observation ordering

Inventory decides whether an observation may update persisted state using
markers stored per item and side (local or remote). The durable order is scoped
to one synchronization and allocated by the Detector's SQLite causal clock.
Local and Remote producers reserve positions before collecting metadata.
Operator verification reserves a position separately for each side before
collecting that side's metadata. Inventory persistence never creates evidence
of physical freshness. A versionless remote response without a comparable
collection position remains uncertain; inserting it into the durable event
queue does not make it newer.

For local observations, Inventory compares durable causal positions. A greater
position may update the local side; a smaller position is obsolete. A missing
position, or a side without a comparable saved position, is incomparable.
Generation labels and system timestamps do not establish order.

For remote observations, Inventory first compares decimal provider versions
when both sides carry one. A greater version may update state and a smaller
version is obsolete. Successful Operator verification stores the verified
provider version with that side's pre-collection causal position, so a queued
older version cannot replace a newer result confirmed by the Operator. When
versions are missing or not comparable, Inventory uses a comparable durable
causal position; otherwise it records uncertainty. Provider versions are
stored as decimal text and compared numerically.

An obsolete observation receives a durable `Obsolete` receipt. An incomparable
observation receives a durable `Incomparable` receipt and a localized Inventory
revalidation record keyed by item and side. The Orchestrator requests that
revalidation through the Detector's public interface. The flow replays
persisted revalidation records after restart. A comparable observation removes
the record in the same transaction that applies its state and receipt. The
event lease is acknowledged only after the request and required analysis
succeed. If processing fails before acknowledgement, the Detector event is
redelivered; its Inventory receipt makes the action idempotent while allowing
interrupted work to run again. Applied observations preserve the optimistic
revision check and write the new side marker in the same Inventory transaction
as the state change and receipt.

The causal clock is incremented in an immediate SQLite transaction and
survives application restarts. Pending queue records retain producer-assigned
positions across delivery retries and restart. Local and Remote observations
can reserve concurrently: SQLite serializes reservations, providing one shared
frontier without coupling their implementations. Remote events without
provider versions are not assigned a position at queue insertion because
concurrent provider queries and earlier pending results make insertion order
insufficient proof. Revalidation work is persisted in Inventory schema 11;
older Inventory schemas are rejected without migration in this development
phase. An event without comparable evidence is never ordered by arrival time or
`observed_at`.

The order is evidence about when a producer began an observation or when a
verified operation result was collected. It is not a conflict-resolution or
operation-execution policy. Revalidation remains localized through the
Detector and preserves causal protections for late events, retries, restarts,
remote versions, Operator confirmations, and concurrent changes.
