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
Se a origem mudar durante envio, preservar snapshot/intenção e bloquear confirmação
sobre bytes divergentes. Respeitar [invariantes](../../../docs/old/invariants.md).
Resultado remoto incerto exige consulta pelo mesmo ID antes de novo efeito.
O orçamento persistente admite cinco passes; espera exponencial e esgotamento
não autorizam substituir ID nem apagar evidência. O contrato executável e as
transições estão em [create-only.md](create-only.md#sync-013--uncertain-outcomes-and-bounded-replay).

## Qualidade
Especificar idempotência, efeitos colaterais, concorrência, cancelamento, erro e recuperação antes da implementação.
