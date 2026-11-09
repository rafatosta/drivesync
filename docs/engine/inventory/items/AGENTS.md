# Orientações para agentes — Inventory items

## Objetivo

Persistir identidade, vínculos, presença, estado e revisão dos itens lógicos `SyncItem`.

## Implementação

- Implementar `resolve`, `list_ids`, `load_header`, `create`, `apply_links`, `set_presence` e `set_status`.
- Preservar identidade estável, integridade dos vínculos e controle de revisão otimista.
- Registrar presença conforme evidências, distinguindo desconhecimento de ausência confirmada.

As assinaturas e regras de cada operação estão em [INTERFACES.md](INTERFACES.md). Os testes previstos estão em [TESTS.md](TESTS.md). A distribuição interna do código será definida na implementação.

## Validação

Validar os contratos com dados isolados e cenários positivos, negativos, de integridade e recuperação. Informar testes executados, resultados e limitações, respeitando o [AGENTS.md do inventário](../AGENTS.md) e as instruções gerais do repositório.

## Definições pendentes

Correlação local sem `local_id`, tombstones, atualização de `ItemKind`, revisão em operações compostas e paginação.

Registrar lacunas relevantes antes de definir comportamentos ainda não especificados.
