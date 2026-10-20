# Contrato — remote

## Propósito
Adaptador REST da Drive API v3.

## Consumidores
changes,upload,download.

## Entrada
Objetos conceituais validados, identificadores estáveis, contexto de cancelamento e, quando pertinente, versões esperadas.

## Saída
Resultado tipado ou erro explícito e classificável: temporário, definitivo, conflito ou resultado ambíguo.

## Responsabilidade e limites
Adaptador REST da Drive API v3. Não assumir responsabilidades de outros módulos nem depender da implementação gráfica.

## Invariante
403/404/429/5xx são erros distintos.

## Qualidade de contrato
Toda operação deverá especificar efeitos colaterais, idempotência, concorrência permitida, pré/pós-condições e recuperação após interrupções. Nenhuma falha pode ser interpretada como sucesso.
