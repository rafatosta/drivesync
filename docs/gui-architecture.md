# GUI architecture

The GUI uses responsibility layers inside the application binary. `engine/`
remains an independent library with no GTK/libadwaita dependency. The interface
currently exposes account management and folder configuration only.

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
| `preferences/` | `features/preferences/` (`window.rs`, models and controls) | Preferences models, window and per-account controls |
| `dashboard/` | `features/dashboard/` | Dashboard page composition and account selection |
| `dashboard/mod.rs` | `features/dashboard/page.rs`, reexported by `mod.rs` | Dashboard page builders and fixture entry points |
| `dashboard/account_details.rs` | `features/dashboard/account_view.rs` | Account panel and private dashboard composition |
| Account actions, header, description and folders | `features/dashboard/components/` | Account widgets and folder controls |

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
background services read preferences. These edges are retained rather than hidden behind
new traits or duplicated models. Feature controllers never move business rules
out of the engine.

## Acceptance and verification

Use the GNOME 50 SDK and Rust extension selected by
`flatpak/com.rtosta.DriveSync.Devel.yml`. Validate formatting, compilation, Clippy,
workspace tests, private rustdoc and doctests there. Opt-in layout fixtures need a
display and an isolated `XDG_DATA_HOME` as described in
[`data/screenshots/README.md`](../data/screenshots/README.md).

Verify dashboard selection, connection, folder configuration, preferences and
navigation with the existing fixtures. Passing unit tests alone does not establish real Drive
integration or packaged application behavior. Execution evidence and outstanding
verification limits belong in commit messages and the task verification report.
