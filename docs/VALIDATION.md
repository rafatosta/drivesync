# Validation and known limitations

## Safety and regression requirements

- Preserve account and scope isolation, durable identities, verified baselines and revision-bound conflict choices.
- Validate initial and incremental transfers, content updates, renames, moves, directory dependencies, conflicts and restart recovery in both directions.
- Never infer deletion from absence or access loss. Permanent deletion is prohibited. Trash propagation is opt-in per session and requires recoverable evidence; refuse unsupported transport.
- Workspace Docs, Sheets, Slides, Drawings and Forms use read-only HTML links (0444); DOCX and PDF remain binary. Do not silently migrate old desktop projections or overwrite occupied destinations. Remote shortcuts and unsupported native types are refused.
- Recovery must not blindly replay uncertain effects. Preserve both copies and durable evidence.
- GUI tests must cover account switching, stale callbacks, independent observation and verification, unknown progress, conflict choices and stopped workers. Idle is not a convergence audit.

## Known acceptance limits at 07471f0

- Default Flatpak trash transport can refuse cross-mount moves. Same-mount sibling trash was exercised; the reserved in-root alternative was not accepted with a real account.
- Portal FUSE write roots, simultaneous real-account synchronization and screen-reader validation remain acceptance gaps.
- Historical installed tests used isolated scopes and do not establish unrestricted release readiness. Latest telemetry commit did not revalidate installed Flatpak or live Drive.
- Distinguish controlled tests, real-provider tests, installed application tests and manual visual acceptance.

## Commands

Run in the manifest GNOME SDK and Rust extension, never on the host:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps --document-private-items
cargo test --workspace --doc
```

Use isolated fixtures and explicit consent for real-account tests. Record actual outcomes and ignored tests; do not infer acceptance from test counts.

## Historical evidence

- [FUNCTIONAL_CORRECTION_STAGE_0.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_0.md)
- [FUNCTIONAL_CORRECTION_STAGE_1.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_1.md)
- [FUNCTIONAL_CORRECTION_STAGE_2.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_2.md)
- [FUNCTIONAL_CORRECTION_STAGE_3.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_3.md)
- [FUNCTIONAL_CORRECTION_STAGE_4.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_4.md)
- [FUNCTIONAL_CORRECTION_STAGE_5.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_5.md)
- [FUNCTIONAL_CORRECTION_STAGE_6.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_6.md)
- [FUNCTIONAL_CORRECTION_STAGE_7.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_7.md)
- [FUNCTIONAL_CORRECTION_STAGE_8.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_8.md)
- [FUNCTIONAL_CORRECTION_STAGE_9.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_9.md)
- [FUNCTIONAL_CORRECTION_STAGE_10.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_10.md)
- [FUNCTIONAL_CORRECTION_STAGE_11.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_11.md)
- [FUNCTIONAL_CORRECTION_STAGE_12.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/FUNCTIONAL_CORRECTION_STAGE_12.md)
- [DRIVESYNC_ETAPA_12_ATIVIDADE_10.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/DRIVESYNC_ETAPA_12_ATIVIDADE_10.md)
- [DRIVESYNC_ETAPA_12_ATIVIDADE_11.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/DRIVESYNC_ETAPA_12_ATIVIDADE_11.md)
- [DRIVESYNC_ETAPA_12_ATIVIDADE_12.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/DRIVESYNC_ETAPA_12_ATIVIDADE_12.md)
- [DRIVESYNC_ETAPA_12_ATIVIDADE_13.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/DRIVESYNC_ETAPA_12_ATIVIDADE_13.md)
- [DRIVESYNC_ETAPA_12_ATIVIDADE_14.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/DRIVESYNC_ETAPA_12_ATIVIDADE_14.md)
- [DRIVESYNC_ETAPA_12_GUI_VALIDACAO_INCREMENTAL.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/DRIVESYNC_ETAPA_12_GUI_VALIDACAO_INCREMENTAL.md)
- [engine/REAL_ACCOUNT_VALIDATION.md](https://github.com/rafatosta/drivesync/blob/07471f09e771233a8009dc09cc017f7489d249a3/docs/engine/REAL_ACCOUNT_VALIDATION.md)
