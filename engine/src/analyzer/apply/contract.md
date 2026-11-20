# Apply contract — complete selected decision

Persist the original observation and decision. Validate scope, targets, guards,
authorization and delivery receipt. Only selected intents permit physical
mutation. Operator does not supply initial analysis evidence; its own checks
protect execution. Reserve and atomically claim an immutable ExecutionId
before effects. Retry or recover the same execution when safe; unknown
effects never permit blind replay. Record verified returned identities.

Execute every compatible selected intent in dependency order, refreshing
guards from verified results. If invalidated, interrupt the remaining work and
record completed effects. No rollback of confirmed effects, no callback to
Decision/Context/Detector, no Resolve. InsufficientEvidence is a durable
terminal diagnosis. Interrupted is a terminal outcome, not a reanalysis
request. Baseline confirmation requires actual convergence. Acknowledge only
after durability; success does not imply Synced. Storage is a mechanical
adapter, not a source of synchronization decisions.
