# Inventory — Operations

## Objetivo

Manter a fila persistente de operações de sincronização, representadas por `SyncOperation`, incluindo seus estados e tentativas de execução.

## Funcionamento

Cada operação é registrada com identidade, item associado, revisão e informações de deduplicação quando disponíveis. A fila permite consulta, reserva atômica de trabalhos elegíveis e atualização de estado, incluindo tentativas, bloqueios e conclusão confirmada.

## Interface

- `enqueue`: registra uma operação.
- `list`: consulta operações com filtros e paginação.
- `claim`: reserva uma operação elegível.
- `transition`: atualiza o estado e as tentativas.
- `complete_verified`: registra conclusão validada.

**Entradas principais:** `NewSyncOperation`, `OperationFilter`, `ClaimOperationRequest`, `UpdateOperationRequest` e `OperationId`.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)
