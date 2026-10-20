# Algoritmos — planner

**Status:** proposta v0.2, independente da linguagem.

## Identificadores
PLAN-001 planejar; SYNC-002 revalidar.

## Objetivo
Comparação tripla B/L/R e plano puro.

## Pré-condições
Configuração validada, dependências disponíveis ou erro explícito, estado prévio recuperável.

## Algoritmo conceitual
1. Carregar base B e observações L/R por ID.
2. classificar modificações, ausência ou desconhecido.
3. unilateral gera transferência.
4. divergência bilateral gera conflito.
5. anexar pré-condições ao plano.

## Pós-condições
Estado confirmado e persistido, ou erro tipado com efeitos parciais identificáveis.

## Possíveis erros
Entrada inválida; perda de conectividade; autorização negada; falta de espaço; versão alterada; erro de I/O; estado desconhecido; falha entre efeito externo e commit.

## O que verificar
IDs e revisões; pré-condições imediatamente antes de ações destrutivas; retorno e efeitos persistidos; invariantes globais e limites desta caixa-preta.

## O que não verificar nem inferir
Não inferir ausência por timeout/erro HTTP, nem identidade apenas pelo nome, nem sucesso pela ausência de exceções. Não inspecionar conteúdo do usuário sem necessidade da operação.

## Invariantes
Sem efeitos colaterais, sem exclusões com UNKNOWN. Aplicam-se também os invariantes globais de `docs/invariants.md`.

## Complexidade
Consultas pontuais tipicamente O(1) em metadados; inventário e planejamento O(n); transferências de B bytes O(B). Latência de rede, disco e consistência entre operações dominam o custo real.

## Critérios de aceitação
Caso nominal, cancelamento, repetição sem duplicação indevida, erro transitório, erro definitivo, falha parcial e reinicialização. Revisar as decisões pendentes antes de codificar.


## Fundamentação técnica por algoritmo

Esta seção distingue **solução consolidada**, **decisão do DriveSync**, **teste verificável** e **hipótese ainda aberta**. Consulte [método de desenvolvimento](../../../docs/development-methodology.md) e [catálogo de fontes](../../../docs/reference-catalog.md). Os links são referências de projeto, não provas de que a implementação esteja pronta.

### PLAN-001

- **Técnica conhecida:** Reconciliação de três estados (base, local, remoto).
- **Referências primárias:** [Unison file synchronizer](https://www.cis.upenn.edu/~bcpierce/unison/), [Google Drive changes](https://developers.google.com/workspace/drive/api/guides/manage-changes).
- **Política do DriveSync:** Planejar sem efeitos; divergências de dois lados viram CONFLICT; UNKNOWN bloqueia operações destrutivas.
- **Testes/oráculos a detalhar:** Tabela completa B/L/R (alterado, excluído, igual e unknown), tipos especiais, renomeação.
- **Limitações / questões abertas:** Mapear versões e IDs reais da Drive API, inclusive múltiplos pais e nomes iguais.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.

### SYNC-002

- **Técnica conhecida:** Revalidação de precondição e controle de corrida.
- **Referências primárias:** [HTTP RFC 9110](https://www.rfc-editor.org/rfc/rfc9110), [Drive files.update](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/update).
- **Política do DriveSync:** Verificar identidade/versão antes de alterar; comparar antes do write sozinho NÃO elimina TOCTOU; se não houver update condicional comprovado, adotar política conservadora de preservação.
- **Testes/oráculos a detalhar:** Alteração remota entre GET e PATCH, falha antes da confirmação, perda de acesso.
- **Limitações / questões abertas:** **Bloqueio de viabilidade:** verificar suporte concreto a precondição atômica da Drive API antes de autorizar substituições.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.
