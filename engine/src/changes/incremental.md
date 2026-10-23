# CHANGES-002 — Durable download-only remote observations

## Scope and policy

The manual cycle consumes the account-level My Drive change feed. A checkpoint
projects that feed onto one OAuth client, account, resolved remote root and local
root. Query filters are fixed (`spaces=drive`, removed entries included, shared
drives excluded). A root never reuses another root's cursor without its matching
inventory. Local monitoring, scheduling, upload and deletion are outside scope.

## Algorithm

1. Validate configuration and remote root. Load the scoped checkpoint, failing
   closed on incompatible/corrupt storage. Keep the download journal unchanged.
2. With no checkpoint, capture startPageToken BEFORE traversing the root.
3. Drain changes from the saved/captured token, including empty intermediate
   pages. Reject repeated tokens, malformed/incomplete envelopes and limits.
   Only a terminal newStartPageToken completes a batch. Last event per ID wins.
4. Apply file additions/revisions/removals to the saved ID graph. A removed or
   inaccessible item is unavailable evidence, never permission to delete locally.
   Folder topology changes trigger a full reconciliation because descendants
   need not receive individual events. Capture a new token before that scan,
   then reconcile its window too. Bound restarts; fail without advancing on an
   unstable tree. HTTP 410 or an explicit invalid `pageToken` parameter triggers
   the same safe bootstrap; other errors fail. Google documents these tokens as
   non-expiring; reconciliation handles rejection, not scheduled expiration.
5. Preserve notices for previously observed paths no longer represented. A
   returning identical path clears its notice. Save the complete inventory,
   notices and terminal cursor in ONE SQLite transaction with compare-and-swap
   revision. A racing collector fails; it cannot overwrite a newer checkpoint.
6. Build a local plan from every cached entry and execute existing verified
   downloads. Saving observations is NOT confirming installation. Failed,
   cancelled and conflicted downloads remain discoverable next cycle even if
   the remote feed has no new events. Local deletions are also rechecked.

Checkpoint storage is a separate versioned SQLite database, not a migration of
existing download state. Opening, privacy, integrity and durability rules are
shared. A crash before checkpoint commit replays the batch; after commit it
replays the inventory against the download journal. No cross-database atomic
commit is required: observation persistence always precedes effects.

## Oracles

- Token captured before initial scan; an in-window revision wins over inventory.
- Empty/multiple pages, repeated tokens, malformed bodies, cancellation and
  network/auth errors never publish a partial checkpoint.
- An unchanged second cycle performs no files.list traversal; pending downloads
  and missing local copies remain eligible. Unchanged journal-confirmed files
  still require inode/hash verification, but need no per-item metadata requests.
  Mutating operations retain provider revalidation.
- New files, duplicate events, folder moves/additions, subtree removals and
  unrelated account events produce deterministic projected observations.
- Invalid cursor rescans without deleting local files; corrupt state never resets.
- Cursor/inventory commit rolls back together on injected failure; reopening and
  concurrent stale commits preserve the latest complete checkpoint.
- Client/account/root/local-root isolation; remote removal preserves local bytes.

## Primary references (verified 2026-10-23)

- https://developers.google.com/workspace/drive/api/guides/manage-changes
- https://developers.google.com/workspace/drive/api/reference/rest/v3/changes/list
- https://developers.google.com/workspace/drive/api/reference/rest/v3/changes/getStartPageToken
- https://www.sqlite.org/atomiccommit.html

The feed reports current metadata, not immutable historical file bytes. It is
not a snapshot; existing download metadata/integrity checks remain mandatory.
