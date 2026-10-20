# Contrato — download

## Objetivo
Baixar em staging e validar antes de instalar.

## Utilizado por
queue,coordinator.

## Entradas
Dados conceituais tipados, IDs estáveis, contexto de operação, versões e pré-condições quando aplicáveis.

## Saídas
Resultado tipado ou erro recuperável, definitivo, conflitante ou ambíguo.

## Limites
Não realizar atividades de outros módulos; não depender de toolkit gráfico ou linguagem específica.

## Invariantes
Arquivo incompleto não substitui arquivo válido. Respeitar `docs/invariants.md`.

## Qualidade
Especificar idempotência, efeitos colaterais, concorrência, cancelamento, erro e recuperação antes da implementação.
