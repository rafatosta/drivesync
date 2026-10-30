# SYNC-003 — reconciliação de observações

Requisitos e políticas: [especificação](../../../docs/SOFTWARE_SPECIFICATION.md#sync-003--inventário-e-reconciliação).

## Contrato

Entradas: inventário local completo com assinatura/fingerprint, inventário remoto
no escopo da conta/raiz, B consultada no journal e caminhos históricos observados.
Saída: plano e classificações ordenadas por caminho/ID. Classificar não instala,
publica, exclui nem confirma B. Leituras remotas para comprovar equivalência são
permitidas e consomem banda; não são operações de instalação.

O planner decide primeiro por identidade e B. Mudanças concorrentes e combinações
de localização/conteúdo do mesmo objeto com B podem ser refinadas por prova: metadados exatos
antes/depois, tamanho/MD5 do provedor, SHA-256 dos bytes recebidos em sink e
comparação com SHA-256 local. Nenhum hash adota uma identidade diferente.
Sem prova, o item fica desconhecido. Configuração alterada, cancelamento ou erro
de estado interrompem a preparação; falhas de leitura de conteúdo adiam o item.
O executor revalida equivalência novamente e não avança B nesse caminho.

O inventário local descobre arquivos novos independentemente de eventos. Paths
históricos não representados pelo grafo atual são indisponibilidade remota; nunca
se tornam autorização para upload. Desaparecimento local não pede exclusão remota.
O registro persistido conserva B/L/R/plano e diagnósticos anteriores à execução;
o relatório não declara que propostas foram executadas. Repetir as mesmas
evidências produz as mesmas classificações; gravações podem gerar nova revisão
de observação sem duplicar operações ou pendências.

Persistência e recuperação: [state/preparation.md](../state/preparation.md#sync-003--observações-de-reconciliação).
Falhas entre checkpoint, plano e registro de reconciliação impedem a execução;
o próximo ciclo reaproveita o feed e observa o local novamente. Não se promete
transação única entre bancos, filesystem e Google.

## Oráculos e rastreabilidade

| Cenário | Testes e efeito esperado |
| --- | --- |
| C06: local, remoto e concorrente depois de offline | `c06_reopened_offline_local_remote_and_divergent_changes_preserve_bases` em [reconciliation_tests.rs](../download/reconciliation_tests.rs): classificação em relação a B reaberta e preservação de versões divergentes. |
| C21: bytes equivalentes em revisões concorrentes | `c21_equivalent_concurrent_bytes_are_read_only_and_idempotent_after_reopen`: equivalência sem cópias, escrita ou avanço de B. |
| C21: timestamps e identidade | `c21_timestamp_only_changes_do_not_create_conflicts_or_adopt_replacements`: timestamp isolado não decide versão; inode diferente ou B ausente não autoriza adoção. |
| Prova inconclusiva e concorrência | `equivalent_plan_revalidates_remote_and_local_before_reporting_success`, `incomplete_remote_equivalence_proof_is_unknown_without_media_installation`, `cancellation_during_equivalence_proof_never_publishes_a_plan` e `unknown_remote_proof_never_authorizes_download_after_local_reversion`. |
| N10: temporários e quiescência | [local/tests.rs](../local/tests.rs): eventos ausentes, padrões explícitos, edição/substituição durante janela, symlinks e permissões. [upload/tests.rs](../upload/tests.rs): `temporary_uploads_are_deferred_and_final_rename_uploads_once` e `temporary_item_does_not_stop_independent_ready_uploads`. |
| N10: snapshot e escritor retomado | `n10_write_or_replacement_during_snapshot_discards_only_partial_staging` em [upload/local.rs](../upload/local.rs); `n10_resumed_writer_stops_before_final_upload_publication_and_keeps_intent` em [upload/tests.rs](../upload/tests.rs). |
| C06/C21/N10 integrados, falha e repetição | `c06_c21_n10_reconnect_after_observation_commit_failure_and_repeat_without_new_events` em [sync003_acceptance.rs](../download/sync003_acceptance.rs): feed, filesystem, journals e observações reais; falha de transação não avança B, replay não exige novo evento, indisponibilidade não exclui nem vira item novo. |
| Estado e isolamento | [state/reconciliation/tests.rs](../state/reconciliation/tests.rs): reabertura, CAS, rollback, cancelamento, formatos inválidos, isolamento e ausência distinta de ancestral inacessível. |
| Interface e agendamento | `reconciliation_status_layouts` em [layout_tests.rs](../../../src/flow/layout_tests.rs) usa GTK nativo com perfil isolado; `unstable_and_unknown_items_keep_periodic_checks_enabled` em [policy.rs](../../../src/automatic/policy.rs) verifica novo ciclo para pendências. |

Essa matriz aponta oráculos locais. Resultados/comandos pertencem aos commits;
não demonstra Google Drive real, reinício real de processo, falha de energia,
portal/mount do aplicativo instalado nem atomicidade contra escritores externos.
A quiescência não prova fechamento de arquivo. Tipos Workspace não recebem prova
universal de equivalência por hash; atualização remota, exclusões e resolução
assistida permanecem fora deste contrato.

## Classificação central SYNC-006

As regras são compartilhadas por `conflicts`; este módulo coleta as evidências.
[locations.rs](locations.rs) compara a localização instalada de B com o inventário
completo e os nomes/pais remotos. O candidato `Classified` registra decisão e
caminho local, preserva B e não é executável. O diagnóstico conciliável não é
conflito nem declaração de sincronização concluída. Caminho associado não aparece
também como arquivo local novo. Limites de identidade, execução e oráculos estão
no [contrato central](../conflicts/contract.md) e na
[matriz SYNC-006](../conflicts/algorithms.md#classificação-sync-006).
