# Matriz de testes — state

| ID | Cenário | Resultado esperado |
|---|---|---|
| STATE-T01 | Entrada e estado normais | Resultado corresponde ao contrato |
| STATE-T02 | Parâmetros inválidos | Erro tipado, sem efeitos |
| STATE-T03 | Rede ou disco indisponível | Preservação, retry controlado |
| STATE-T04 | Interrupção antes da persistência final | Reconciliação sem perda de dados |
| STATE-T05 | Execução repetida | Idempotência ou bloqueio com reconciliação |
| STATE-T06 | Concorrência e dados alterados | Conflito explícito, sem sobrescrita silenciosa |

Critérios de aprovação devem ser independentes da implementação. Antes de programar, completar fixtures de entrada, estado anterior e estado esperado para cada caso.

## SYNC-005: recuperação não destrutiva

| Cenário | Oráculo e teste |
| --- | --- |
| N02 nominal / repetição | `healthy_stores_are_retained_and_recovery_is_idempotent`: bancos válidos mantêm caminhos e conteúdo. |
| N02 corrupção / observação íntegra | `corrupt_journal_rebuild_preserves_files_sidecars_and_healthy_observations`: originais, sidecars, cursor e bytes do usuário preservados; nenhuma B artificial. |
| N02 corrupção semântica / incompatibilidade | `semantic_corruption_and_incompatible_formats_are_preserved`: JSON inválido e versão incompatível substituídos só pela ação explícita. |
| N02 resultado remoto incerto | `missing_upload_intents_remain_blocked_across_restart_and_repetition`: bloqueio de upload durável. |
| Cancelamento / caminhos inseguros | `cancellation_unsafe_files_and_invalid_selectors_fail_closed`; `interruption_before_publication_keeps_old_selector_and_candidates`. |
| Reinício / checkpoint | `interrupted_sqlite_transaction_rolls_back_without_losing_confirmed_cursor`: subprocesso termina sem destructors com transação incompleta; SQLite real recupera cursor anterior. |
| Seleção perdida / publicação interrompida | `lost_or_corrupt_selector_never_revives_stale_original_baselines`; `interruption_after_no_fallback_marker_is_explicitly_recoverable`: nenhuma volta silenciosa a geração antiga. |
| Perda posterior | `lost_selected_database_fails_closed_until_explicit_recovery`: banco selecionado perdido não é recriado por leitura normal. |
| N02/E10 integração / concorrência / repetição | `download::tests::recovery_tests::sync005_rebuilt_state_preserves_preexisting_bytes_and_retries_integrity_failures`: provedor falso, filesystem/SQLite reais, lock concorrente recusado, conteúdo divergente não confirmado. |
| E05 efeito sem confirmação / parcial | `download::tests::recovery_tests::i25_incomplete_stream_and_verified_staging_never_confirm_a_base`; `i26_actual_effect_without_confirmation_replays_only_the_verified_identity`. |
| Cursor observado antes de efeito | `download::tests::incremental_tests::observed_update_survives_failed_transfer_and_retries_without_another_event`. |

Reconstrução local não usa rede; offline é condição estrutural desse protocolo.
Resultados de execução pertencem ao commit/PR. Subprocesso encerrado não simula
queda física de energia nem falha real de fsync; Google real e UI instalada exigem
validação separada. Bloqueios advisory não coordenam escritores externos.
