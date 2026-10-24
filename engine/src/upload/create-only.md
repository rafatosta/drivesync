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
3. For each absent destination, generate a provider ID and persist intent BEFORE
   creation. Snapshot a regular local file into private staging, verifying inode,
   size and timestamps across the copy. Persist the original source identity;
   a replaced source must not be silently adopted even with identical content. Persist hashes and immutable staging ID.
4. Query the persisted remote ID before replay. Verify parent, name, type, size
   and MD5 before confirmation. Never generate a replacement ID for an intent.
5. Persist the resumable session URI privately. On restart query the server's
   offset; send aligned bounded chunks, validating ranges. An expired session
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
