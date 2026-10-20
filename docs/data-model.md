# Conceptual data model

`Account(id, provider_id, auth_state)`; `SyncRoot(account_id, root_local, root_remote_id, policy)`; `Identity(remote_id, local_id?, parent_remote_id, display_path)`; `Observation(status=PRESENT|ABSENT|UNKNOWN, revision?, hash?, size?, mtime?)`; `BaseVersion(remote_version?, local_fingerprint?)`; `ChangeEvent(source, identity, kind, seq)`; `Plan(action, expected_local, expected_remote, dependencies, destructive)`; `Operation(id, plan_id, state, attempts)`; `Conflict(base, local, remote, preserved_versions)`; `Cursor(page_token, generation)`.

Remote ID is authoritative for remote identity; rename must not be inferred only from name. A failed API call yields UNKNOWN, never ABSENT.
