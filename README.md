# DriveSync

Cliente de sincronização do Google Drive para Linux/GNOME **em fase de especificação**.

## Estrutura

- `src/main.rs`: app GTK4/Libadwaita, apenas janela DriveSync.
- `engine/`: biblioteca Rust independente, interface pública em `engine/src/lib.rs`.
- `engine/src/<módulo>/`: implementação-base `mod.rs` e documentação `README.md`, `contract.md`, `algorithms.md`, `tests/test-cases.md` agrupadas.
- `docs/`: invariantes, modelos e decisões globais.

`cargo run` executa a janela (requer toolchain Rust e bibliotecas de desenvolvimento GTK4/Libadwaita); `cargo test -p drivesync-engine` valida a fachada inicial.

**Não existe sincronização funcional ainda**. Métodos da API não implementados retornam erro explícito. Os arquivos de especificação são propostas para revisão antes de implementar o motor. Não usar conta ou dados pessoais para testes destrutivos.
