# Módulo download

## Objetivo
Receber conteúdo remoto e instalar resultado local com segurança.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
queue, coordinator.

## Saídas e compromissos
Arquivo verificado ou erro recuperável.

## Limites: o que NÃO faz
Não planejar qual arquivo sincronizar.

## Algoritmos previstos
DOWNLOAD-001 transferência temporária; DOWNLOAD-002 validação final.

## Erros e cenários a verificar
Rede interrompida; disco cheio; remoto alterado; hash divergente.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** rascunho v0.1; não implementado.
