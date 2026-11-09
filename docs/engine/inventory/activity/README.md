# Inventory — Activity

## Objetivo

Registrar e consultar o histórico persistente de atividades associadas aos itens do inventário, representadas por `SyncActivity`.

## Funcionamento

As atividades são associadas ao identificador lógico do item e, quando aplicável, ao identificador de operação. Os registros são preservados em ordem cronológica e podem ser consultados com filtros e paginação.

## Interface

- `append`: registra atividade validada.
- `list`: consulta atividades por item, operação ou período.

**Entradas principais:** `NewActivity`, `ActivityFilter`, `Pagination`, `SyncItemId` e `OperationId`.

**Resultados:** `SyncActivity` e páginas de atividades.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)
