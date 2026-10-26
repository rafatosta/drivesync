# State machines

Cycle: STOPPED → INITIALIZING → RECOVERING → IDLE ↔ PROCESSING; ERROR_TRANSIENT → RETRY_WAIT; STATE_CORRUPT → DEGRADED (destructive work disabled).

Operation: PLANNED → PERSISTED → RUNNING → VERIFYING → COMMITTED; branches RETRY_WAIT, BLOCKED, CONFLICT, CANCELLED. After crash: RUNNING → RECONCILE, not COMMITTED.

Observation: PRESENT, ABSENT, UNKNOWN. Unknown/permission failures must never be used as evidence of deletion.

Three-way matrix (baseline B, local L, remote R): neither changed → noop; only local changed → upload; only remote changed → download; both changed to distinct versions → conflict; changed vs deleted → conflict; both deleted confirmed → tombstone. Always revalidate destructive preconditions.
