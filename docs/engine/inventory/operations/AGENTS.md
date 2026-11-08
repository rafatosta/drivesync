# Orientações para agentes — Inventory operations

## Objetivo

Manter a fila persistente de operações de sincronização e seus estados de processamento.

## Implementação

- Implementar `enqueue`, `list`, `claim`, `transition` e `complete_verified`.
- Registrar revisão, identidade, tentativas, deduplicação e estados válidos.
- Executar reservas atômicas e registrar conclusão confirmada com consistência transacional.

As assinaturas e regras de cada operação estão em [INTERFACES.md](INTERFACES.md). Os testes previstos estão em [TESTS.md](TESTS.md). A distribuição interna do código será definida na implementação.

## Validação

Validar os contratos com dados isolados e cenários positivos, negativos, de integridade e recuperação. Informar testes executados, resultados e limitações, respeitando o [AGENTS.md do inventário](../AGENTS.md) e as instruções gerais do repositório.

## Definições pendentes

Elegibilidade após falhas, estado de resultado desconhecido, requeue e chave de idempotência.

Registrar lacunas relevantes antes de definir comportamentos ainda não especificados.
