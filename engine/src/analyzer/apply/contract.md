# Apply contract (behavior-first)

Apply materializes Decision, never reclassifies it. Inventory is an adapter for
facts authored by Analyzer; its existing schema is deliberately not a limit.

## Durable effect boundary

1. Validate the complete decision, target cardinality, scope, and intents.
2. Read the exact delivery receipt. Reject mismatched File identity; skip a
   completed identical receipt before revision checks.
3. Read current raw guards; stale state requires durable reanalysis.
4. Retain the original File, except never replace newer facts with obsolete data.
5. For nonmutating outcomes record the decision or request. For baseline
   establishment verify both sides and atomically confirm.
6. For actionable work plan using existing OperationCommand and ExecutionIntent.
   Verify live source/destination, reserve and claim under guards before dispatch.
7. Persist actual verified effects; never substitute an old physical identity.
   Unknown or applied-but-unverified effects enter recovery, not blind retry.
8. After one verified step of a multi-intent decision request reanalysis; do not
   execute dependent commands using obsolete preconditions.
9. Complete each durable target outcome and finally the delivery. Only then Ok
   authorizes acknowledgement; persistence failure returns Err.

Retries preserve immutable ExecutionId and command. No-effect failures can be
retried within finite bounds. Unknown effects require checkpoint recovery or
new verification before repetition. No rollback of confirmed physical effects.
Permanent deletion is prohibited; Trash requires target-bound authorization.
Restore needs an explicit future user directive, not implicit fallback.

## Deferred review

This contract specifies required observable facts, not the implementation of
their production or storage. Source/destination evidence, parent bindings,
baseline proof, retry scheduling and durable authorization provenance will be
mapped after the complete Analyzer review. The fakes return scripted values and
record calls, not guaranteed realistic physical effects. Several cases exercise
only dispatch guards or persistence ordering; multi-step continuation deliberately
requests new analysis rather than executing a second obsolete command. Retry
limit and cancellation ports need concrete policy before production integration.
The existing Operator interface is reused; neither Operator nor Inventory is
modified. No algorithms are implemented.

## RED

cargo test -p drivesync-engine --test analyzer_contracts apply_behavior -- --ignored

Behavior tests remain ignored until deliberately run. Structural tests are active.
