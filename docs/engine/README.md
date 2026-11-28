# Engine

The Engine owns account-scoped synchronization and exposes public application contracts through `drivesync_engine`.

| Module | Responsibility |
| --- | --- |
| [Accounts](accounts/README.md) | Account identity and credential lifecycle |
| [Connection](connection/README.md) | Authentication and application lifecycle |
| [Providers](providers/README.md) | Provider-specific external APIs |
| [Detectors and Context](../OBSERVATION_SOURCES.md) | Scoped observations and inventory-enriched File models |
| [Inventory](../INVENTORY.md) | Account-isolated persistence, typed activity reporting, baselines and operation evidence |
| Analyzer | Pure decisions from File evidence; no I/O |
| [Orchestrator](../ORCHESTRATOR.md) | Coordinates analysis, execution, recovery and public status |
| [Operations](../OPERATIONS.md) | Verified effects, preconditions and persistence |

GTK consumes the public Engine boundary, not private synchronization modules. Observation telemetry is distinct from confirmed effects. Idle does not prove whole-tree convergence.
