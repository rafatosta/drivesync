# SYNC-020 — proteção de arquivos locais

Contrato e política: [especificação](../../../docs/SOFTWARE_SPECIFICATION.md#sync-020--links-arquivos-especiais-temporários-e-locks).

## Algoritmo e erros

1. Validar a raiz autorizada e abrir cada ancestral por descritor com NOFOLLOW.
2. Classificar por stat sem seguir links. Múltiplos hard links e tipos especiais
   são Unsupported; descendentes permanecem UNKNOWN.
3. Abrir regular com NOFOLLOW/NONBLOCK, exigir nlink=1 e adquirir flock
   compartilhado não bloqueante. Contenção vira LocalWriting; outros erros de
   acesso/I/O permanecem distintos e não autorizam ausência.
4. Comparar assinatura após quiescência, calcular hash e revalidar assinatura,
   pai e raiz. Trocas ou alterações adiam; não reutilizar observação após reinício.
5. Upload retém o descritor bloqueado durante os efeitos remotos, cria snapshot
   privado e revalida fonte em cada fronteira de chunk. Falha remove somente
   snapshot parcial próprio; intent já persistido permanece para reconciliação.
6. Instalação remota recusa nomes temporários no planner e mantém staging,
   hash, journal e rename NOREPLACE existentes. Recuperação revalida os descritores.

Não há alteração de schema. Operações adiadas são repetíveis sem apagar fonte,
base ou intenção. Um alias criado durante a leitura invalida a assinatura;
a proteção não promete exclusão mútua contra alterações hostis do mesmo UID.

## Oráculos de aceite

| Cenário | Testes |
| --- | --- |
| N05: links externos, FIFO, repetição e cópia independente | `local::tests::sync020_n05_aliases_and_special_files_are_never_upload_candidates` |
| N05: symlink/alias durante quiescência | `local::tests::sync020_n05_link_creation_and_symlink_swap_during_wait_fail_closed` |
| N05: ancestral trocado e traversal | `local::tests::sync020_n05_descriptor_chain_refuses_parent_swaps_and_traversal`; `download::tests::symlinks_never_redirect_downloads` |
| E01: lock exclusivo, compartilhado e liberação | `local::tests::sync020_e01_exclusive_flock_defers_and_shared_flock_allows_reading` |
| N10: temporários remotos não instalados | `download::tests::sync020_n10_remote_temporary_names_never_install_or_request_media` |
| N10: temporário local e renomeação definitiva | `upload::tests::temporary_uploads_are_deferred_and_final_rename_uploads_once` |
| N10: escrita/substituição durante snapshot | `upload::local::tests::n10_write_or_replacement_during_snapshot_discards_only_partial_staging` |
| N10: escritor retomado entre chunks | `upload::tests::n10_resumed_writer_stops_before_final_upload_publication_and_keeps_intent` |

SQLite e remoto não são necessários nos testes de classificação/descritores.
Os testes de upload/download usam journals SQLite reabertos e remoto simulado;
não há acesso a conta Google. Reinício significa reabertura dos objetos duráveis,
não kill de processo. Nenhum cenário deste escopo exige Google Drive real ou UI.

## Fundamentação

Consultada em 2026-11-01:
- [rustix openat](https://docs.rs/rustix/1.1.5/rustix/fs/fn.openat.html):
  abertura relativa com flags NOFOLLOW/NONBLOCK, sem canonicalização de filhos.
- [rustix flock](https://docs.rs/rustix/1.1.5/rustix/fs/fn.flock.html):
  lock advisory vinculado ao descritor; compartilhado para leitores cooperativos.
- [Linux flock(2)](https://man7.org/linux/man-pages/man2/flock.2.html):
  duração por open file description e ausência de coordenação geral com fcntl.
