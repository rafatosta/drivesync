# Arquitetura do DriveSync

## Princípio
Independência de responsabilidade e implementação, **não independência de dados e funcionamento**. Módulos colaboram por contratos tipados, respeitando pré-condições funcionais explícitas.

## Organização
- `src/`: aplicativo GNOME inicial; consome somente a fachada pública.
- `modules/`: documentação dos módulos (estado atual do repositório). **Organização alvo:** mover documentação e implementação juntas para `engine/src/<modulo>/`, tornando `engine/` uma biblioteca Rust independente.
- `docs/`: arquitetura, invariantes, modelo de dados e dependências globais.

Esta atualização é documental: não movimenta código nem altera a compilação atual.

## Arquitetura lógica
GUI/CLI → DriveSyncEngine (fachada) → Coordinator → contratos de Auth, Changes, Planner, Queue, Transfers, Conflicts e State → Google Drive API / filesystem / SQLite.

## Comunicação
- Dentro do motor: métodos e contratos de módulo, preferencialmente modelos imutáveis e interfaces tipadas.
- Entre processos, quando houver daemon: adaptador D-Bus/IPC sobre a mesma semântica da API pública.
- Nenhuma variável global mutável e acessível a todos.
- Só os responsáveis podem alterar suas categorias de estado.
- Dependências funcionais são verificadas pelo Coordinator. Uma sincronização remota exige autorização, mas testes de algoritmos podem injetar provedores simulados.

## Documentos complementares
- [Dependências entre módulos](module-dependencies.md)
- [Dados compartilhados](shared-context.md)
- [API pública prevista](public-engine-api.md)
- [Invariantes de segurança](invariants.md)
