# Orientações para agentes — Inventory metadata

## Objetivo

Persistir e recuperar atributos observados de `LocalFile` e `RemoteFile`.

## Implementação

- Implementar `LocalMetadataRepository.apply/load` e `RemoteMetadataRepository.apply/load`.
- Preservar informações conhecidas, estados de coleta, campos opcionais e metadados especializados aprovados.
- Garantir reconstrução fiel dos atributos persistidos e sanitização de dados sensíveis.

As assinaturas e regras de cada operação estão em [INTERFACES.md](INTERFACES.md). Os testes previstos estão em [TESTS.md](TESTS.md). A distribuição interna do código será definida na implementação.

## Validação

Validar os contratos com dados isolados e cenários positivos, negativos, de integridade e recuperação. Informar testes executados, resultados e limitações, respeitando o [AGENTS.md do inventário](../AGENTS.md) e as instruções gerais do repositório.

## Definições pendentes

Seleção dos campos persistidos, estados de coleta, DDL, limites, codec versionado e sanitização.

Registrar lacunas relevantes antes de definir comportamentos ainda não especificados.
