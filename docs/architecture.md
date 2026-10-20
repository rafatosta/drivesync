# Arquitetura DriveSync

A aplicação `src/main.rs` consome apenas a fachada pública em `engine/src/lib.rs` por uma dependência Cargo. O motor é uma biblioteca Rust independente em `engine/`. Cada módulo fica integralmente em `engine/src/<módulo>/` com `mod.rs`, README, contract, algorithms e testes conceituais.

A API pública encapsula módulos privados. Módulos são independentes por **responsabilidade**, mas cooperam por dados de domínio compartilhados e contratos explícitos. Não há estado global mutável livre. O Coordinator organiza pré-condições e o fluxo. Autenticação real é pré-requisito para operações remotas, mas testes unitários podem usar provedores simulados.

A sincronização ainda não foi implementada. `DriveSyncEngine::add_account` e `start_sync` retornam explicitamente `NotImplemented`; a janela inicial GTK4/Libadwaita continua exibindo somente DriveSync.

Documentos globais: [dependências](module-dependencies.md), [dados compartilhados](shared-context.md), [API pública](public-engine-api.md), [invariantes](invariants.md).

Extensão futura: daemon/CLI usam a mesma biblioteca; processos distintos comunicam-se por adaptador IPC, não por acesso direto ao estado interno.
