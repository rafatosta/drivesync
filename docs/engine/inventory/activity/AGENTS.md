# Orientações para agentes — Inventory activity

## Objetivo

Registrar e consultar atividades persistentes por item do inventário.

## Implementação

- Registrar `SyncActivity` validada e associada a `SyncItemId`, com `OperationId` quando aplicável.
- Implementar `append` e `list` com filtros, ordenação e paginação.
- Preservar o histórico e utilizar transações atômicas quando a atividade integrar uma confirmação de sincronização.

As assinaturas e regras de cada operação estão em [INTERFACES.md](INTERFACES.md). Os testes previstos estão em [TESTS.md](TESTS.md). A distribuição interna do código será definida na implementação.

## Validação

Validar os contratos com dados isolados e cenários positivos, negativos, de integridade e recuperação. Informar testes executados, resultados e limitações, respeitando o [AGENTS.md do inventário](../AGENTS.md) e as instruções gerais do repositório.

## Definições pendentes

Idempotência, retenção, sanitização da descrição e desempate da ordenação temporal.

Registrar lacunas relevantes antes de definir comportamentos ainda não especificados.
