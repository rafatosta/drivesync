# Global sync cycle

1. Acquire exclusive root lease.
2. Validate config, account and ledger.
3. Reconcile ambiguous/in-flight operations.
4. Stage remote paginated changes and local inventory; persist before cursor movement.
5. Construct B/L/R observations by stable identity.
6. Pure planner emits action, conflict or deferred item.
7. Persist plan with preconditions.
8. Revalidate and execute safely via staging, upload sessions and integrity checks.
9. Commit new baseline only after verified effects; publish progress.
10. Retry transient failures with bounded backoff and no false deletion.
