# UPLOAD-001/002 — Optional create-only synchronization

Policy approved for this increment: keep download-only as default. Enabling new
uploads requires explicit per-account browser consent. Never update, trash or
delete remote objects. Never upload historical download paths, staging files or
backups produced by the downloader. Existing destinations and ambiguous names
are not authorization to overwrite or adopt a file.

For each scoped client/account/folder pair:
1. Complete download reconciliation first; retain independent failures.
2. Lock and validate the local root against its persisted opt-in inode. Traverse descriptor-relative directories;
   reject symlinks/special files and bound traversal. Resolve remote parents by
   unique name and validated identity. Exclude historical downloaded paths.
3. Complete a descriptor-relative local inventory with one shared 500 ms
   quiescence window. Defer recognized editor/download temporaries, changing files
   and unknown evidence; ordinary hidden files and `.tmp` are not blanket exclusions.
   Revalidate the observed signature before snapshotting and compare the snapshot
   hash/identity and current signature with that observation. A quiescent pause
   cannot prove an external writer has finished. For each absent destination, generate a provider ID and persist intent BEFORE
   creation. Snapshot a regular local file into private staging, verifying inode,
   size and timestamps across the copy. Persist the original source identity;
   a replaced source must not be silently adopted even with identical content. Persist hashes and immutable staging ID.
4. Query the persisted remote ID before replay. Verify parent, name, type, size
   and MD5 before confirmation. Never generate a replacement ID for an intent.
5. Persist the resumable session URI privately. On restart query the server's
   offset; send aligned bounded chunks, validating ranges. Recheck the source
   signature before each chunk and its identity/hash before the final chunk;
   a detected resumed writer leaves the intent pending without final publication. An expired session
   may restart creation with the SAME ID after checking completion. Never log
   tokens, session URIs or raw provider responses.
6. Confirm only verified remote content. If the current local identity/content
   still matches the snapshot, record installed evidence for download replay.
   Preserve divergent local bytes as a conflict. Keep completed intent evidence
   so remote deletion does not recreate the item automatically.

Oracles: exact account/root scope; default off; explicit permission; duplicate
names; lost create response; lost chunk response; expired session; cancellation;
changed local file; symlink/root swap; repeated cycles/reopen; no overwrite/delete.
No algorithm claims atomic name uniqueness against independent Drive clients.

Primary references verified 2026-10-24:
- https://developers.google.com/workspace/drive/api/guides/manage-uploads
- https://developers.google.com/workspace/drive/api/guides/create-file
- https://developers.google.com/workspace/drive/api/reference/rest/v3/files/generateIds

Source revalidation is not a filesystem lock against other applications. A writer
can still change the source after the last check or after remote completion;
final confirmation rechecks local identity/bytes and never advances B over such
a divergent source. The private snapshot preserves exactly the previously
verified candidate; no cross-system atomic writer/publication guarantee is made.


## SYNC-013 — uncertain outcomes and bounded replay

The create-only executor retains its generated provider ID, immutable snapshot,
source identity and resumable session across errors. A failed request never
means remote absence. Replay first validates parent/name and queries that exact
ID; a different type, parent, name, size or checksum remains a conflict. A 404
alone does not establish deletion or authorize a replacement ID. A missing item
can only be retried with the original generated ID and verified snapshot/session.

DriveSync policy: each intent admits at most **five execution passes** (initial
pass plus four retries), not five chunks. Before remote parent/name/ID checks,
reserve the pass durably in SQLite, including a Unix-seconds deadline. The wait
is `60 * 2^(attempt - 1)` seconds plus random 0–30 seconds, capped at 960+30
seconds. A pass may transfer all chunks normally, with server-confirmed offsets
and existing local/parent revalidation. Any interruption consumes the reserved
pass, including a crash before the request, offline reads, quota/rate limiting
(403/429), and malformed replies. This deliberately conservative budget avoids
assuming an error had no effect. No in-pass automatic retry loop is introduced.

Deterministic transition algorithm:

1. Validate root, consent and local source using the existing root lock.
2. If confirmed, never recreate. If `now < not_before`, report `UploadDeferred`;
   make no upload-adapter parent, child, ID, session or content request.
3. If fewer than five passes were used, increment the counter. Otherwise admit
   only a read-only probe. In either case persist the next deadline BEFORE reads
   or effects. A wall-clock rollback postpones the operation.
4. Revalidate the parent and siblings, then query the persisted ID. Confirm only
   exact matching evidence; conflicting evidence remains pending and preserves
   both sides. Offline/unknown evidence never authorizes a write.
5. If not complete and the budget is exhausted, report `UploadRetryExhausted`.
   Preserve the original ID, session, snapshot and local file; do not reset the
   budget by restarting, toggling consent, or recreating a session. Later
   explicitly requested cycles can perform spaced read-only probes and confirm
   a late remote success, including success on the fifth pass.
6. Otherwise resume/create with the SAME ID. Query session position before
   resending bytes; an expired session requires another exact-ID completion
   check. Persist session and confirmation before advancing local evidence.

The limiter covers upload-intent execution, not general browsing, OAuth or the
coordinator's inventory/root checks. It does not implement a general API scheduler
or parse Retry-After. Permanent failures are surfaced immediately; later user
cycles still share the same finite budget. Exhaustion requires investigation;
there is no automatic reset or destructive cleanup action. Independent Drive
clients remain outside the cooperative root lock and name uniqueness guarantee.

`uploads.sqlite3` uses internal format **2**. Format 1 is rejected and preserved;
there is no automatic migration/reset. Do not delete old upload evidence to
recover: it can represent an already completed remote effect. Use the explicit
state-recovery workflow and retain its upload reconciliation block when evidence
cannot be verified.

References checked 2026-10-30:
- [Drive pre-generated IDs](https://developers.google.com/workspace/drive/api/guides/manage-uploads#pre-generated-ids):
  same-ID create retry is idempotent for binary files and folders; completed
  creates reject another create with 409. This does not cover Workspace conversion.
- [Drive resumable upload recovery](https://developers.google.com/workspace/drive/api/guides/manage-uploads#resume-upload):
  query completion/offset after interruption; do not assume an entire chunk arrived.
- [Drive error handling](https://developers.google.com/workspace/drive/api/guides/handle-errors#exponential-backoff):
  use bounded exponential backoff for rate limiting/server errors. The concrete
  pass limit and durable conservative delay above are DriveSync policy.

Test oracles are in `tests.rs`: nominal nested/empty files, N11 lost folder/file
confirmation, E04 lost partial chunk and offline reads, E08 finite attempts and
backoff, crash after reservation, concurrent worker exclusion, stale-state
rejection, expired sessions and repeated confirmation. They combine a simulated
provider/clock with real temporary filesystem and SQLite reopened between passes.
They do not prove Google behavior, process/power-loss durability or packaged UI
integration. Real-provider coverage remains tracked in issue #28.
