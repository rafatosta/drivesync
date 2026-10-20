# Contrato — changes

## Propósito
Coleta incremental local e remota.

## Consumidores
planner,coordinator.

## Entrada
Objetos conceituais validados, identificadores estáveis, contexto de cancelamento e, quando pertinente, versões esperadas.

## Saída
Resultado tipado ou erro explícito e classificável: temporário, definitivo, conflito ou resultado ambíguo.

## Responsabilidade e limites
Coleta incremental local e remota. Não assumir responsabilidades de outros módulos nem depender da implementação gráfica.

## Invariante
Estado desconhecido nunca é ausência.

## Qualidade de contrato
Toda operação deverá especificar efeitos colaterais, idempotência, concorrência permitida, pré/pós-condições e recuperação após interrupções. Nenhuma falha pode ser interpretada como sucesso.
