# Engine

The Engine currently provides account metadata and authentication workflows, plus provider APIs used by those workflows and remote folder browsing.

| Module | Responsibility |
| --- | --- |
| [Accounts](accounts/README.md) | Persisted account identity and credential lifecycle |
| [Connection](connection/README.md) | Google sign-in and account connection lifecycle |
| [Providers](providers/README.md) | Google OAuth and Drive API access |

Synchronization implementation and its exclusive data contracts have been removed.
