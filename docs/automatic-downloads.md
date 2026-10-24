# Automatic download behavior

Configured accounts are checked three seconds after registration and every minute
**after the previous cycle completes**. Opening an existing account again or
rebuilding the dashboard reuses its application-owned job. The explicit
synchronization button uses that same job. Hiding the window leaves scheduling
active; Quit cancels all running jobs. Session startup is registered separately through the native Background portal;
its `--background` launch follows the same scheduling path.

At most one worker runs per account. Reconfiguration and account removal cancel
the previous job and retain its guard until it stops; obsolete results cannot
replace the state of a new configuration. Engine journal and filesystem locks
remain the final safety boundary across processes and local-root aliases.

Transient transport/provider failures, rate limits, busy resources and remotely
changing items retry after 30, 60, 120, 240 and at most 300 seconds. This includes
per-item failures in a partially completed report. Success resets backoff. The
native network monitor is only a hint: actual engine results remain authoritative.
A connectivity-restored signal retries network failures promptly, but does not
bypass rate-limit backoff, explicit pause or blocking errors.

Cancellation pauses automatic checks for that account until the user starts
synchronization again (or explicitly saves its folder configuration). Pause is an
in-memory session state; restarting the application schedules configured accounts
again. Invalid folders, revoked authorization, integrity, storage and local I/O
failures require correction/manual retry. Temporary startup authentication
transport failures remain eligible for retry. No error triggers automatic login,
credential removal or destructive repairs.

Local conflicts and skipped unsupported items remain visible as needing attention;
ordinary checks continue. Previously observed paths changed or no longer listed
in the selected remote root are informational activity, never pending failures.
They require no acknowledgment and cannot cause retry/backoff or an attention state.
The collapsed native activity view retains full previous and known current paths.
Local edits and previous copies are preserved using the existing engine behavior.
The default download-only mode never uploads, removes local copies or resolves
conflicts automatically.

An account can explicitly enable **Upload new files and folders**. This requires
additional browser consent for that exact account. The same single-flight job
then performs download reconciliation followed by create-only upload. Only absent
destinations are created; existing remote files are never updated and deletions
are never propagated. Folder-pair opt-in and upload intents are durable and scoped
by client, account and folder configuration. The selected local inode is verified
before upload. See [create-only contract](../engine/src/upload/create-only.md).
Signing out disables the folder pair's upload setting without deleting files.

## Validation

The deterministic scheduling tests use explicit timestamps instead of waiting for
minute-long deadlines. Scripted worker tests exercise overlap rejection, network
retry, cancellation acknowledgement, stale configuration results and shutdown
without credentials or user files. Existing engine tests cover incremental remote
changes, conflicts, interrupted installation and durable recovery.

The ignored `automatic::runtime::tests::hidden_window_runs_automatic_cycles` test
requires the matching desktop sandbox, session bus and background permission, with
`DRIVESYNC_BACKGROUND_TEST=1`. It injects a network failure and a conflict report,
uses the real application timer and simulates a connectivity-restored signal while
the window is hidden. It never initializes the engine or accesses account data.
The regular GTK layout fixture uses isolated manual controllers so preview windows
cannot start automatic network jobs.
