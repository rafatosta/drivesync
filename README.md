# DriveSync

**[MVP — roteiro de implementação do zero, por fases e checklists](docs/MVP.md)** · [Índice da documentação](docs/README.md)

> **Etapa atual:** planejamento. A janela inicial e a biblioteca Rust estão no repositório, mas a construção no GNOME Builder, o Flatpak e o fluxo de sincronização ainda não foram validados. Inicie pela fase F00 do MVP; não avance sem cumprir o gate anterior.


Cliente de sincronização do Google Drive para Linux/GNOME **em fase de especificação**.

## Estrutura

- `src/main.rs`: app GTK4/Libadwaita, apenas janela DriveSync.
- `engine/`: biblioteca Rust independente, interface pública em `engine/src/lib.rs`.
- `engine/src/<módulo>/`: implementação-base `mod.rs` e documentação `README.md`, `contract.md`, `algorithms.md`, `tests/test-cases.md` agrupadas.
- `docs/`: invariantes, modelos e decisões globais.

`cargo run` executa a janela (requer toolchain Rust e bibliotecas de desenvolvimento GTK4/Libadwaita); `cargo test -p drivesync-engine` valida a fachada inicial.

**Não existe sincronização funcional ainda**. Métodos da API não implementados retornam erro explícito. Os arquivos de especificação são propostas para revisão antes de implementar o motor. Não usar conta ou dados pessoais para testes destrutivos.

## Método de desenvolvimento (spec-first)

O projeto é organizado em problemas pequenos: **pesquisar técnica já conhecida → documentar fundamento/limites → definir contrato e testes → implementar Rust em escopo pequeno → revisar e integrar**. Cada módulo contém algoritmo e referências técnicas na própria pasta, sem outra árvore de documentação de módulos. A IA atua como implementadora/revisora de tarefas delimitadas, não como autoridade sobre políticas de sincronização.

- [Índice da documentação](docs/README.md)
- [Método de desenvolvimento](docs/development-methodology.md)
- [Catálogo de problemas e soluções conhecidas](docs/problem-catalog.md)
- [Template de algoritmo](docs/algorithm-spec-template.md)
- [Diretrizes para IA](docs/ai-development-guidelines.md)
- [Estudo de viabilidade](docs/feasibility-validation.md)
- [Referências oficiais](docs/reference-catalog.md)

**Critério de confiabilidade:** testes reais de recuperação, conflito e integridade; compilar e passar testes unitários não comprovam consistência de sincronização. Algoritmos seguem em DRAFT até revisão.
