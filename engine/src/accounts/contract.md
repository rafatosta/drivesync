# Contrato — accounts

## Propósito
Identidade de contas e vínculos com pastas.

## Consumidores
coordinator,ui.

## Entrada
Objetos conceituais validados, identificadores estáveis, contexto de cancelamento e, quando pertinente, versões esperadas.

## Saída
Resultado tipado ou erro explícito e classificável: temporário, definitivo, conflito ou resultado ambíguo.

## Responsabilidade e limites
Identidade de contas e vínculos com pastas. Não assumir responsabilidades de outros módulos nem depender da implementação gráfica.

## Invariante
Desvincular nunca apaga conteúdo local ou remoto.

## Qualidade de contrato
Toda operação deverá especificar efeitos colaterais, idempotência, concorrência permitida, pré/pós-condições e recuperação após interrupções. Nenhuma falha pode ser interpretada como sucesso.
