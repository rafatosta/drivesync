# Algoritmos — conflicts

**Estado:** proposta v0.2, independente da linguagem.

## Algoritmos
CONFLICT-001,CONFLICT-002.

## Objetivo
Preservar versões e resolver divergências.

## Pré-condições
Configuração validada, identidade disponível, entradas consistentes ou falha explícita.

## Passos
1. Congelar item.
2. preservar as duas versões com nomes sem colisões.
3. registrar origem das cópias.
4. obter decisão.
5. revalidar versões antes de resolver.

## Pós-condições
Estado confirmado e persistido, ou erro tipado com efeitos parciais conhecidos.

## Erros
Rede, permissão, versão concorrente, armazenamento insuficiente, corrupção persistente, cancelamento, resultado remoto ambíguo.

## O que verificar
Identidade, versão, estado anterior, integridade, efeito externo e confirmação durável quando aplicáveis.

## O que não inferir
Timeout não é exclusão; nome não é identidade; sucesso HTTP isolado não confirma persistência local; UNKNOWN não significa ausente.

## Invariante
Não sobrescrever dados não preservados.

## Cenários e dependências revisados

Os algoritmos CONFLICT-001/002 devem considerar [56 casos e políticas](validation-checklist.md), [viabilidade](technical-feasibility.md) e [pesquisa de UX](ux-research.md). Em especial: não confundir erro com conflito, revalidar decisões obsoletas (N06), bloquear ações massivas (N08), suspender raízes inacessíveis (N03/N04) e reconciliar operações remotas ambíguas (N11). Não adicionar implementação com base nesta revisão.

## Testabilidade
Executar nominal, entrada inválida, falha de rede, repetição, interrupção e concorrência. Os dados esperados são definidos antes da implementação.

## Complexidade
Para n arquivos, planejamento e varreduras O(n); lookup indexado típico O(log n); transferência de B bytes O(B). A correção operacional é dominada por falhas e estados concorrentes.


## Fundamentação técnica por algoritmo

Esta seção distingue **solução consolidada**, **decisão do DriveSync**, **teste verificável** e **hipótese ainda aberta**. Consulte [método de desenvolvimento](../../../docs/development-methodology.md) e [catálogo de fontes](../../../docs/reference-catalog.md). Os links são referências de projeto, não provas de que a implementação esteja pronta.

### CONFLICT-001

- **Técnica conhecida:** Detecção/preservação de edições divergentes.
- **Referências primárias:** [Unison file synchronizer](https://www.cis.upenn.edu/~bcpierce/unison/).
- **Política do DriveSync:** Preservar ambas as versões com nomes reservados e identidade rastreável; decisão pendente não bloqueia outros itens independentes.
- **Testes/oráculos a detalhar:** Mesmo nome para duas cópias, conflito offline, crash durante preservação.
- **Limitações / questões abertas:** Política de retenção e apresentação de cópias ainda a decidir.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.

### CONFLICT-002

- **Técnica conhecida:** Resolução assistida pelo usuário.
- **Referências primárias:** [Unison file synchronizer](https://www.cis.upenn.edu/~bcpierce/unison/).
- **Política do DriveSync:** Usuário escolhe manter local, remoto ou ambos; revalidar versões antes de executar; janela oculta mantém conflito pendente.
- **Testes/oráculos a detalhar:** Usuário escolhe após arquivo mudar de novo, reinicia com decisão pendente.
- **Limitações / questões abertas:** Sem auto-merge na primeira versão; definir UX apenas após contrato seguro.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.
