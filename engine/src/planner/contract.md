# Contrato — planner

## Propósito
Comparação tripla B/L/R e plano puro.

## Consumidores
coordinator,queue.

## Entrada
Objetos conceituais validados, identificadores estáveis, contexto de cancelamento e, quando pertinente, versões esperadas.

## Saída
Resultado tipado ou erro explícito e classificável: temporário, definitivo, conflito ou resultado ambíguo.

## Responsabilidade e limites
Comparação tripla B/L/R e plano puro. Não assumir responsabilidades de outros módulos nem depender da implementação gráfica.

## Invariante
Sem efeitos colaterais, sem exclusões com UNKNOWN.

## Qualidade de contrato
Toda operação deverá especificar efeitos colaterais, idempotência, concorrência permitida, pré/pós-condições e recuperação após interrupções. Nenhuma falha pode ser interpretada como sucesso.

## Impacto de lotes — SYNC-014

BULK-001 agrega a população protetiva de identidades previamente confirmadas e
os diagnósticos atuais por raiz e pasta ancestral. Retorna uma revisão sem efeitos
nem permissão de exclusão. Política, limiares, digest de evidência e critérios em
[proteção de lotes](../../../docs/SOFTWARE_SPECIFICATION.md#proteção-de-lotes--sync-014).

## Permissões e atributos — SYNC-018

Acesso remoto negado, capacidade desconhecida e integridade ausente geram,
respectivamente, `AccessDenied`, `Unknown` e `MetadataUnavailable` na projeção
de diagnósticos. Nenhum oferece escolha de versão nem autoriza efeito.
O mapeamento de atributos e os limites estão na
[especificação vigente](../../../docs/SOFTWARE_SPECIFICATION.md#permissões-e-atributos--sync-018).
