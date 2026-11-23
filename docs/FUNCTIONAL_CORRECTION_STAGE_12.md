# Stage 12 — GUI components and event 1

Scope: mandatory component groundwork and existing remote file download only. Events 2–14
are not implemented or validated by this activity. The attached Stage 12 specification is
the acceptance reference. English identifiers and gettext strings follow repository conventions.

### Evento: Mandatory component groundwork
- Estado inicial / fixture / escopo: clean main at 0226204, identical to origin/main after fetch; deterministic presentation-only fixtures and isolated temporary GTK/XDG profile.
- Ação única executada: extracted reusable presentation components and composed the existing dashboard using its existing sidebar.
- Resultado esperado na Engine: no Engine initialization for component tests.
- Resultado esperado na GUI: independently controlled status, activity, attention, conflict review and issue dialog; selection isolation and real data boundaries.
- API pública disponível / lacuna identificada: existing public latest-state subscription and convergence audit; no byte telemetry or public operation identity. Coalescing is not a transition log.
- Resultado observado (local, remoto, Inventory, GUI): five GTK wrapper components, own state/setters/callbacks, FakeSyncBackend, replaceable SyncBackend, DashboardController; no Google/Inventory access in components. Existing real revision-bound conflict flow retained for later event integration.
- Diagnóstico por fronteira: previous SynchronizationView mixed Engine lifecycle, rendering and conflict access. Operation-count progress must not be described as downloaded bytes. Idle alone cannot prove convergence.
- Correção mínima aplicada: reuse existing GTK widget/ownership conventions, status/progress primitives, dashboard selection and existing conflict implementation. EngineAdapter maps public API values only. Controller cancels old listeners on selection/unrealize and rejects other scopes. Activity incremental progress requires scope plus operation identity. Audit is rechecked against the latest subscription snapshot after awaiting it.
- Testes direcionados e regressões: isolated_components_and_fake_backend passed under GNOME SDK 50/rust-stable with Broadway and isolated XDG profile; invalid_progress_cannot_become_a_percentage passed; clippy workspace/all-targets passed before final icon/localization changes; gettext catalog validation passed. Tests exercise repeated construction, all status states, Unicode/long names, invalid progress, stale operation/account updates, deduplication/click callbacks, conflict confirmation/duplicate submission/stale/failure states, issue open/close and fake backend selection.
- Evidências (logs, hashes, IDs, capturas): _build/stage12-components-test.log; _build/stage12-clippy.log. These are SDK fixture tests, not installed application acceptance.
- Limitações remanescentes: screen-reader behavior, keyboard/focus, visual resizing and installed Flatpak/Google presentation still need direct verification. Future-event conflict/issue contracts are fixtures, not newly homologated real flows. Current API can omit short-lived operations. No byte telemetry is synthesized.
- Resultado: BLOQUEADO (automated component checks pass; interactive accessibility and installed presentation acceptance remain unverified).
