# Algoritmos — conflicts

**Estado:** classificação SYNC-006 implementada; preservação geral e resolução assistida propostas.

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

## Classificação SYNC-006

Contrato e precedência: [contract.md](contract.md). A decisão pura compara deltas
normalizados de L/R em relação a B; o adaptador retém as evidências no escopo.
Não há transferência, mutação ou confirmação de B na classificação. A coleta
pode ler conteúdo remoto em sink para comprovar igualdade/diferença, validando
metadados antes/depois. A prova compara R com B e com L: revisão remota de
metadados não transforma uma edição exclusivamente local em conflito de conteúdo.

| Cenário | Oráculo e referência |
| --- | --- |
| C01 | `c01_c03_c04_and_p01_three_way_matrix` em [tests.rs](tests.rs); divergência real integrada em `c06_reopened_offline_local_remote_and_divergent_changes_preserve_bases` de [download/reconciliation_tests.rs](../download/reconciliation_tests.rs). |
| C03 | Matriz pura distingue exclusão confirmada versus edição. `c03_unlisted_remote_with_local_edit_is_unavailable_not_confirmed_deletion` em [sync006_acceptance.rs](../download/sync006_acceptance.rs) cobre o limite real do inventário: não listado não prova exclusão. |
| C04 | Matriz pura para exclusão confirmada; `c04_missing_local_and_new_remote_revision_require_review_after_reopen` e `local_removal_after_remote_update_planning_invalidates_the_candidate` em [reconciliation_tests.rs](../download/reconciliation_tests.rs) preservam R/B e não recriam L silenciosamente. |
| C09/C12 | `c09_c12_independent_changes_are_conciliable_in_both_directions` em [tests.rs](tests.rs); `c09_c12_classify_both_directions_persist_and_repeat_without_duplicate_copies` em [sync006_acceptance.rs](../download/sync006_acceptance.rs) usa inventários reais, B reaberta e prova remota simulada nos dois sentidos. Aplicação da combinação permanece pendente. |
| C21 | `c21_equal_content_and_matching_locations_converge_without_false_conflict` em [tests.rs](tests.rs); os testes C21 de [reconciliation_tests.rs](../download/reconciliation_tests.rs) verificam timestamps, identidades, revalidação e repetição; `metadata_only_remote_revision_does_not_turn_local_edit_into_content_conflict` impede conflito falso por revisão de metadados. |
| P01 | Matriz pura; testes de baseline/download mantêm criação/atualização segura para mudanças unilaterais suportadas e nenhuma escrita para ausência de mudança. Atualização remota continua bloqueada. |
| Falha/reinício/offline | `location_content_proof_offline_or_stale_remains_unknown_and_preserves_base`, `failed_observation_commit_and_cancellation_keep_the_previous_classification` em [sync006_acceptance.rs](../download/sync006_acceptance.rs); provedores simulados, arquivos e SQLite reais isolados. |
| Independência e identidade | `conciliable_item_does_not_block_independent_downloads`, `equal_bytes_in_a_different_identity_never_prove_a_local_relocation` e `conflicting_names_or_content_remain_real_conflicts_not_conciliable` em [sync006_acceptance.rs](../download/sync006_acceptance.rs); `relocated_managed_identity_is_not_uploaded_as_a_new_file` em [upload/tests.rs](../upload/tests.rs). |

Essas referências definem oráculos e limites, não resultados de execução com
Google Drive real, teste manual de interface, queda real de processo/energia ou
aplicação de operações ainda não suportadas. Evidências/comandos ficam no commit/PR.
