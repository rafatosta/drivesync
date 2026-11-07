# Modelo de dados — DriveSync

A especificação completa do modelo relacional da engine está centralizada no módulo Inventário:

**[docs/engine/inventory/DATA_MODEL.md](engine/inventory/DATA_MODEL.md)**

Esse documento é a **única referência normativa** para os bancos e suas tabelas, evitando duas versões concorrentes do esquema.

Ele define:

- **Banco global da aplicação:** contas, conexões de sincronização e preferências (`app.sqlite3`);
- **Banco por pasta sincronizada:** metadados locais, metadados Google, vínculo `SyncItem`, baseline, fila, cursores e eventos (`inventory.sqlite3`);
- **Histórico por sincronização:** atividades e resultados (`history.sqlite3`);
- **Armazenamento externo de credenciais:** Secret Service/keyring, sem tokens em SQLite;
- **Regras de integridade:** chaves, transações, ausência confirmada versus estado desconhecido, isolamento por conexão e recuperação.

O Inventário é inicialmente a interface para o acesso estruturado a esses bancos. Os demais módulos consomem seus contratos, sem executar SQL diretamente.

**Decisão de modelo:** `LocalFile` e `GoogleFile` têm tabelas separadas, relacionadas por `sync_items`. O objeto Rust `SyncItem` agrega os dois lados e o último estado confirmado. A versão anterior deste documento, que concentrava os metadados em `sync_items`, foi substituída pela especificação modular do Inventário.

Nenhuma implementação ou migração de dados é definida neste arquivo.
