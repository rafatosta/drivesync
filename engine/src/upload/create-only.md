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
