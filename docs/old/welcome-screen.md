# Welcome, folder setup and dashboard

`src/welcome.rs` builds the centered GTK4/Libadwaita introduction and connection
action. `src/main.rs` owns one engine, one application window, a ToastOverlay and
an AdwNavigationView. `src/setup.rs` selects folders; `src/dashboard.rs` displays
saved settings and connection state.
The setup instruction cards remain removed.

## Behavior

- Connect invokes only the public engine facade on a worker thread. A spinner and
  inline status show progress; Connect is disabled and Cancel is available.
- GIO opens Google authorization in the default system browser via the desktop
  portal in Flatpak. The engine has no GTK dependency and never requests a password.
- Completion proceeds only after code exchange, UserInfo identity confirmation
  and successful secure credential persistence.
- Errors appear inline and as Libadwaita toasts; controls recover for a retry.
- Successful login opens folder selection; saving opens Dashboard. Configured
  accounts open Dashboard on startup, including connection failures shown explicitly.
- Back navigation allows connecting another account in the same engine. Existing
  account tokens remain isolated. ACCT-003 restores saved profiles; CONFIG-003 routes configured accounts to Dashboard.
- Learn More expands inline. Narrow windows and large text retain vertical scrolling.
- Repeated activation presents the same window. Close hides it; Quit/Ctrl+Q exits.
  Application shutdown cancels pending authorization. Background/autostart portal
  work remains separate.

## Manual checks

1. Check light/dark style, keyboard navigation, large text and widths 360–960 px.
2. As maintainer, embed the official Desktop OAuth client at build time and complete the
   [real OAuth checklist](google-oauth.md#gnome-builder--flatpak-test-procedure).
3. Confirm Connect cannot start duplicate flows; Cancel and errors allow retry.
4. Confirm setup identifies the connected account, lists real Drive folders and
   opens the system local-folder picker. Cancel must not change saved settings.
5. Save settings, reopen and confirm Dashboard appears. Overlapping local folders
   must be rejected; no file transfers should be claimed or started.
6. Close and reactivate: the same page returns. Quit terminates the process.

Compilation and automated engine tests do not substitute for visual checks or a
real Google consent flow. See [validation evidence](auth-validation.md).
