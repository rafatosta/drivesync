# Inventory — Storage

## Objetivo

Disponibilizar a persistência SQLite do inventário, assegurando acesso consistente, integridade e transações atômicas.

## Funcionamento

Abre e valida o banco `inventory.sqlite3`, gerencia seu esquema, índices e integridade referencial, e fornece contextos de leitura e transação. As operações transacionais são confirmadas ou revertidas atomicamente e as falhas são representadas por erros tipados.

## Interface

- `open`: abre e valida o inventário.
- `read`: executa consultas no contexto de leitura.
- `transaction`: executa operações em transação atômica.

**Entradas principais:** `OpenInventoryRequest`, `SyncIdentity` e `InventoryTx`.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)
