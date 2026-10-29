# Disponibilidade e suspensão de raízes — SYNC-004

## Escopo de aceite da entrega

O aceite atual de SYNC-004 é offline, seguindo a separação adotada em SYNC-001 e
SYNC-003: provedor Google simulado, filesystem e SQLite reais em diretórios
temporários, injeção determinística de falhas, reabertura de estado e repetição.
Não exige conta Google, credenciais nem acesso à internet. A execução opcional de
fixtures GTK também é local e usa perfil isolado.

A bateria real ao final é uma etapa futura fora deste aceite offline. Sua ausência
não bloqueia a entrega local nem transforma simulações em evidência de Google,
desmontagem física, reinício real de processo ou empacotamento instalado.

## Oráculos locais

Fixtures usam diretórios temporários e SQLite reais, sem conta ou rede Google.
Cada suspensão proíbe exclusões, adoção de identidade e nova confirmação de bytes
não instalados. Falha após efeito externo mantém journal para reconciliação.

| Cenário | Fixture/evento e resultado obrigatório | Testes |
| --- | --- | --- |
| N03 / vazio versus ausente | Raiz válida vazia permite inventário vazio; caminho ausente não é recriado; substituto vazio não recebe marcador ou arquivos. Repor a raiz original permite nova verificação. | `coordinator::tests::n03_empty_is_available_but_absent_or_replaced_volume_never_becomes_empty_inventory` |
| N03 / identidade | Retirar/corromper/trocar marcador, trocar cliente ou perder vínculo externo bloqueia acesso sem recriação; reconfigurar associação existente não recria marcador perdido. | `coordinator::tests::missing_corrupt_foreign_and_replaced_markers_do_not_rebind_or_write`; `accounts::tests::configured_root_cannot_be_reinitialized_after_volume_or_marker_loss`; `accounts::tests::missing_external_binding_does_not_recreate_a_saved_pairs_missing_marker` |
| N03 / acesso e concorrência | Revogar acesso de escrita sem modificar conteúdo; segundo ciclo recebe `SyncBusy`; substituição ou revogação durante leitura impede escrever até em staging já aberto. | `coordinator::tests::n03_revoked_local_access_does_not_probe_or_recreate_user_files`; `download::tests::concurrency_tests::n03_*` |
| N04 / C25 | Capacidade de listar deve ser explícita; mover/renomear conservando ID e acesso não troca associação. Lixeira, tipo/ID divergente ou acesso desconhecido impedem trabalho. Escrita exige capacidade no pai efetivo. | `remote::inventory::tests::n04_c25_root_read_and_write_permissions_are_independent_and_unknown_is_blocked` |
| N04 / C25 durante download | Revogar raiz durante leitura interrompe ciclo, preserva conteúdo anterior e fase incompleta; repetir bloqueado não cria novas operações; restaurar acesso permite reconciliação. | `download::tests::concurrency_tests::n04_c25_revoked_root_during_media_stops_without_confirming_or_touching_other_items` |
| N03 / N04 / C25 durante upload | Após primeiro bloco, trocar raiz ou revogar acesso impede publicação final. Restaurar permite retomar o mesmo ID sem duplicação. | `upload::tests::n03_upload_root_replacement_stops_before_final_remote_publication`; `upload::tests::n04_c25_upload_revocation_keeps_intent_and_resumes_same_remote_id_once` |
| E09 / item independente | Ausência de um item é diagnóstico desse item; outro arquivo independente pode concluir, sem inferir exclusão nem apagar conteúdo. | `download::tests::concurrency_tests::e09_missing_remote_item_is_not_root_deletion_and_independent_download_continues` |
| Persistência / reinício | Volume ausente não impede reabrir suspensão. Repetições mantêm vínculo, isolamento e motivo; banco incompatível/corrompido não é resetado; falha de atualização preserva registro anterior. | `coordinator::tests::suspension_survives_absent_volume_restart_and_repetition_without_changing_binding`; `external_registry_corruption_incompatibility_and_failed_updates_preserve_evidence` |
| Lock de associação | Trocar o volume não libera o lock lógico; outra associação pode progredir; symlink no lock é recusado sem alterar alvo. | `coordinator::tests::association_lock_survives_root_replacement_and_isolates_unrelated_work`; `association_lock_refuses_symlinks_without_touching_the_target` |
| Suspensão em preparação/transferência | Motivo de raiz não vira apenas item desconhecido; relatório com falha de transporte não limpa suspensão nem autoriza upload posterior. | `download::tests::reconciliation_tests::root_suspension_during_equivalence_is_fatal_instead_of_an_item_unknown`; `coordinator::tests::transfer_transport_failure_cannot_clear_suspension_or_authorize_upload` |
| Concorrência de configuração | Duas tentativas de vincular IDs de marcador diferentes não podem ambas obter sucesso. | `coordinator::tests::concurrent_binding_cannot_accept_a_different_marker` |
| Scheduler / retomada | Falha transitória usa backoff; limite remoto não é ultrapassado por evento de rede. Pausa permanece; motivo permanente exige tentativa explícita. Nenhum ciclo automático precede restauração de estado; resultado obsoleto não substitui nova configuração. | `automatic::policy::availability_tests`; `automatic::runtime::tests::automatic_workers_retry_preserve_guards_and_reject_obsolete_results` |
| Interface | Estado de raiz suspensa, ação de retry e motivo completo em 360/720 px; não mostrar sucesso global. | `flow::layout_tests::reconciliation_status_layouts` (desktop isolado, execução explícita) |

Os testes de download estão em [concurrency_tests.rs](../../download/concurrency_tests.rs),
os de upload em [tests.rs](../../upload/tests.rs) e os gates em [tests.rs](../tests.rs).
Resultados de execução e comandos pertencem aos commits; esta matriz define
rastreabilidade, não substitui a evidência de cada ambiente.

## Bateria real futura — fora do aceite offline atual

Usar conta/perfil e pastas exclusivamente de teste, com arquivos dispensáveis.
Registrar IDs, hashes locais/remotos, motivo apresentado, baseline/journal antes e
depois e efeito da repetição. Não executar em dados pessoais.

1. **N03:** configurar volume removível; sincronizar conteúdo conhecido; desmontar
   antes de novo ciclo e, separadamente, durante transferência. Incluir desmontagem
   normal, lazy unmount e remontagem no mesmo caminho quando suportados. Confirmar
   bloqueio ou recusa da desmontagem pelo sistema, sem apresentar simulação como
   desmontagem real; verificar diretório subjacente intacto e recuperação com volume
   original. Não interpretar `dev/inode` isoladamente como identificação universal
   de um dispositivo através de reinícios/remontagens.
2. **N04/C25:** numa pasta compartilhada exclusiva, retirar e restaurar permissão
   com um segundo perfil autorizado. Testar antes e durante download/upload;
   confirmar suspensão, arquivos preservados, motivo reapresentado após encerrar e
   abrir o aplicativo e retomada com plano novo. Repetir com raiz movida, renomeada
   e colocada na lixeira; mesma identidade acessível deve continuar reconhecida.
3. **E09:** tornar apenas um arquivo indisponível mantendo raiz e outro arquivo
   acessíveis. Confirmar diagnóstico inconclusivo quando a API não distingue
   remoção de falta de acesso e progresso independente sem exclusões.
4. **Offline/reinício:** retirar rede antes da preparação e durante transferência;
   reiniciar aplicativo, restaurar rede e conferir backoff, reconciliação e ausência
   de duplicações. Repetir com pausa explícita antes de recuperar a conexão.
5. **Desktop/Flatpak:** no aplicativo empacotado, testar concessão/revogação do
   portal, teclado e leitor de tela, motivo completo sem depender de tooltip/cor,
   isolamento de outra conta ativa e persistência após reinício real de processo.

SDK Flatpak, respostas simuladas, reabertura SQLite e screenshots de fixtures não
demonstram estes cenários reais, interrupção de energia nem atomicidade entre uma
verificação de permissão e o efeito seguinte no Google.
