# Inventory — Baseline

## Objetivo

Armazenar e recuperar o último estado local e remoto comprovadamente sincronizado de cada item, representado por `SyncBaseline`.

## Funcionamento

O baseline é consultado pelo identificador lógico do item e atualizado quando há confirmação válida de um novo estado sincronizado. A gravação utiliza o contexto transacional do inventário.

## Interface

- `load`: recupera o baseline existente, quando disponível.
- `replace_confirmed`: registra um novo estado confirmado.

**Entradas principais:** `SyncItemId`, `SyncBaseline` e `BaselineSide`.

**Resultado:** `Option<SyncBaseline>` ou confirmação da atualização.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)
