# Módulo changes

## Objetivo
Identificar mudanças locais e remotas incrementalmente.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
planner, coordinator.

## Saídas e compromissos
Diferenças e eventos duráveis para avaliação.

## Limites: o que NÃO faz
Não transferir ou excluir conteúdo.

## Algoritmos previstos
CHANGES-001 reconciliação local; CHANGES-002 cursor remoto.

## Erros e cenários a verificar
Cursor inválido; paginação; eventos perdidos; descontinuidade.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** rascunho v0.1; não implementado.
