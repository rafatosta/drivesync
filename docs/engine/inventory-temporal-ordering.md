# Inventory observation ordering

Inventory decides whether an observation may update persisted state using markers stored per item and side (local or remote). The durable order is scoped to one synchronization and is allocated by the Detector's SQLite causal clock. Scanner reserves from that same clock before collecting a local entry; Local Detector reserves before collecting a notification result; remote records receive a position before durable queue insertion; and Operator results reserve after the physical effect has been verified.

For local observations, Inventory compares only the durable causal positions. A greater position may update the local side; a smaller position is obsolete. An absent position, or a side without a comparable saved position, is incomparable. Generation labels and system timestamps do not establish order.

For remote observations, Inventory first compares decimal provider versions when both sides carry one. A greater version may update state and a smaller version is obsolete. When versions are missing or not comparable, Inventory uses the same durable causal position rule as the local side. Provider versions are stored as decimal text and compared numerically.

An obsolete observation receives a durable `Obsolete` receipt. An incomparable observation receives a durable `Incomparable` receipt, leaves the item and optimistic revision unchanged, and requests one targeted revalidation from the Detector. Replaying either receipt is idempotent; a replay does not request another revalidation. Applied observations preserve the existing optimistic revision check and write the new side marker in the same Inventory transaction as the state change and receipt.

The causal clock is incremented in an immediate SQLite transaction and survives application restarts. Pending queue records retain their allocated position across delivery retries and restart. Scanner and Local Detector can reserve concurrently: SQLite serializes reservations, providing a shared frontier without making either module depend on the other. An event without comparable evidence is never ordered by arrival time or `observed_at`.

The order is evidence about when each producer began an observation or confirmed a verified result. It is not a conflict-resolution or operation-execution policy. Revalidation is localized through the responsible Detector and does not restart Scanner in continuous operation.
