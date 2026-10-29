# Módulo remote

## Objetivo
Traduzir solicitações conceituais em chamadas Google Drive API v3.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
changes, planner, upload, download.

## Saídas e compromissos
Metadados tipados ou erros remotos classificados.

## Limites: o que NÃO faz
Não decidir conflitos ou exclusões locais.

## Algoritmos previstos
REMOTE-001 listagem paginada; REMOTE-002 operações; REMOTE-003 backoff.

## Erros e cenários a verificar
403; 404; 429; limites; respostas parciais.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** inventário paginado, feed e leitura/revalidação de metadados e mídia
implementados; ver [inventário](inventory.md) e [feed](../changes/incremental.md).
Contratos genéricos de operações futuras não representam suporte a atualização
remota ou exclusão.
