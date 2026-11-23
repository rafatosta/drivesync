# Functional correction — Stage 9

Stage 9 integrates a Linux projection policy into Context, Analyzer, Operations and
Inventory convergence. It does not provide document export, offline Workspace editing,
or remote shortcut traversal.

## Supported representations

| Remote/local item | Policy |
| --- | --- |
| Google Docs, Sheets, Slides, Drawings, Forms | Read-only derived `<name>.desktop` link |
| Google folder | Native local directory; normal identity and parent association |
| Binary file | Native regular file; known size and valid SHA-256 required |
| Drive `application/vnd.google-apps.shortcut` | Explicit per-item refusal; never followed |
| Other Google native MIME types, including `drive-sdk` | Explicit per-item refusal |
| Symlink, FIFO, socket, device or other special local entry | Explicit per-item refusal; never followed/read as regular content |
| Local non-UTF-8 name | Preserved and explicitly refused |

The Workspace MIME allowlist is deliberate. Google's [MIME type reference](https://developers.google.com/workspace/drive/api/guides/mime-types)
and [remote shortcut guide](https://developers.google.com/workspace/drive/api/guides/shortcuts)
distinguish documents, folders and shortcut targets. No implicit content conversion is used.

## Names and collisions

Linux names retain exact UTF-8 bytes, case and normalization. No sanitization or automatic
renaming establishes identity. Empty names, dot entries, `/`, control characters, reserved
`.drivesync` and `.drivesync-transfer-*` names, and names exceeding 255 bytes are refused.
The Workspace suffix counts toward that limit. Duplicate raw names within one parent and
collisions in the projected namespace refuse the involved items. An existing local projection
slot is never adopted or overwritten, even if its bytes happen to match.

Invalid names and insufficient binary metadata are item-level issues. Structural capture
errors, missing scope evidence and incomplete enumeration still invalidate absence claims;
unsupported entries do not by themselves make a complete scan partial. Independent supported
items continue, while account status must not claim full synchronization.

## Identity, effects and recovery

Initial projection requires a verified empty destination, a registered ItemId/RemoteId and
a confirmed parent association. The executor rechecks the raw and projected provider
namespaces. A link contains only fixed desktop-link keys and a percent-encoded Drive ID in a
fixed HTTPS URL. It has no `Exec` entry and is initially non-executable.

Convergence verifies canonical local link bytes, actual local identity, projected filename,
current remote identity/version and the parent association. It stores
`GoogleWorkspaceShortcut`, rather than comparing a link digest with a document digest.
Provider revisions can refresh this baseline without downloading or uploading document
content. Remote rename/move updates only the local link location. Local edits, replacement,
removal or location changes are preserved and refused; they never authorize remote content
writes. Existing native baselines are not silently converted to projections.

Link publication uses the existing stage identity and execution checkpoint. Lost receipts
are reconciled from publication evidence; no blind replay occurs. A newer document revision
can be accepted during recovery only when identity, name, parent and supported representation
still agree. Divergent local bytes remain inconclusive or unsupported.

## Validation

Automated coverage includes MIME classification, reserved/invalid/oversized names, inert URL
encoding, remote shortcut refusal, projection collisions, local special/non-UTF-8 entries,
initial link planning, baseline refresh, remote rename, local edit refusal, and lost-receipt
recovery after a document revision without replay. Existing binary type-conflict coverage
remains applicable. Real-account results are recorded below.

### Real account, 2026-11-23

The opt-in `connection::live_projection_tests::real_workspace_projection_mixed_scope_and_restart`
test ran in the existing Devel Flatpak build sandbox with the actual Engine, its account
store, Secret Service and Google API. It verified exactly one connected account matching
`rtosta.coc@gmail.com` before touching the selected isolated scope.

[Remote fixture](https://drive.google.com/drive/folders/1zTV_mKlbUtPHQjCsekU2gJoS-rMtzO2i):
`DriveSync-stage9-validation-2026-11-23`; matching local fixture under `/home/tosta/GoogleDrive/`.
Both fixtures are retained. No unrelated files were changed or deleted.

- A binary upload, directory creation, Docs link and nested Sheets link converged with
  confirmed native/projection baselines and no pending uncertain effects.
- Remote invalid separator names, duplicate names and a document/binary projected-name
  collision were isolated. A real local symlink, FIFO and non-UTF-8 filename remained intact.
- Restart preserved the exact Inventory records, revisions, identities and executions.
- Remote document rename produced `Renamed.desktop`, retaining ItemId, RemoteId and inode.
- Editing the local link preserved the edited bytes and the entire prior Inventory snapshot;
  Google still reported the original Workspace MIME. No content upload or replay occurred.
- Account status was `Waiting` with `Unsupported`, rather than claiming whole-scope completion.

The first mixed run exposed a raw-name/projected-name destination lookup error; after a
regression assertion and correction, resume, restart, rename and local-edit phases passed.
Remote shortcut refusal, unavailable hash and revision-advance crash recovery are covered
by automated fixtures, not claimed as real remote shortcut or document-content edit tests.
This validates the Engine in the Devel sandbox; it does not constitute a new GUI or installed
package acceptance run.

### Reproducibility

Final workspace tests: 685 passed, zero failed, 16 opt-in tests ignored in the default
run. The four real resume/restart/rename/edit phases passed separately. Formatting, Clippy,
private documentation and doctests passed.

All Rust checks use `org.gnome.Sdk//50`, the manifest's `rust-stable` extension and offline
Cargo dependencies. Required commands: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
`cargo doc --workspace --no-deps --document-private-items`, and `cargo test --workspace --doc`.
Real tests require explicit `DRIVESYNC_STAGE9_PHASE`, `DRIVESYNC_STAGE9_LOCAL_ROOT`,
`DRIVESYNC_STAGE9_REMOTE_ROOT`, `DRIVESYNC_REAL_ACCOUNTS_DIRECTORY` and
`DRIVESYNC_REAL_OAUTH_FILE`. Only phase `sync` creates a new isolated fixture; `resume`,
`verify`, `rename` and `edit` operate on the already selected fixture.
