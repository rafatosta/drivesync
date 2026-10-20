# AUTH-001 / AUTH-002 test oracles

Tests use fixed synthetic identities only. GoogleApi and CredentialStore doubles
exist only under `cfg(test)`; production uses Google HTTPS and Secret Service.

| Case | Input / initial state | Expected effect |
|---|---|---|
| PKCE | RFC 7636 verifier | Exact published S256 challenge |
| Randomness | Two flows | Different state/verifier/challenge, valid verifier length |
| Callback | Correct host/path/state + code | Return code once |
| Forgery | Wrong state/host/path, POST, duplicates, absent/empty code | Reject; listener still accepts a later valid callback |
| Denial | Valid state + access_denied | Denied; no stored account credentials |
| Provider error | Valid state + server_error | ProviderUnavailable |
| Success | Valid callback, tokens and UserInfo | Real provider subject and optional verified email; stored before success |
| Bad code | invalid_grant on exchange | InvalidCode; no credentials persisted |
| Exchange failure | Network/5xx | Typed error; no success/account persisted |
| Cancellation | Before login / pending listener / before persistence | Cancelled, no success |
| Browser failure | Browser port returns failure | BrowserUnavailable, listener dropped |
| Timeout | No response before deadline | TimedOut, listener dropped |
| Permissions | Token response lacks required granted scopes | MissingPermissions |
| Email | Missing/unverified email | Account still identified by sub; no unverified email displayed |
| Storage | Vault unavailable or save fails | SecureStorage; no success reported |
| Renewal | Expired account, six callers | One refresh; all get same account, prior refresh retained if omitted |
| Restart | New provider, same store and ID | Reuse persisted credential |
| Isolation | A expired/invalid/offline; B valid | A fails explicitly, B still identifies, neither credential deleted |
| Client isolation | Another OAuth client ID | No credential from first client is loaded |
| Identity mismatch | Credential/UserInfo belongs to another subject, including renewal | IdentityMismatch; prior vault entry unchanged |
| Login overlap | Authorization already running | Busy before opening browser |
| Profile idempotency | Same subject, changed email, another subject | Update same account, preserve other account |

Real Google, browser portal, desktop vault persistence and Flatpak integration
remain manual checks in `docs/google-oauth.md`. These tests do not validate
AUTH-003 revocation, file scopes on existing folders, or synchronization.
