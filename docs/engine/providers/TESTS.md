# Providers — TESTS

## Objetivo

Verificar que cada submódulo disponibiliza somente as operações reais da API externa que representa, sem acrescentar comportamento funcional da aplicação.

## Verificações documentais

| ID | Cenário | Critério |
| --- | --- | --- |
| PRV-01 | Submódulo de serviço | Documentação corresponde à API oficial do serviço |
| PRV-02 | Interfaces públicas | Não há contrato universal artificial entre provedores |
| PRV-03 | Fronteira | Não existem decisões de sincronização, gerenciamento de transferências ou regras de negócio |
| PRV-04 | Credenciais | Nenhuma persistência ou gestão de contas pertence ao provider |
| PRV-05 | Mapeamento | Operações não são duplicadas entre `google/` e uma camada `google/api/` |

| PRV-06 | I/O externo | Métodos que aguardam HTTP ou streams não bloqueiam a thread GTK; a forma assíncrona é decidida por API específica |
| PRV-07 | Estado | Sem status global nem progresso de `SyncItem`; somente dados técnicos efetivamente fornecidos por respostas/erros |
| PRV-08 | Efeito indeterminado | Timeout/cancelamento após envio não transforma operação incerta em reversão confirmada |
| PRV-09 | Backpressure de streams | Streams grandes usam processamento incremental, sem obrigatoriedade de carregar tudo em memória |

Os testes técnicos de requisições, respostas, tipos e erros ficam na documentação específica: [Google — TESTS](google/TESTS.md).

**Estado:** plano de verificação documental, não testes executados.
