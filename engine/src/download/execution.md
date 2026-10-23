# Download inicial seguro — DOWNLOAD-001/002

Escopo autorizado: um ciclo manual Drive → computador, somente arquivos binários
novos e diretórios. Não implementar upload, exclusões, sobrescrita, changes,
execução automática ou exportação de documentos Google nesta unidade.

## Técnica e política

- Consultar metadados por ID antes/depois do streaming: ID, pai, nome, tipo,
  versão, tamanho, MD5 e permissão precisam corresponder ao inventário.
- Streaming limitado ao tamanho esperado, buffers de 64 KiB; validar tamanho e
  MD5 do provedor. Guardar SHA-256 como evidência local adicional.
- Abrir raiz e componentes por descritores Linux com NOFOLLOW; diretórios
  criados sem substituir entradas existentes. Manter lock exclusivo na raiz.
- Persistir intenção antes de criar staging exclusivo 0600 no mesmo diretório
  do destino. Sincronizar arquivo e diretório antes de registrar Ready.
- Revalidar configuração, identidade da raiz/pai e estado remoto. Instalar com
  renameat2(RENAME_NOREPLACE), sincronizar diretório e só então confirmar Installed.
  Nenhum arquivo final existente pode ser substituído, inclusive em corrida.
- Registrar raiz/pai/staging por device+inode. Ready após interrupção só é
  confirmado se o destino tiver identidade, tamanho e SHA-256 esperados. Se
  apenas staging existir, revalidar e finalizar. Ambiguidade preserva ambos.
- Started interrompido não é base: seus temporários são preservados; uma nova
  tentativa pode baixar novamente. Limpeza automática fica fora do escopo.
- Installed intacto e com metadados remotos iguais permite reconhecer repetição.
  Arquivos preexistentes, alterados ou com nova versão remota permanecem conflitos;
  atualizar arquivos já instalados exige uma unidade posterior.
- Erros por item não impedem itens independentes; corrupção/erro no journal,
  mudança de configuração e cancelamento suspendem o ciclo. Não confirmar
  sucesso global quando houver itens pendentes. Cancelamento é cooperativo;
  leituras HTTP têm prazo finito e o commit já iniciado é reconciliável.

SQLite schema 2 adiciona journal por cliente/conta/raiz remota/local. Como nas
regras do projeto, formatos internos antigos são rejeitados e preservados, sem
migração ou reinicialização automática. Tokens não entram no journal.

## Limites e oráculos

Não há transação distribuída entre Drive, filesystem e SQLite. Metadados remotos
não são snapshot; a verificação posterior detecta divergências observadas. Uma
mudança após a última consulta será vista no ciclo seguinte. Descritores evitam
redirecionamento por symlink; um processo com a mesma identidade do usuário pode
mover diretórios abertos ou modificar arquivos após a verificação. Não prometer
proteção contra um processo local hostil com acesso equivalente. Falhas de fsync
são erros, não prova de persistência. Filesystems sem NOREPLACE ou sync de diretório
falham sem fallback destrutivo.

Testes antes da integração: streaming nominal e vazio; checksum/tamanho inválido;
rede interrompida, cancelamento e escrita falha; versão remota alterada; destino
criado durante transferência; symlink em ancestrais/destino; repetição e alteração
local; Ready antes/depois de rename; falha ao confirmar journal; isolamento de
conta/raiz e lock concorrente. Testes usam filesystem/SQLite isolados e fonte
simulada. E2E Google/portal e queda real de energia não são inferidos desses testes.

Referências oficiais consultadas em 2026-10-23:
- https://developers.google.com/workspace/drive/api/guides/manage-downloads
- https://developers.google.com/workspace/drive/api/reference/rest/v3/files
- https://docs.rs/rustix/1.1.5/rustix/fs/fn.openat.html
- https://docs.rs/rustix/1.1.5/rustix/fs/fn.renameat_with.html
- https://man7.org/linux/man-pages/man2/rename.2.html
- https://man7.org/linux/man-pages/man2/fsync.2.html
- https://www.sqlite.org/atomiccommit.html
- https://docs.rs/md-5/0.10.6/md5/
