# Módulo upload

## Objetivo
Enviar arquivos locais respeitando versão-base e integridade.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
queue, coordinator.

## Saídas e compromissos
Revisão remota confirmada ou conflito.

## Limites: o que NÃO faz
Não escolher sobrescrever arquivo remoto divergente.

## Algoritmos previstos
UPLOAD-001 sessão retomável; UPLOAD-002 reconciliação.

## Erros e cenários a verificar
Arquivo mudou durante envio; resposta perdida; quota excedida.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** rascunho v0.1; não implementado.
