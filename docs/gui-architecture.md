# GUI architecture

The GUI uses responsibility layers inside the application binary. `engine/`
remains an independent library with no GTK/libadwaita dependency. Public engine
contracts and application actions, signals, widget names and callbacks are unchanged.

## Inventory and relocation map

Paths are relative to `src/`. Directory entries cover all their existing children,
except for the more specific entries below. This map describes ownership, not new
features or a redesign.

| Original module | Destination | Responsibility and dependencies |
| --- | --- | --- |
| `main.rs` lifecycle | `app/lifecycle.rs` | Application startup, window, shutdown; composes infrastructure and features |
| `flow/navigation.rs`, `flow/tests.rs` | `app/navigation/` | Account routing; engine facade and dashboard |
| `flow/layout_tests.rs`, `flow/screenshot_demo.rs` | `app/` | Isolated cross-feature GUI fixtures |
| `main_menu.rs` | `app/main_menu.rs` | Application actions and menu; build identity |
| `restoration/` | `app/restoration/` | Startup restoration page/controller; routes to welcome or dashboard |
| `background/` | `core/background/` | Desktop portal, background lifetime and startup; preferences |
| `build_info.rs`, `i18n.rs` | `core/` | Build metadata and localization |
| `flow/worker.rs` | `core/worker.rs` | Off-main-thread execution with main-context completion |
| `flow/widgets.rs` | `ui/widgets.rs` | Generic content, row and scroll builders; GTK only |
| `welcome/` | `features/welcome/` (`page.rs`) | Welcome page; connection and application menu |
| `connection/` | `features/accounts/connection/` (`dialog.rs`, `controller.rs`) | Account authorization dialog and auth controller; engine facade and navigation |
| `session.rs` | `features/accounts/session.rs` | Confirmed sign-out action; engine facade and routing |
| `folder_picker/` | `features/accounts/folder_picker/` (`dialog.rs`, `controller.rs`) | Folder selection dialog/controller; engine facade and worker |
| `automatic/` | `features/synchronization/automatic/` | Scheduling policy and runtime controller; engine facade, preferences, background services |
| `sync_progress_card.rs` | `features/synchronization/components/progress_card.rs` | Typed synchronization presentation; depends on synchronization snapshots and dashboard presentation, so not a generic widget |
| `preferences/` | `features/preferences/` (`window.rs`, models and controls) | Preferences models, window and per-account controls; engine facade and synchronization |
| `dashboard/` | `features/dashboard/` | Dashboard page composition and account selection; accounts, preferences, synchronization |
| `dashboard/problems/` | `features/conflicts/` | Account problem center, comparison and merge dialogs; engine facade and synchronization controller |
| `dashboard/mod.rs` | `features/dashboard/page.rs`, reexported by `mod.rs` | Dashboard page builders and fixture entry points |
| `dashboard/account_details.rs`, `dashboard/synchronization.rs` | `features/dashboard/account_view.rs`, `features/dashboard/synchronization_view.rs` | Account panel and job presentation; private dashboard composition |
| `dashboard/account_actions.rs`, `account_header.rs`, `attention_summary.rs`, `description.rs`, `folders.rs`, `recent_activity.rs` | `features/dashboard/components/` (same filenames) | Account widgets, folder controls and presentation bindings |
| `dashboard/bulk_review.rs`, `state_recovery.rs` | `features/dashboard/dialogs/` (same filenames) | Explicit review and state recovery confirmations |
| `dashboard/problems/comparison.rs`, `text_merge.rs` | `features/conflicts/comparison_page.rs`, `text_merge_page.rs` | Version comparison and text merge navigation pages |

Dashboard components and views retain private APIs bounded to the dashboard;
feature modules expose the existing entry functions through their `mod.rs`.
Preferences already separate persisted models, account controls and the window;
they do not need empty pages/controllers directories. Dialogs remain dialogs even
when built by functions rather than wrapper structs. No standalone popover module
is introduced: the application menu is built by `app/main_menu.rs`.

## Dependency boundaries

`main` launches `app`; `app` composes `features`, `core` and `ui`. Features use the
engine facade and shared infrastructure. `ui` holds only domain-independent GTK
helpers. Current coordination edges are explicit: account flows call app routing,
background services read preferences, and the progress card uses dashboard
presentation. These pre-existing edges are retained rather than hidden behind
new traits or duplicated models. Feature controllers never move business rules
out of the engine.

## Acceptance and verification

Use the GNOME 50 SDK and Rust extension selected by
`flatpak/com.rtosta.DriveSync.Devel.yml`. Validate formatting, compilation, Clippy,
workspace tests, private rustdoc and doctests there. Opt-in layout fixtures need a
display and an isolated `XDG_DATA_HOME` as described in
[`data/screenshots/README.md`](../data/screenshots/README.md).

Review relocation diffs for unchanged function bodies, action names, widget names,
labels, callbacks and engine calls. Verify dashboard selection, connection,
folder configuration, preferences, conflict dialogs, progress and navigation with
the existing fixtures. Passing unit tests alone does not establish real Drive
integration or packaged application behavior. Execution evidence and outstanding
verification limits belong in commit messages and the task verification report.
