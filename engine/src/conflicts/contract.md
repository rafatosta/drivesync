# Contrato — conflicts

## Objetivo
Preservar versões e resolver divergências.

## Utilizado por
planner,ui.

## Entradas
Dados conceituais tipados, IDs estáveis, contexto de operação, versões e pré-condições quando aplicáveis.

## Saídas
Resultado tipado ou erro recuperável, definitivo, conflitante ou ambíguo.

## Limites
Não realizar atividades de outros módulos; não depender de toolkit gráfico ou linguagem específica.

## Invariantes
Não sobrescrever dados não preservados. Respeitar `docs/invariants.md`.

## Qualidade
Especificar idempotência, efeitos colaterais, concorrência, cancelamento, erro e recuperação antes da implementação.
