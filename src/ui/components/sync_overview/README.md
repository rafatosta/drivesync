# SyncOverview

Standalone GTK4/libadwaita presentation. The dashboard controller feeds it through
the existing account-scoped EngineAdapter subscription. The component itself has
no Engine imports. The simulated dashboard selector has been removed; the isolated
example remains available.

The adapter maps checks, active synchronization, audited completion and actionable
issues. Idle alone never means completion. States outside the four-state contract
(stopped, incomplete verification, waiting or finalizing) keep the existing detailed
status visible and hide the overview. Its action opens the existing account-scoped
diagnostic page. Selection changes reset the view and cancel obsolete subscriptions.

The Engine currently exposes operation steps, not account-wide item or byte
progress. These metrics (including completed_items) are unavailable, not zero;
active synchronization therefore pulses without a percentage or invented counts.
A complete persisted conflict list supplies a pending-item count only when there
is no additional current issue. The generic status updated_at is not a last-sync
timestamp; last_sync remains unavailable. Completion still requires the Engine's
existing convergence audit, scoped to its captured observation horizon.

`SyncOverview::new()` starts in Checking with an unknown count. Place `widget()`
in a GTK container, call `set_state(SyncOverviewState)` on the GTK thread, and
connect `connect_attention_requested` to receive the native button activation.
Clones share widgets. The caller owns all state decisions, item counts and action
handling. No engine, accounts, providers, inventory or persistence are accessed.

All values are independently supplied: no remaining count, progress or completion
is inferred. `None` hides unavailable details; zero remains visible. Progress is
clamped to 0–1; NaN and infinity use indeterminate progress. The frame callback
only runs while mapped and is removed when indeterminate progress ends. Updates
reuse widgets; detail visibility uses a short native crossfade. Text wraps at
narrow widths. Semantic styles and symbolic Adwaita icons support both themes.

Messages use gettext with Brazilian Portuguese translations in `po/pt_BR.po`.
Dates use local absolute date and time, avoiding a stale relative timestamp.
Storage sizes use GLib's localized decimal units; item grouping uses a translated
separator. The caller must initialize gettext before creating the widget.

## Interactive preview

From the repository root, run inside the SDK specified by the development manifest:

```sh
flatpak run --command=sh --socket=x11 --share=ipc \
  --filesystem="$PWD" --filesystem=/home/tosta/.cargo \
  --env=CARGO_HOME=/home/tosta/.cargo org.gnome.Sdk//50 -c '
  export PATH=/usr/lib/sdk/rust-stable/bin:$PATH
  cd /home/tosta/Projetos/drivesync
  mkdir -p _build/sync-overview-locale/pt_BR/LC_MESSAGES
  msgfmt --check -o _build/sync-overview-locale/pt_BR/LC_MESSAGES/drivesync.mo po/pt_BR.po
  LANG=pt_BR.UTF-8 DRIVESYNC_PREVIEW_LOCALEDIR=$PWD/_build/sync-overview-locale \
    cargo run --offline --example sync_overview
'
```

Choose a state, edit counts, uncheck optional values, and switch the theme. All
counters can be marked unavailable. The attention button displays a signal receipt
in the preview only. Indeterminate progress animates normally. No account is needed.

Within the same SDK environment, run the isolated display test with:

```sh
GDK_BACKEND=x11 GSK_RENDERER=cairo cargo test --offline --example sync_overview -- --ignored --test-threads=1
```

The test verifies stable widget identity, transitions, optional and zero values,
progress normalization, pulse shutdown and action delivery. It also writes eight
state/theme PNGs under `_build/sync-overview-{light,dark}-{0,1,2,3}.png` for review.
Run without personal accounts; this test opens only its own temporary GTK window.

Validation performed: workspace formatting, Clippy with warnings denied, workspace
tests, documentation, doctests, translation compilation and the isolated GTK test
in GNOME SDK 50. The scoped-subscription GTK test covers selection isolation and
the attention action; the existing component/backend GTK suite also passes. An
explicit opt-in test opened the isolated rtosta.coc@gmail.com Engine profile and
verified its stopped-state snapshot through EngineAdapter without starting a
worker. Other opt-in workspace tests remain ignored. Active Google transfers and
the installed Flatpak were not exercised for this integration.
