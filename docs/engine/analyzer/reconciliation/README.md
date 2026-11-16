# Reconciliador

## Objetivo

Comparar o estado anterior confirmado (**baseline**), o estado local e o estado remoto de **um `SyncItem`** e produzir **decisões ordenadas de transformação** para reconciliar os estados.

O Reconciliador é um algoritmo privado e independente do [Analisador](../README.md). Recebe todos os dados necessários e devolve uma avaliação tipada, sem acessar serviços externos, alterar o Inventário ou executar os comandos propostos.

## Funcionamento

1. Recebe um `SyncItem`, com snapshots, indicadores de presença, identidade, revisão e baseline opcional.
2. Compara separadamente presença, conteúdo, nome e localização, distinguindo diferenças comprovadas de evidências desconhecidas.
3. Detecta transformações unilaterais e compatíveis cuja execução é determinável sem decisão externa.
4. Produz **zero ou mais decisões semânticas** em ordem fixa para o mesmo item. A materialização em comandos e suas dependências ainda não foi implementada.
5. Devolve separadamente divergências que exigem avaliação pelo módulo [Conflitos](../conflicts/README.md) e requisitos de nova observação quando faltarem dados.

O resultado é uma **avaliação de decisões**, sem comandos executáveis ou confirmação de execução. O Analisador coordena a avaliação de conflitos quando necessário, valida o plano e usa o [Operador](../../operations/INTERFACES.md) para executá-lo.

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
├── decisions[]       (intenções, sem comandos executáveis)
├── divergences[]     (situações para avaliar separadamente)
└── requirements[]    (evidência que precisa ser obtida)
```

A saída pode conter decisões e pendências simultaneamente: por exemplo, uma mudança de nome compatível com uma divergência de conteúdo ainda não decidida. O Analisador avalia **dependências e segurança antes de aplicar qualquer operação**; presença de uma decisão não autoriza executá-la se sua validade depender de uma divergência não resolvida.

## Regras essenciais

- `PresenceState::Unknown` não equivale a recurso excluído. Somente `AbsentConfirmed` sustentado por observação confiável pode fundamentar exclusão.
- Um arquivo novo local com ausência remota comprovada gera intenção de criação remota; a situação inversa gera intenção de criação local. A direção persistida é `Upload`/`Download`.
- Na primeira observação de um item sem baseline nem vínculo/snapshot do outro lado, a origem completa já identifica a direção de replicação: local → `Upload`, remoto → `Download`. Presença oposta continua `Unknown`; a intenção permanece bloqueada por reobservação, verificação de identidade e resolução do destino. Não é prova de ausência nem autorização para criar uma duplicata.
- Arquivo existente com conteúdo alterado unilateralmente gera decisão de propagação, preservando a identidade do destino.
- Renomeações e movimentações remotas são propostas por referência/ID, não por reenvio do conteúdo.
- Alterações simultâneas compatíveis podem produzir uma sequência de decisões; alterações incompatíveis são relatadas como divergências.
- Quando `baseline=None`, não se inventa estado histórico: a decisão exige evidência suficiente de identidade e presença, especialmente na primeira sincronização.
- Destinos e pais ainda não resolvidos exigem evidências; a montagem futura de comandos deverá registrar dependências sem presumir execução de outros itens.
- O Reconciliador não escolhe qual versão descartar e não toma decisões pelo usuário.
- Os comandos não são persistidos nem executados por este módulo.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Orientações para agentes](AGENTS.md)

**Estado:** núcleo puro implementado e testado em `engine/src/analyzer/reconciliation/`. Criação, atualização, rename e move são decisões semânticas; comandos do Operador, dependências executáveis e propagação de metadados continuam pendentes. Consulte as [interfaces implementadas](INTERFACES.md) para a representação real e seus limites.
