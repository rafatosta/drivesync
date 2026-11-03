# SYNC-020 — proteção de arquivos locais

Contrato e política: [especificação](../../../docs/SOFTWARE_SPECIFICATION.md#sync-020--links-arquivos-especiais-temporários-e-locks).

## Algoritmo e erros

1. Validar a raiz autorizada e abrir cada ancestral por descritor com NOFOLLOW.
2. Classificar por stat sem seguir links. Múltiplos hard links e tipos especiais
   são Unsupported; descendentes permanecem UNKNOWN.
3. Abrir regular com NOFOLLOW/NONBLOCK e exigir nlink=1. Observações e downloads
   tentam flock compartilhado não bloqueante: contenção vira LocalWriting.
   Somente ENOSYS/EOPNOTSUPP permitem leitura otimista, com assinatura, hash,
   identidade e cadeia de descritores revalidados. Outros erros permanecem erros.
   Comparação e execução de decisões também admitem essa ausência de capacidade,
   com o protocolo de preservação e revalidação descrito abaixo.
4. Comparar assinatura após quiescência, calcular hash e revalidar assinatura,
   pai e raiz. Trocas ou alterações adiam; não reutilizar observação após reinício.
5. Upload retém o descritor da origem durante os efeitos remotos, tenta flock
   quando suportado e cria snapshot
   privado e revalida fonte em cada fronteira de chunk. Falha remove somente
   snapshot parcial próprio; intent já persistido permanece para reconciliação.
6. Instalação remota recusa nomes temporários no planner e mantém staging,
   hash, journal e rename NOREPLACE existentes. Recuperação revalida os descritores.

Não há alteração de schema. Operações adiadas são repetíveis sem apagar fonte,
base ou intenção. Um alias criado durante a leitura invalida a assinatura;
a proteção não promete exclusão mútua contra alterações hostis do mesmo UID.

## Seleção de uma raiz nova no Flatpak

A configuração resolve a origem informada pelo document portal. Se não existe
`.drivesync` em nenhuma das vistas e a origem já pode ser aberta pelo sandbox
com descritores NOFOLLOW, usa o caminho direto antes de registrar identidades.
Isso não concede permissões nem altera uma associação salva. Havendo estado,
mantém a vista selecionada: identidades de portal e host não são intercambiáveis.
Um arquivo temporário exclusivo verifica escrita e tenta flock durante a
configuração; sua ausência não bloqueia download. A sondagem própria é removida.
Não adicionar acesso geral ao home como solução. Downloads usam a mesma vista
mesmo com journal existente; nenhuma identidade ou base é migrada ou regravada
para satisfazer a verificação. O teste opt-in de seleção do portal verifica essa
continuidade e a preservação das evidências.

## Transferências e decisões sem flock de arquivo regular

O lock de raiz e o lock privado por conta continuam exclusivos e obrigatórios.
Leituras de transferências verificadas, observação e captura de base usam a
política otimista. Upload de itens novos exige autorização por conta, snapshot
privado e hashes da origem antes de enviar o bloco final. Cada bloco verifica
a assinatura do snapshot antes do envio. Decisões de conflito exigem ainda o
consentimento aplicável, snapshots verificados de ambas as versões, revalidação
da origem e escrita remota condicional; a ausência de flock nunca autoriza uma
escolha ou uma sobrescrita por si só.
A instalação mantém staging exclusivo, tamanho/MD5/SHA-256, fsync e NOREPLACE.
Revalidar staging após consultas remotas e verificar o destino instalado antes
de confirmar Installed/Updated. Falha conserva o journal recuperável e os bytes;
não anunciar instalação confirmada. Atualizações preservam e revalidam a versão
anterior no backup. A política detecta alterações observadas, sem prometer uma
transação com escritores externos não cooperantes.

Fundamentação: [portal 1.22.1](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.1/document-portal/document-portal-fuse.c)
retorna ENOSYS em flock, embora implemente fcntl;
[flock(2)](https://man7.org/linux/man-pages/man2/flock.2.html) é cooperativo;
[rename(2)](https://man7.org/linux/man-pages/man2/rename.2.html) define NOREPLACE.
A adoção dessa política não altera a camada de arquivos nem introduz GIO.

## Oráculos de aceite

| Cenário | Testes |
| --- | --- |
| ENOSYS em arquivos, download inicial/vazio/binário, repetição, reinício, rede, cancelamento, links e lock de raiz | `download::tests::downloads_without_file_flock_keep_integrity_recovery_and_local_preservation` |
| ENOSYS, atualização com backup, divergência, troca de inode e recuperação | `download::tests::update_tests::portal_lock_capability_does_not_prevent_verified_updates_or_recovery` |
| Mutação de staging na instalação não confirma binário corrompido | `download::tests::portal_staging_mutation_at_install_never_confirms_a_corrupt_binary` |
| Leitura estrita continua recusando ENOSYS; tipos especiais continuam recusados | `download::filesystem::capacity_tests::only_verified_reads_allow_unsupported_flock_and_never_special_files` |
| N05: links externos, FIFO, repetição e cópia independente | `local::tests::sync020_n05_aliases_and_special_files_are_never_upload_candidates` |
| N05: symlink/alias durante quiescência | `local::tests::sync020_n05_link_creation_and_symlink_swap_during_wait_fail_closed` |
| N05: ancestral trocado e traversal | `local::tests::sync020_n05_descriptor_chain_refuses_parent_swaps_and_traversal`; `download::tests::symlinks_never_redirect_downloads` |
| E01: lock exclusivo, compartilhado e liberação | `local::tests::sync020_e01_exclusive_flock_defers_and_shared_flock_allows_reading` |
| N05/E01: fontes bloqueadas não impedem upload independente | `upload::tests::sync020_n05_e01_blocked_sources_preserve_bytes_and_allow_independent_upload` |
| N05/E01: falha de rede, journal reaberto, lock/alias/symlink na retomada e mesmo ID | `upload::tests::sync020_n05_e01_locked_replay_keeps_snapshot_and_resumes_same_id_once` |
| N05: alias criado durante snapshot invalida somente staging parcial | `upload::local::tests::sync020_n05_alias_created_during_snapshot_preserves_source_and_removes_partial` |
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


## Resolução e comparação pelo document portal

A leitura regular tenta flock compartilhado; somente ENOSYS/EOPNOTSUPP permitem
prosseguir com descritores sem seguir links. Contenção e demais erros continuam
bloqueando. A comparação verifica assinatura/identidade/hash de L e consulta R
antes/depois. Decisões persistem intenção, preservam L/R, revalidam hashes e a
origem em cada fronteira, usam precondição remota forte e mantêm instalação
NOREPLACE. O replay confere snapshots, arquivo deslocado e resultado antes de
confirmar B. Locks de raiz/conta não têm fallback. Nenhuma vista direta é adotada
e nenhuma permissão ou identidade salva é modificada.

A política é a mesma usada contra escritores não cooperantes em acesso direto:
flock é advisory, não uma transação com o editor. Se um escritor continuar no
inode deslocado, os bytes ficam no backup e a confirmação de B é recusada quando
a alteração é observada. Corrupção de snapshots, aliases e mudanças de raiz,
origem ou revisão continuam erros, com evidências preservadas.

Oráculos: `unsupported_file_locks_preserve_comparison_resolution_and_recovery`
executa escolhas, propostas obsoletas, substituições, conteúdo concorrente,
colisões, links, corrupção, rollback e replay com ENOSYS injetado somente em
arquivos regulares. `supported_file_lock_contention_still_blocks_resolution_without_effects`
conserva a recusa de contenção real. O ensaio opt-in
`real_document_portal_supports_verified_conflict_decisions` exige exportação nova,
confirma flock não suportado no backend real e executa escolhas e recuperação
sem injeção, com remoto simulado. Esses testes não comprovam Google real nem
acessibilidade da interface instalada.

Fundamentação reconsultada em 2026-11-03: o portal 1.22.1 não implementa flock;
[flock(2)](https://man7.org/linux/man-pages/man2/flock.2.html) descreve a cooperação
por descritor. A autorização por intenção/snapshot/precondição é política do
DriveSync, não capacidade fornecida por flock ou pelo portal.
