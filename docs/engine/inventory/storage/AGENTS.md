# Orientações para agentes — Inventory storage

## Objetivo

Fornecer persistência SQLite consistente para os dados do inventário.

## Implementação

- Implementar `open`, `read` e `transaction`.
- Validar identidade, esquema e integridade do banco de dados.
- Aplicar transações atômicas, rollback e tradução de erros de persistência.
- Preservar os registros existentes diante de erros ou incompatibilidades.

As assinaturas e regras de cada operação estão em [INTERFACES.md](INTERFACES.md). Os testes previstos estão em [TESTS.md](TESTS.md). A distribuição interna do código será definida na implementação.

## Validação

Validar os contratos com dados isolados e cenários positivos, negativos, de integridade e recuperação. Informar testes executados, resultados e limitações, respeitando o [AGENTS.md do inventário](../AGENTS.md) e as instruções gerais do repositório.

## Definições pendentes

DDL, WAL e locking, timeouts, provisionamento com raiz inacessível, índices, conexões e testes de crash.

Registrar lacunas relevantes antes de definir comportamentos ainda não especificados.
