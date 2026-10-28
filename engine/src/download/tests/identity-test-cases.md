# Oráculos de identidade — SYNC-002

**Contrato acordado; catálogo de verificadores e variantes.** Requisitos e limites: [RF-ID-01–07](../../../../docs/SOFTWARE_SPECIFICATION.md#contrato-de-identidade--sync-002).
Fonte dos IDs: [proposta de aceite I01–I28](https://github.com/rafatosta/drivesync/issues/2#issuecomment-6483197201).
Este catálogo relaciona testes existentes e variantes exigidas; não registra
execuções nem declara cenários aprovados. Resultados e comandos pertencem aos
commits/PR. A existência de um teste relacionado não comprova todas as variantes.

## Fixtures e oráculos comuns

Usar `dados.db` binário com bytes distinguíveis BASE-V1, BASE-V2, LOCAL-V3 e
OUTRO-OBJETO. Cada caso parte de estado isolado; criar B por instalação real
confirmada. Reabrir SQLite e reconstruir instâncias entre ciclos. Substituições
locais mantêm o original em outro caminho e verificam device/inode diferentes.

O remoto falso precisa fornecer metadados e mídia por ID, falhas explícitas,
registro dos IDs consultados, inventário paginado/eventos controlados e hooks
nas janelas de observação, metadados, transferência e instalação. O `Fake` em
[tests.rs](../tests.rs) fornece conteúdo por ID e registra IDs consultados;
`provider_uses_requested_identity_and_reports_missing_objects` verifica dois
objetos distintos e erros controlados para IDs ausentes. Eventos/paginação têm
fixtures próprias. Hooks adicionais ficam limitados às janelas necessárias.

Em todos os cenários verificar ação e motivo, IDs/revisões/caminhos/device/inode,
bytes locais/remotos/backups, B e fase durável, chamadas ao provedor, reabertura e
repetição. Bloqueio no planejamento exige zero mídia; divergência durante mídia
já iniciada impede instalação indevida. Escrita de observação/pendência é
permitida, confirmação indevida de B não. Não exigir cópia remota local nos casos
bloqueados: preservar cada versão disponível no seu lado é suficiente.

## Relação entre cenários e verificadores

Abreviações: **B** = [baseline_tests.rs](../baseline_tests.rs), **D** =
[tests.rs](../tests.rs), **U** = [update_tests.rs](../update_tests.rs), **E** =
[incremental_tests.rs](../incremental_tests.rs), **P** =
[planner/tests.rs](../../planner/tests.rs), **R** =
[remote/inventory/tests.rs](../../remote/inventory/tests.rs), **I** =
[identity_tests.rs](../identity_tests.rs), **C** =
[collision_tests.rs](../collision_tests.rs), **H** =
[restoration_tests.rs](../restoration_tests.rs).
A coluna final define o complemento necessário ao oráculo, não um resultado de execução.

| Caso | Teste existente relacionado | Variantes e complemento exigidos |
| --- | --- | --- |
| I01 | I `i01_continuity_reopens_without_media_or_identity_changes` | Confirmar continuidade de vínculo e identidade, reabrir/repetir sem mídia, instalação ou backup redundantes. |
| I02 | I `i02_content_edit_retains_identity_and_confirmed_base` | Comprovar mesmo device/inode e hash diferente; motivo de conteúdo, não substituição; B intacta. |
| I03 / C05 | I `i03_i04_replacements_keep_identity_diagnostic_despite_equal_metadata_or_bytes` | Bytes iguais/diferentes × remoto igual/atualizado; motivo explícito de identidade, ambas as versões preservadas. |
| I04 | I `i03_i04_replacements_keep_identity_diagnostic_despite_equal_metadata_or_bytes` | Igualar nome/tamanho/timestamp, variar bytes; comprovar inode diferente e nenhuma adoção. |
| I05 / C05 | I `i05_new_remote_id_never_inherits_the_old_installation` | Retirar A e inserir B no mesmo nome/pai, bytes iguais/diferentes; B sem base herdada, A/local preservados. |
| I06 | I `i06_same_remote_id_updates_only_after_durable_installation` | Mesmo ID, nova revisão/bytes, local íntegro; nova B somente após confirmação e V anterior preservada. |
| I07 | E `i07_same_id_path_changes_preserve_association_and_occupied_destinations` | Renomear/mover mesmo ID, destinos livre/ocupado; conferir diagnóstico anterior/atual, associação e B. Teste existente permite receber no destino livre e conserva origem. |
| I08 | I `i08_each_scope_component_requires_its_own_base` | Variar separadamente cliente, conta, raiz local/remota com nomes/bytes/IDs repetidos; nenhum vínculo alheio autoriza efeito. |
| I09 | I `i09_nonregular_or_unreadable_identity_never_becomes_absence`; I `i09_incomplete_persisted_identity_does_not_authorize_media_or_confirmation`; P `incomplete_identity_evidence_is_unknown_even_when_fingerprints_match` | Leitura falha, identidade incompleta, diretório e symlink; motivo/erro explícito, desconhecido não vira ausente, alvo externo intacto. |
| I10 / C20 | C `i10_i11_i13_homonyms_block_before_and_after_installation_across_pages` | Dois IDs mesmo pai/nome, bytes iguais/diferentes, sem L; integrar inventário/plano/execução, zero mídia e nenhum vencedor. |
| I11 / C20 | C `i10_i11_i13_homonyms_block_before_and_after_installation_across_pages` | Instalar A e adicionar B homônimo; preservar B de A e ambos remotos, sem transferência/avanço durante colisão. |
| I12 / C20 | C `i12_i13_folder_homonyms_block_distinct_descendants_in_every_order` | Duas pastas homônimas com filhos distintos; integrar execução e conferir IDs/árvores sem mistura. |
| I13 / C20 | C `i13_same_names_under_distinct_paths_install_the_correct_objects` | Separar colisões entre páginas, inverter ordem de I10–I12; controle com nomes iguais em pais/caminhos distintos. |
| I14 / C20 | C `i14_removed_or_renamed_homonym_revalidates_the_remaining_object` | Desfazer colisão por remoção/renomeação: restante A revalida B; restante B não adota local de A; repetir após reabertura. |
| I15 / N12 | H `i15_historical_bytes_at_a_newer_revision_preserve_the_current_version` | V1 → V2 → bytes V1 com revisão maior; continuidade, instalação durável e preservação de V2, sem afirmar intenção. |
| I16 / N12 | H `i16_historical_bytes_in_a_new_remote_object_never_inherit_base` | Após V2, V1 em novo ID; distinguir objeto e preservar local sem herdar B. |
| I17 / N12 | H `i17_local_history_distinguishes_content_from_identity_without_rewinding_base` | Restaurar V1 após V2 com mesmo/outro inode; distinguir conteúdo/identidade e conservar B de V2. |
| I18 / N12 | H `i18_regressive_zero_and_missing_revisions_are_not_historical_authority` | Revisão menor/zero/ausente; diagnóstico desconhecido/inconsistente e zero mídia/efeito indevido. |
| I19 / N12 | H `i19_historical_and_independent_edits_conflict_against_the_last_confirmed_base` | Histórico em um lado e edição independente no outro, inclusive offline; comparar com última B e preservar. |
| I20 / N12 | H `i20_known_historical_content_without_provenance_has_only_observable_diagnostics` | Sem proveniência, diagnosticar alteração/ambiguidade sem afirmar intenção. Variante com prova somente se houver suporte real especificado; hash antigo não basta. |
| I21 / N12 | H `i21_restored_or_missing_state_never_adopts_divergent_current_files` ; H `i21_joint_local_rollback_without_an_independent_anchor_looks_like_remote_change` | Banco antigo + arquivos atuais; arquivos antigos + banco atual; banco ausente + arquivos. Revalidar; explicitar impossibilidade de detectar reversão conjunta consistente sem âncora externa. |
| I22 / C05 | U `edits_and_inode_swaps_during_download_remain_untouched` | Troca após plano e durante streaming, bytes iguais/diferentes; identidade divergente impede instalação mesmo com plano antigo. |
| I23 / C05 | U `remote_change_during_update_preserves_old_file`; D `remote_version_changes_and_concurrent_local_creation_are_conflicts` | Revisão muda, ID desaparece, homônimo novo aparece; conferir IDs consultados e impedir troca por nome/instalação sob revisão errada. |
| I24 | U `network_checksum_and_cancellation_keep_old_file_at_its_path`; E `removal_is_durable_unavailability_and_preserves_user_bytes` | Indisponibilidade → substituição local/remota → retorno; reconciliar com B, sem inferir exclusão nem adoção. |
| I25 | B `cancellation_during_initial_or_update_transfer_does_not_confirm_partial_bytes`; B `later_incomplete_transfer_masks_old_base_until_verified_recovery` | Interrupção/cancelamento no streaming e após staging; nenhuma nova identidade/B confirmada, reabrir/repetir. |
| I26 | B `filesystem_effect_without_confirmation_never_exposes_old_or_new_base`; U `confirmation_failure_leaves_both_versions_and_recovers`; U `new_destination_after_backup_is_not_replaced_or_restored_over` | Efeito sem confirmação, replay normal/com substituto; comprovar identidade/bytes antes de confirmar, preservar substituto e evidenciar pendência. |
| I27 | B `corrupt_database_or_malformed_installation_evidence_stops_before_effects`; B `unreadable_database_is_an_error_and_is_not_reinitialized`; B `installed_phase_without_remote_revision_is_not_a_confirmed_base` | SQLite corrompido/incompatível e identidade incompleta; erro ou B desconhecida, sem reset/adotar por hash. Auditar validação de device/inode além de hash/revisão. |
| I28 | E `restart_no_changes_local_deletion_and_local_edit_are_reconciled`; E `historical_activity_never_suppresses_a_current_local_conflict` | Repetir após substituições/colisões/restaurações; duplicar/reordenar eventos, não ressuscitar associação, gerar backups extras ou perder pendências. |

## Contrato de diagnóstico e limites

- `Verified(String)` conserva o formato SHA-256:device:inode; o planner decodifica
  componentes e compara identidade antes dos bytes. Fingerprint incompleta não
  autoriza ação. Os motivos separados estão nos algoritmos do planner.
- I01–I09 verificam planos persistidos, bytes/B e ausência de mídia nos bloqueios.
  I07 usa checkpoint real para localização anterior/atual; não duplica persistência.
  I09 combina filesystem real (permissão negada, diretório/symlink), erro injetado
  no observador e evidência SQLite incompleta. Não demonstra falha real de disco.
- O executor revalida binários independentemente da ação planejada. O diagnóstico
  é verificável no plano; o relatório pode reduzir o erro a `DownloadConflict`.
  Não se exige alteração da GUI.
- I10–I14 usam o scanner real com páginas JSON por pai, ordenação direta/inversa
  e uma/todas as entradas por página. Conferem IDs e planos reabertos, zero mídia
  e nenhuma nova B durante ambiguidade; remoção/renomeação de cada homônimo exige
  reavaliação. Os testes relacionados não representam integração com Google.
- I15–I21 criam V1/V2 pelo executor e usam bytes históricos no mesmo/novo ID,
  edição/troca de inode, revisões menores/zero/ausentes e bancos/arquivos antigos.
  Os bloqueios comparam todos os bytes/inodes da raiz (incluindo backups), B e
  plano reaberto; a atualização histórica legítima confirma uma revisão maior e
  preserva V2. Zero/revisão ausente no modelo de binário produz erro seguro no
  executor, mantendo diagnóstico desconhecido no plano; não gera mídia.
- O intervalo offline de I19 representa ausência de observações entre ciclos,
  com reconstrução de instâncias na volta. Não é simulação de falha de rede.
- I22–I28 continuam sujeitos às variantes do catálogo; testes relacionados não
  representam conclusão automática dessas baterias.
- Para I20, aplicar RF-ID-06: não há proveniência explícita de restauração no
  inventário atual. Histórico de hashes prova conteúdo conhecido, não intenção.
  A variante com prova não é aplicável à interface atual; a ausência de prova
  é exercitada explicitamente. Não introduzir nova dependência ou serviço de
  histórico para simular essa prova.
- I21 também exercita B/L antigos consistentes, com inode original restaurado e
  R atual mais novo: a observação equivale a atualização remota comum, sem sinal
  que comprove rollback local. O efeito seguro ainda preserva a versão anterior.
  Se R e todos os vestígios também coincidirem com o passado, não há detecção
  prometida sem âncora independente (RF-ID-07).

O aceite local exige as variantes aplicáveis e regressões da #1 no SDK/Rust do
manifesto Flatpak, incluindo fmt, Clippy, workspace tests, documentação e doctests.
Não comprova Google Drive real, reinício real do aplicativo, queda de energia,
falha real de fsync ou funcionamento do Flatpak instalado/portal.
