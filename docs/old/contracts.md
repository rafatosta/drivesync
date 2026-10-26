# Global contracts

Ports: AuthProvider.authorize/renew/revoke; RemoteStorage.list/stat/changes/download/upload/trash; LocalInventory.scan/observe/watch; StateRepository.stage/commit/load; Planner.plan(base,local,remote); ConflictPolicy.preserve/resolve; OperationExecutor.execute; StatusPort.snapshot/subscribe.

Errors: InvalidInput, NotFound, Unauthorized, Revoked, RateLimited, TransientNetwork, PermissionDenied, DiskFull, Integrity, ConcurrentModification, Conflict, UnknownState, CorruptState, Cancelled, Unsupported.

Each contract documents typed inputs, outputs, side effects, timeout, idempotency, and pre/postconditions. All algorithms remain language-neutral.
