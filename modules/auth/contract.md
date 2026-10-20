# Contrato — auth

## Propósito
Autenticação OAuth 2.0 com PKCE e refresh.

## Consumidores
accounts,remote.

## Entrada
Objetos conceituais validados, identificadores estáveis, contexto de cancelamento e, quando pertinente, versões esperadas.

## Saída
Resultado tipado ou erro explícito e classificável: temporário, definitivo, conflito ou resultado ambíguo.

## Responsabilidade e limites
Autenticação OAuth 2.0 com PKCE e refresh. Não assumir responsabilidades de outros módulos nem depender da implementação gráfica.

## Invariante
Não registrar tokens; falha de rede não significa revogação.

## Qualidade de contrato
Toda operação deverá especificar efeitos colaterais, idempotência, concorrência permitida, pré/pós-condições e recuperação após interrupções. Nenhuma falha pode ser interpretada como sucesso.
