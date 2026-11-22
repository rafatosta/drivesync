# Functional correction — Stage 0 baseline

Date: 2026-11-21 (America/Bahia).

## Scope and starting point

Stage 0 of `DRIVESYNC_PLANO_CORRECAO_FUNCIONAL_V1_0.md` is a starting-point
revalidation, not a synchronization behavior change. Only this stage was requested.
The V3.1 specification supplies contractual context; its reconstruction sequence
does not authorize restarting the architecture or implementing later stages.

- Repository: `/home/tosta/Projetos/drivesync`.
- Branch: `refactor/engine`.
- Initial HEAD: `280d8e74f766c2bcb50ff92c7c02beb1949de968`.
- Initial `git status --short`: empty. No preexisting changes to preserve.
- HEAD exactly matches the plan's audited commit; no intervening implementation
  differences require adjusting the stage.
- Read the supplied plan, V3.1 and root `AGENTS.md`. This delivery changes only
  documentation directly under `docs/`; nested engine documentation instructions
  do not apply to the changed files.

Input documents, kept unchanged outside the repository:

| Document | Absolute location | SHA-256 |
|---|---|---|
| Specification V3.1 | `/home/tosta/Documentos/DRIVESYNC_ESPECIFICACAO_MODELOS_DADOS_V3_1.md` | `364031fc583f8d5b290b847556281dd50e440ac05e459c9cd0a399b118455768` |
| Functional correction plan V1.0 | `/home/tosta/Documentos/DRIVESYNC_PLANO_CORRECAO_FUNCIONAL_V1_0.md` | `1cd50f75bf64e55dcba267c2746f10c195e8ebf0426c3673b46b01f831312ca4` |

## Focused source revalidation

This is not a repeated architectural compliance audit. The following checks
establish the immediate functional starting point for subsequent increments.

| Evidence | Current behavior | Follow-up stage |
|---|---|---|
| `engine/src/detectors/context/mod.rs`, `Context::build`; `cache.rs`, `Cache::consolidate` | A conclusive missing record has no logical item ID; current sides consolidate only through a persisted ID and `Found`. | 1–2 |
| `engine/src/detectors/context/models.rs`, `ContextConfig::environment` | Destination parents and collision evidence start as `NotCollected`. | 1 |
| `engine/src/inventory/mod.rs`, `Inventory` | Typed facade exposes per-item lookup/write, without scoped enumeration for reconciling unseen items or startup pending work. | 1, 3 |
| `engine/src/orchestrator/analyzer/mod.rs`, `analyze_unbased` | New unilateral items defer; adoption requires matching name, size, checksum and explicit authorization, including the binary metadata checks unsuitable for directories. | 2, 4–5 |
| `engine/src/connection/synchronization.rs` | Startup sets `initial_association_authorized: false`. Opening synchronization does not authorize adoption. | 2 |
| `engine/src/orchestrator/mod.rs`, `process` | `Adopt` becomes an `AdoptionRequired` blocker rather than a persistent association. Partial sources suspend dispatch; a fresh observation is required by the current flow. | 2, 6 |
| `engine/src/inventory/database/write.rs` | Initial insert requires `expected_revision == Some(0)`. No bootstrap identity flow is supplied by an anonymous `File`. | 2 |
| `engine/src/orchestrator/operations/mod.rs`, `baseline` | Confirmation checks direct root parent correspondence; nested parent associations are not resolved here. | 2 |

No contract refinement or runtime correction is introduced in Stage 0. Remaining
diagnoses in the plan are reference findings, not independently re-audited claims
of this delivery. Existing execution tests do not establish that initial creation
is reachable through the Engine's observation pipeline.

## Test environment and reproduction

The development manifest specifies `org.gnome.Sdk//50` and the `rust-stable`
extension. Both required installed components have origin `flathub`; no component
was installed or updated.

| Component | Verified value |
|---|---|
| GNOME SDK 50 commit | `612defccb665c82ca47ea9b17f855317c8714fbfe6773f511bc1bcc3e97a21ab` |
| Rust extension 25.08 commit | `09784a9989ae5a7fda6a2280033ca9aa32aac6907961da8b18be4590dd261507` |
| Cargo inside SDK | `1.98.1 (797e8a9bc 2026-08-05)` |
| rustc inside SDK | `1.98.1 (48a229cea 2026-09-01)` |
| Cargo executable | `/usr/lib/sdk/rust-stable/bin/cargo` |

Run each Cargo command below inside this wrapper (replace `<cargo command>`):

```sh
flatpak run --devel --unshare=network \
  --filesystem=/home/tosta/Projetos/drivesync \
  --filesystem=/home/tosta/.cargo \
  --env=CARGO_HOME=/home/tosta/.cargo \
  --command=sh org.gnome.Sdk//50 -c '
    export PATH=/usr/lib/sdk/rust-stable/bin:$PATH
    cd /home/tosta/Projetos/drivesync
    <cargo command>
  '
```

The first offline workspace-test attempt without explicit Cargo-cache access
failed during dependency resolution (`libadwaita` not found). Giving the SDK
access to the existing Cargo cache resolved this environment issue. No Rust host
fallback or dependency update was used. All successful Cargo commands used the
wrapper above. Test files/databases use the suite's temporary fixtures, not real
synchronization roots; opt-in desktop and keyring tests were not enabled.

## Validation results

| Command inside SDK | Observed result |
|---|---|
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets --offline --locked -- -D warnings` | Passed |
| `cargo test --workspace --offline --locked` | Passed: 599 tests, 0 failures, 7 ignored |
| `cargo doc --workspace --no-deps --document-private-items --offline --locked` | Passed |
| `cargo test --workspace --doc --offline --locked` | Passed; 0 doctests defined |

Workspace-test breakdown: application 20 passed / 6 ignored; engine unit tests
271 passed / 1 ignored; public Accounts integration 1 passed; simulated Google
Provider integration 307 passed; engine doctests 0 defined.

The seven ignored tests are:

- `application::background::startup::tests::denied_startup_and_late_success_preserve_manual_use`
- `application::background::startup::tests::session_launch_stays_hidden_and_launcher_activation_reopens`
- `application::background::tests::desktop_close_and_reopen`
- `application::tests::account_only_layout_and_initial_name_editor`
- `ui::components::remote_folder_picker::tests::native_picker_selection_cancel_and_preview`
- `ui::dashboard::pages::synchronization::tests::status_widget_presents_only_measured_progress_and_releases_selection`
- `accounts::tests::isolated_accounts_vault_roundtrip`

The first six require desktop/display/portal isolation or explicit autostart
opt-in; the last requires an isolated Secret Service, never the personal keyring.

Existing useful regression locations include Context tests (uncorrelated sides,
scope and baseline preservation), Analyzer tests (guarded updates, adoption,
conflicts and incomplete evidence), Inventory database tests (scope, revisions and
verified effects), Operations tests (durable uncertainty, replay refusal,
collisions, replaced root and persistence conflicts), Orchestrator tests (ordering,
source coverage, blocking and receipts), observation producer tests, and Connection
tests entering via Engine with temporary local trees and simulated remote sources.
The workspace command above executes these suites without opt-in ignored tests.

## Delivery and limits

Stage 0 records the baseline without modifying runtime code, contracts, real local
or remote files, credentials or existing user databases. Build outputs and Cargo
cache access are development artifacts. Automated checks do not homologate Google
Drive, the installed Flatpak, desktop integration or synchronization convergence.

The next dependency is Stage 1: sufficient capture, destination and scope
reconciliation evidence. It remains unimplemented by this delivery and requires a
subsequent request. A local documentation commit records this unit; no push, PR,
release or other remote publication is part of this stage.
