# Remote folder picker

Reusable, single-use UI component, opened from the setup remote-folder choice. No engine, provider, authentication,
persistence or synchronization operations are performed here.

```rust,ignore
let source = MockFolderSource::default();
let picker = RemoteFolderPicker::new(
    RemoteFolderPickerConfig {
        provider_name: "Google Drive".into(),
        provider_icon_svg: provider.icon_svg.into(),
        account_label: "conta@example.com".into(),
        root: source.root(),
    },
    source,
);
picker.connect_response(|response| {
    // The calling flow owns any subsequent action.
    match response {
        RemoteFolderPickerResult::Selected(location) => println!("{location:?}"),
        RemoteFolderPickerResult::Cancelled => {}
    }
});
picker.present(&parent);
```

The caller supplies SVG source in `provider_icon_svg`. The UI loads it as a
scalable bytes icon without theme registration or temporary files. No provider
name or icon is hardcoded in the picker. Folder artwork is embedded in the binary.
The modal `adw::Window` and `ToolbarView` match the application's libadwaita 1.4
baseline; `AdwDialog` would require raising that baseline.

The configured root is initially open and selected. A single click selects a
child; double click or Enter opens it. Clicking the last breadcrumb selects the
currently open folder again, including the root. The footer always identifies
the pending selection; navigation selects the newly opened folder. Refresh and
history navigation reset selection to that folder. Escape, the close button,
window-manager close and Cancel emit `Cancelled`. Confirmation emits exactly one
`Selected` response, including the root-to-selection path, then closes.

Sources return immediate child folders on GTK's local executor. IDs and page
tokens are opaque. Pages load progressively, deduplicate IDs and reject repeated
continuation tokens. Request generations prevent stale responses from changing
the current view, including after closing. Already-running futures may finish;
they cannot update the closed or superseded browser. A source must yield rather
than perform blocking work. Query failures preserve already loaded pages; Retry
reloads the folder from its first page.

The mock includes the specified tree and test fixtures under
`Projetos / DriveSync / Testes`: an empty folder, a failing folder and 80 folders
for pagination. `delay` and `page_size` can be adjusted without changing the UI.

## Verification

Run all Rust commands inside the SDK specified by the Flatpak manifest.
The regular tests cover history and the source; the opt-in GTK test additionally
covers stale responses, progressive pagination, errors, empty folders, returning
the root, grid activation, selected paths and cancellation. It uses its own
application and never activates DriveSync's account flow.

```sh
cargo test -p drivesync remote_folder_picker
cargo test -p drivesync native_picker_selection_cancel_and_preview -- --ignored --test-threads=1
```

For a rendered preview, set `DRIVESYNC_PICKER_PREVIEW` to an absolute PNG output
path. `DRIVESYNC_PICKER_DARK=1` selects the dark theme. Optionally compile the
translation catalog into a locale directory and supply that directory using
`DRIVESYNC_PICKER_LOCALE`, with `LANG=pt_BR.UTF-8 LANGUAGE=pt_BR`.

Setup retains the selected location only in memory, displays it in review and
requires both directory choices before continuing. The selected folder never
grants synchronization readiness or dashboard access.

## Real setup integration

Setup uses `EngineFolderSource` and the engine's account-scoped
`remote_folder_root` and `list_remote_folders` queries. The engine obtains
Accounts-managed sessions, renews expired credentials, and calls existing
Google Provider methods on its Tokio executor. Tokens never reach the picker.
The root is resolved to its canonical remote ID before presentation.

Consent requests `drive.metadata.readonly` for read-only metadata browsing.
An explicit scope/authentication failure returns setup to its connection page
and offers authorization for the same account. Folder permission errors remain
separate from missing consent. Configure the required scope in the Google OAuth
application as well; successful mock tests do not verify that external setup.

Closing or navigating cancels outstanding folder queries. Setup calls the
picker cancellation method before beginning account logout. Engine-side query
cancellation releases the connection lease used to exclude concurrent logout.
The reusable picker retains a request generation check as a second safeguard.
Mock sources are used only by explicit test fixtures and the documented example;
production setup does not fall back to simulated data on errors.

The selected path remains an in-memory choice. No synchronization, remote
modification or persistent synchronization configuration is implemented here.
