# Contrato — local

## Propósito
Inventário e eventos do filesystem.

## Consumidores
changes,upload,download.

## Entrada
Objetos conceituais validados, identificadores estáveis, contexto de cancelamento e, quando pertinente, versões esperadas.

## Saída
Resultado tipado ou erro explícito e classificável: temporário, definitivo, conflito ou resultado ambíguo.

## Responsabilidade e limites
Inventário e eventos do filesystem. Não assumir responsabilidades de outros módulos nem depender da implementação gráfica.

## Invariante
Symlink não deve escapar da raiz autorizada.

## Qualidade de contrato
Toda operação deverá especificar efeitos colaterais, idempotência, concorrência permitida, pré/pós-condições e recuperação após interrupções. Nenhuma falha pode ser interpretada como sucesso.
