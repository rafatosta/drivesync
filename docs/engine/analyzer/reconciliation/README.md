# Reconciliador

## Objetivo

Comparar o estado anterior confirmado (**baseline**), o estado local e o estado remoto de **um `SyncItem`** e produzir uma **lista ordenada de transformações propostas** para reconciliar os estados.

O Reconciliador é um algoritmo privado e independente do [Analisador](../README.md). Recebe todos os dados necessários e devolve uma avaliação tipada, sem acessar serviços externos, alterar o Inventário ou executar os comandos propostos.

## Funcionamento

1. Recebe um `SyncItem`, com snapshots, indicadores de presença, identidade, revisão e baseline opcional.
2. Compara separadamente presença, conteúdo, nome, localização e atributos relevantes, distinguindo diferenças comprovadas de evidências desconhecidas.
3. Detecta transformações unilaterais e compatíveis cuja execução é determinável sem decisão externa.
4. Produz **zero ou mais operações** em ordem, com dependências e precondições, para o mesmo item.
5. Devolve separadamente divergências que exigem avaliação pelo módulo [Conflitos](../conflicts/README.md) e requisitos de nova observação quando faltarem dados.

O resultado é um **plano**, não a confirmação de que houve execução. O Analisador coordena a avaliação de conflitos quando necessário, valida o plano e usa o [Operador](../../operations/INTERFACES.md) para executá-lo.

## Entrada e saída

```text
SyncItem
├── baseline (última sincronização confirmada)
├── local (snapshot e presença)
└── remote (snapshot e presença)
              │
              ▼
         Reconciliador
              │
              ▼
ReconciliationAssessment
├── operations[]      (zero ou mais comandos propostos)
├── divergences[]     (situações para avaliar separadamente)
└── requirements[]    (evidência que precisa ser obtida)
```

A saída pode conter operações e pendências simultaneamente: por exemplo, uma mudança de nome compatível com uma divergência de conteúdo ainda não decidida. O Analisador avalia **dependências e segurança antes de aplicar qualquer operação**; presença de um comando não autoriza executá-lo se sua validade depender de uma divergência não resolvida.

## Regras essenciais

- `PresenceState::Unknown` não equivale a recurso excluído. Somente `AbsentConfirmed` sustentado por observação confiável pode fundamentar exclusão.
- Um arquivo novo local com ausência remota comprovada pode gerar `Upload`; a situação inversa pode gerar `Download`.
- Arquivo existente com conteúdo alterado gera proposta de `UpdateContent`, preservando a identidade do destino.
- Renomeações e movimentações remotas são propostas por referência/ID, não por reenvio do conteúdo.
- Alterações simultâneas compatíveis podem produzir uma sequência de comandos; alterações incompatíveis são relatadas como divergências.
- Quando `baseline=None`, não se inventa estado histórico: a decisão exige evidência suficiente de identidade e presença, especialmente na primeira sincronização.
- Para diretórios e descendentes, registrar dependências conhecidas; se o plano depende de itens que não fazem parte da entrada, informar a dependência ao Analisador, em vez de presumir sua execução.
- O Reconciliador não escolhe qual versão descartar e não toma decisões pelo usuário.
- Os comandos não são persistidos nem executados por este módulo.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Orientações para agentes](AGENTS.md)

**Estado:** especificação documental inicial. As estruturas representam o contrato proposto, não uma implementação Rust comprovada.
