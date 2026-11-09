# Orientações para agentes — Inventory baseline

## Objetivo

Persistir e recuperar o último estado local/remoto comprovadamente sincronizado de cada item.

## Implementação

- Implementar `load` e `replace_confirmed` conforme as assinaturas documentadas.
- Preservar ausência legítima de baseline e campos opcionais.
- Registrar substituições confirmadas dentro de uma transação atômica.

As assinaturas e regras de cada operação estão em [INTERFACES.md](INTERFACES.md). Os testes previstos estão em [TESTS.md](TESTS.md). A distribuição interna do código será definida na implementação.

## Validação

Validar os contratos com dados isolados e cenários positivos, negativos, de integridade e recuperação. Informar testes executados, resultados e limitações, respeitando o [AGENTS.md do inventário](../AGENTS.md) e as instruções gerais do repositório.

## Definições pendentes

Tipagem de `BaselineSide.parent_identity`, critérios de confirmação, tombstones e campos opcionais.

Registrar lacunas relevantes antes de definir comportamentos ainda não especificados.
