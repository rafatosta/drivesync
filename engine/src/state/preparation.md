# Preparação durável de downloads (STATE-001 / PLAN-001)

Toda a persistência necessária para identificar e controlar uma raiz local fica
em `.drivesync/`, dentro da pasta selecionada. `identity.json` vincula a raiz ao
cliente OAuth, à conta e à pasta remota; `state.sqlite3` registra o journal de
operações e evidências de arquivos instalados; `changes.sqlite3` mantém o
checkpoint remoto; `uploads.sqlite3` guarda preferências e intents de upload;
`reconciliation.sqlite3` registra o inventário local completo, observações remotas,
B consultada e diagnósticos anteriores à execução.
Esses arquivos sobrevivem à remoção dos dados privados do aplicativo e não
armazenam tokens. A pasta `.drivesync` é excluída do inventário de upload e é um
caminho reservado que o inventário remoto não pode usar como destino.

O estado global de contas e autenticação continua no diretório privado do
aplicativo; ele não identifica nem controla uma raiz local específica.

O journal por raiz usa schema 2, application_id próprio, foreign_keys ON,
synchronous FULL e journal DELETE. Transações IMMEDIATE publicam atomicamente uma
nova geração de inventário e plano da raiz. Itens não observados permanecem como
evidência antiga, sem gerar exclusões. O journal portátil preserva evidência
operacional de downloads entre reinstalações, mas não restaura autenticação nem
substitui a verificação de identidade e conteúdo feita pelo executor.
Chaves compostas contêm cliente OAuth + conta + raiz remota + raiz local; itens
referenciam essa identidade por chave estrangeira. Corrupção, schema estranho,
symlink e banco de outra aplicação são erros: nunca apagar/recriar para recuperar.
Falhas de inventário/plano não publicam uma geração; cancelamento observado
antes do commit provoca rollback.

O [planner](../planner/algorithms.md) classifica usando somente a base confirmada
extraída do [journal](algorithms.md). A fachada combina essas bases com observação
atual de um inventário local completo, com estabilização e fingerprint por
descritores para arquivos regulares. Sem B, os bytes observados não autorizam adoção. O plano persistido não é autorização de escrita. O executor implementado
reabre/revalida destinos antes de instalar e faz replay conforme
[download inicial](../download/execution.md) e [atualizações](../download/updates.md).

## Rastreabilidade das regras do eixo 1

Requisitos e aceite: [RF-SYNC-01–05](../../../docs/SOFTWARE_SPECIFICATION.md#47-detecção-de-mudanças-e-sincronização).
A01–A12 referem-se ao [comentário de aceite da issue #1](https://github.com/rafatosta/drivesync/issues/1#issuecomment-6480152816).
Os nomes abaixo identificam funções de teste nos arquivos vinculados, não resultados
de execução com Google. Detalhes de comandos/resultados pertencem aos commits.

| Regra / cenário | Implementação | Testes verificadores |
| --- | --- | --- |
| A01: instalação confirmada, reabertura e repetição sem download redundante | [executor](../download/execution.rs), [bases](downloads.rs) | `installs_verifies_reopens_and_never_overwrites_local_changes` em [download/tests.rs](../download/tests.rs); `updated_base_survives_reopen_and_classifies_later_local_edit_against_v2` em [baseline_tests.rs](../download/baseline_tests.rs) |
| A02: observar/preparar não confirma instalação | [save](mod.rs), [confirmed_bases](downloads.rs) | `observed_only_and_first_sync_never_adopt_equal_or_different_local_bytes` em [baseline_tests.rs](../download/baseline_tests.rs) |
| A03: mudança remota permite atualização, B nova só após confirmação | [executor](../download/execution.rs), [atualizações](../download/execution/updates.rs) | `updates_reopens_and_preserves_prior_versions_including_empty_updates` em [update_tests.rs](../download/update_tests.rs); `filesystem_effect_without_confirmation_never_exposes_old_or_new_base` em [baseline_tests.rs](../download/baseline_tests.rs) |
| A04: mudança local isolada bloqueia download e preserva B | [decide](../planner/mod.rs), [executor](../download/execution.rs) | `reopened_base_classifies_local_and_offline_concurrent_edits_without_advancing` em [baseline_tests.rs](../download/baseline_tests.rs) |
| A05/C01 e A06/C06: divergência após reabertura é concorrência em relação a B, sem timestamps | [planner](../planner/mod.rs), [fingerprint](../download/filesystem.rs) | `three_way_classification_requires_a_confirmed_base_and_fresh_local_fingerprint` em [planner/tests.rs](../planner/tests.rs); `reopened_base_classifies_local_and_offline_concurrent_edits_without_advancing` em [baseline_tests.rs](../download/baseline_tests.rs) |
| A07/N01: arquivo preexistente, igual ou diferente, não é adotado | [planner](../planner/mod.rs), [executor](../download/execution.rs) | `observed_only_and_first_sync_never_adopt_equal_or_different_local_bytes` em [baseline_tests.rs](../download/baseline_tests.rs) |
| A08/N02: perda do banco preserva dados, sem reconstruir B | [state](mod.rs), [planner](../planner/mod.rs) | `missing_or_observation_only_state_cannot_reconstruct_a_base` em [baseline_tests.rs](../download/baseline_tests.rs) |
| A09/N02: estado inválido gera erro; evidência incompleta não fornece B | [abertura](mod.rs), [validação do journal](downloads.rs) | `corrupt_database_or_malformed_installation_evidence_stops_before_effects`, `unreadable_database_is_an_error_and_is_not_reinitialized`, `installed_phase_without_remote_revision_is_not_a_confirmed_base`, `missing_or_observation_only_state_cannot_reconstruct_a_base` em [baseline_tests.rs](../download/baseline_tests.rs); `cancellation_and_foreign_schema_preserve_state_and_private_permissions` em [state/tests.rs](tests.rs) |
| A10: transferência parcial/cancelada não confirma B; operação posterior incompleta mascara B antiga | [fases](../download/models.rs), [bases](downloads.rs), [stream](../download/stream.rs) | `later_incomplete_transfer_masks_old_base_until_verified_recovery`, `cancellation_during_initial_or_update_transfer_does_not_confirm_partial_bytes` em [baseline_tests.rs](../download/baseline_tests.rs) |
| A10: efeito no filesystem sem confirmação exige replay verificado | [instalação](../download/execution.rs), [replay da atualização](../download/execution/updates.rs) | `filesystem_effect_without_confirmation_never_exposes_old_or_new_base` em [baseline_tests.rs](../download/baseline_tests.rs); `ready_replays_before_and_after_atomic_rename` em [download/tests.rs](../download/tests.rs); `recovery_covers_both_renames_and_does_not_redownload`, `new_destination_after_backup_is_not_replaced_or_restored_over` em [update_tests.rs](../download/update_tests.rs) |
| A11: V2 confirmada passa a ser a base da edição local posterior | [bases](downloads.rs), [planner](../planner/mod.rs) | `updated_base_survives_reopen_and_classifies_later_local_edit_against_v2` em [baseline_tests.rs](../download/baseline_tests.rs) |
| A12: mesmo hash com outro inode ou outro escopo não herda autorização | [fingerprint](../download/filesystem.rs), [consulta por escopo](downloads.rs), [executor](../download/execution.rs) | `inode_replacement_and_foreign_scopes_never_inherit_write_authority` em [baseline_tests.rs](../download/baseline_tests.rs); `confirmed_installation_drives_item_scoped_three_way_planning`, `existing_identical_file_is_not_adopted_and_accounts_are_isolated` em [download/tests.rs](../download/tests.rs) |
| Sem exclusão inferida de ausência de B/evento remoto | [ações sem delete](../planner/mod.rs), [ciclo incremental](../changes/mod.rs) | `missing_or_observation_only_state_cannot_reconstruct_a_base` em [baseline_tests.rs](../download/baseline_tests.rs); `removal_is_durable_unavailability_and_preserves_user_bytes` em [incremental_tests.rs](../download/incremental_tests.rs) |

## Natureza da evidência e limitações

- **Implementado:** B por item/escopo, classificação parcial sem escrita,
  observações separadas de instalações e replay local de downloads/atualizações.
- **Verificado pelos testes automatizados:** decisões puras e efeitos com
  arquivos/SQLite reais em diretórios temporários. `Fake` fornece bytes/metadados;
  `Feed` simula eventos. As fixtures reabrem bancos entre ciclos e injetam erros
  de leitura, cancelamento e triggers SQLite que recusam a confirmação final.
  Algumas janelas de rename são reconstruídas a partir de uma instalação válida.
- **Não demonstrado:** reinício real do aplicativo/processo, queda de energia,
  falha real de fsync, comportamento do empacotamento/portal e apresentação das
  classificações na interface. Preservação de bytes sozinha não demonstra a
  classificação; os testes de planejamento a verificam separadamente.
- **Google Drive real:** a bateria A01–A12 permanece pendente. O smoke test
  ignorado `saved_account_can_browse_drive_folders` em
  [live_google_tests.rs](../facade/live_google_tests.rs) cobre apenas navegação e
  metadados; não demonstra B nem aceite de sincronização. Não foi habilitado na
  bateria com provedor falso. N01/N02 e C01/C06 não recebem aceite real por esses
  testes locais.

Referências oficiais consultadas em 2026-10-23:
- https://www.sqlite.org/atomiccommit.html
- https://www.sqlite.org/pragma.html#pragma_integrity_check
- https://docs.rs/rusqlite/0.37.0/rusqlite/struct.OpenFlags.html

## SYNC-003 — observações de reconciliação

[reconciliation.rs](reconciliation.rs) mantém formato próprio 1, application_id
próprio e as mesmas garantias de abertura privada, integridade e rejeição sem
reset dos demais bancos. Uma transação publica B/L/R, plano e diagnósticos juntos
por cliente, conta e raízes, usando CAS para recusar trabalhador obsoleto.
`save` não altera o journal de instalação nem o cursor remoto. Falha de commit
ou cancelamento preserva o registro anterior; o próximo ciclo sempre observa de
novo. Não existe atomicidade entre os três bancos e o filesystem.

Preparação e ciclos com feed usam o mesmo classificador. Paths históricos do
feed/journal não viram arquivos novos para upload. Arquivo local desaparecido
sem representação remota fica ausente observado; descendentes de ancestral
inacessível/especial continuam desconhecidos. `saved_reconciliation` permite
ler os diagnósticos sem rede, inclusive após reabertura, sem executar o plano.

Oráculos de persistência estão em [reconciliation/tests.rs](reconciliation/tests.rs):
reabertura integral de evidências, isolamento, CAS, cancelamento, rollback por
trigger, formato incompatível, JSON inválido, banco estrangeiro, symlink e
ancestral inacessível. Os testes C06/C21 integrados ficam em
[download/reconciliation_tests.rs](../download/reconciliation_tests.rs).
