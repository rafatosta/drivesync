# Contrato — upload

## Objetivo
Enviar arquivos com sessão retomável e confirmar revisão.

## Utilizado por
queue,coordinator.

## Entradas
Dados conceituais tipados, IDs estáveis, contexto de operação, versões e pré-condições quando aplicáveis.

## Saídas
Resultado tipado ou erro recuperável, definitivo, conflitante ou ambíguo.

## Limites
Não realizar atividades de outros módulos; não depender de toolkit gráfico ou linguagem específica.

## Invariantes
Se arquivo mudar durante envio, reagendar sua versão atual. Respeitar `docs/invariants.md`.

## Qualidade
Especificar idempotência, efeitos colaterais, concorrência, cancelamento, erro e recuperação antes da implementação.
