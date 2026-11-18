# Detector operational status

The public `drivesync_engine::detectors::Detector` composes Local and Remote
privately, selects the remote specialist from Accounts, and owns one durable
SQLite queue at `<local-root>/.drivesync/detector.sqlite3`. The queue is scoped
to the account, SyncId, and configured roots. Google is the implemented remote
specialist; unsupported providers fail explicitly.

The public lifecycle includes `new`, `start`, `pause`, `resume`, `stop`,
`request_scan`, `status`, `take_events`, `acknowledge`, and `release_events`.
`request_scan` is a generic targeted reobservation request to the active
producers. It does not access Inventory or choose synchronization actions.

- `start` starts Local and Remote together. A source failure is reported
  separately and does not conceal the health of the other source.
- `pause` and `resume` control event delivery only. Filesystem watching,
  provider polling, and durable persistence continue during a delivery pause.
- `stop` pauses delivery and stops both sources while preserving queued events
  and remote checkpoints. A period while stopped is not treated as proof of
  absence when the Detector restarts.
- `status` reads cached source and queue state without HTTP, filesystem, or SQL
  work. Local, Remote, delivery, and queue errors remain distinguishable.
- Queue batches may contain both origins. Acknowledgement or release applies
  to the whole leased batch. Expired leases make events available for
  redelivery; event IDs and payloads remain stable.
- Availability callbacks run after queue commit. They signal that work may be
  available; they neither deliver nor confirm observations.

Local and Remote emit the shared `File` contract to the same queue. A
`FileBatch` preserves complete event payloads and causal positions across
delivery retries. The queue's commit order is not a claim about causal
freshness. Inventory applies per-item causal positions, provider versions, and
durable receipts. The Detector does not own reconciliation, conflict policy,
baseline, or file operations.

The Orchestrator starts the Detector with delivery paused, opens Inventory,
then starts one consumer. It persists each event and calls the Analyzer for
affected resources. Detector confirmation follows successful processing. The
Analyzer owns dispatch policy; the Operator executes and verifies requested
operations.

Changes made locally while DriveSync is closed remain outside the current
scope. A future retrospective local-observation strategy will be developed
from scratch as a third internal Detector producer. It will use the generic
observation contract and will be scheduled by the Detector, not directly by
the Orchestrator.

Detector and Inventory use separate SQLite databases. Re-delivery and
Inventory receipts provide recovery across the write boundary; no distributed
transaction is claimed. Older development database formats are rejected as
documented by their owners, with no automatic migration or deletion.

## Validation boundaries

Local and Remote unit tests use temporary roots, fake providers, and durable
SQLite storage. They cover simultaneous sources, failure reporting, pause and
resume, mixed batches, acknowledgements, release, redelivery, concurrency,
and remote recovery. This does not validate real Google credentials, desktop
portal permissions, packaged Flatpak installation, or live Drive behavior.
