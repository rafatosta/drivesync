# Módulo local

## Objetivo
Observar, classificar e verificar arquivos locais.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
changes, planner, download, upload.

## Saídas e compromissos
Eventos e snapshots locais.

## Limites: o que NÃO faz
Não chamar Google Drive diretamente.

## Algoritmos previstos
LOCAL-001 observar diretório; LOCAL-002 estabilização de gravação.

## Erros e cenários a verificar
Eventos duplicados; overflow; links simbólicos; caminhos inválidos.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** inventário completo e estabilização implementados em `inventory.rs`.
Contrato vigente e oráculos: [inventário e estabilização](reconciliation.md).
Monitor contínuo permanece proposta; cada ciclo varre a árvore para recuperar
mudanças ocorridas sem eventos ou com o aplicativo fechado.
