# Download inicial seguro — DOWNLOAD-001/002

Escopo autorizado: um ciclo manual Drive → computador, arquivos binários novos,
atualizações gerenciadas conforme [DOWNLOAD-003](updates.md) e diretórios. Não implementar upload, exclusões, substituição cega, changes,
execução automática ou exportação de documentos Google nesta unidade.

## Técnica e política

- Consultar metadados por ID antes/depois do streaming: ID, pai, nome, tipo,
  versão, tamanho, MD5 e permissão do arquivo precisam corresponder ao inventário.
  Para pastas ancestrais, validar identidade, pai, nome e tipo; sua versão pode
  mudar por outros metadados sem alterar a localização do arquivo.
- Streaming limitado ao tamanho esperado, buffers de 64 KiB; validar tamanho e
  MD5 do provedor. Guardar SHA-256 como evidência local adicional.
- Abrir raiz e componentes por descritores Linux com NOFOLLOW; diretórios
  criados sem substituir entradas existentes. Manter lock exclusivo na raiz.
- Persistir intenção antes de criar staging exclusivo 0600 no mesmo diretório
  do destino. Sincronizar arquivo e diretório antes de registrar Ready.
- Revalidar configuração, identidade da raiz/pai e estado remoto. Instalar com
  renameat2(RENAME_NOREPLACE), sincronizar diretório e só então confirmar Installed.
  Instalações nunca sobrescrevem destinos concorrentes. Atualizações preservam a
  base anterior antes da instalação, conforme DOWNLOAD-003.
- Registrar raiz/pai/staging por device+inode. Ready após interrupção só é
  confirmado se o destino tiver identidade, tamanho e SHA-256 esperados. Se
  apenas staging existir, revalidar e finalizar. Ambiguidade preserva ambos.
- Started interrompido não é base: seus temporários são preservados; uma nova
  tentativa pode baixar novamente. Limpeza automática fica fora do escopo.
- Installed intacto e com metadados remotos iguais permite reconhecer repetição.
  Arquivos preexistentes ou alterados localmente permanecem conflitos. Uma versão
  remota mais nova permite atualização com backup e verificação da base.
- Erros por item não impedem itens independentes; corrupção/erro no journal,
  mudança de configuração e cancelamento suspendem o ciclo. Não confirmar
  sucesso global quando houver itens pendentes. Cancelamento é cooperativo;
  leituras HTTP têm prazo finito e o commit já iniciado é reconciliável.

## Base confirmada, entradas e recuperação

O ciclo recebe inventário remoto, plano com escopo e observações, journal,
cancelamento, fonte de metadados/bytes e callback de progresso.
[execution.rs](execution.rs) implementa validação, instalação e replay inicial;
[execution/updates.rs](execution/updates.rs) implementa atualização e restauração;
[filesystem.rs](filesystem.rs), [stream.rs](stream.rs) e [models.rs](models.rs)
implementam operações locais, verificação dos bytes e fases persistidas.
Requisitos em [RF-SYNC-01–05](../../../docs/SOFTWARE_SPECIFICATION.md#47-detecção-de-mudanças-e-sincronização).

| Fase persistida | Significado e decisão na recuperação |
| --- | --- |
| `Started` | Intenção inicial; temporário parcial não fornece B. Preservar staging, podendo repetir transferência. |
| `Ready` | Staging íntegro/sincronizado e identidade registrada, mas instalação não confirmada. Se destino já é a identidade esperada, verificar tamanho/SHA-256 e sincronizar diretório antes de `Installed`. Se só staging existe, verificar evidência, configuração/remoto e instalar sem substituir destino. |
| `Installed` | Evidência durável da instalação inicial; somente revalidação atual autoriza reconhecer repetição ou propor atualização. |
| `Updating` | Intenção de atualizar com evidência anterior; não autoriza mover destino nem fornece B ao planner. |
| `UpdateReady` | Staging novo e base anterior registrados; reconciliar destino, backup e staging conforme [DOWNLOAD-003](updates.md), sem substituir entradas inesperadas. |
| `Updated` | Nova instalação confirmada; fornece a base renovada, independentemente da retenção de backups antigos. |
| `UpdateAbandoned` | Atualização anterior retirada da execução; preservar evidências e replanejar em novo ciclo. Não fornece B ao planner. |

No replay `Ready` com destino já instalado, confirmar a instalação histórica
verificada não equivale a aceitar uma revisão remota nova: se o item do inventário
mudou, o executor relata `RemoteChanged` depois dessa confirmação. Não afirmar
que a base confirmada representa a observação remota mais recente.

O planner não substitui essas verificações. O executor pode recuperar uma fase
pendente mesmo quando o plano não possui B. Destino ocupado sem evidência de
instalação é conflito, inclusive com bytes iguais. Erro de confirmação SQLite
após rename deixa uma fase recuperável; não apagar dados para forçar uma repetição.
A regra de extração de B e o mascaramento da base antiga estão em
[state](../state/algorithms.md).

**Evidência:** a [matriz do eixo 1](../state/preparation.md#rastreabilidade-das-regras-do-eixo-1)
liga cada regra a testes de decisão, persistência, interrupção e isolamento.
Os testes de confirmação usam triggers SQLite que falham depois do efeito no
filesystem; os testes de fronteiras de rename também usam estados reconstruídos.
Isso verifica replay local, não queda real do processo/energia. O relatório pode
trazer `DownloadConflict` sem distinguir os motivos do planner; o aceite da
classificação na interface/diagnóstico continua pendente. A bateria A01–A12 com
Google Drive real permanece pendente.

## Progresso observável

A barra do ciclo mede itens processados (arquivos, diretórios, itens ignorados e
falhas), com denominador fixo após o inventário. Avança após o resultado de cada
item, sem prometer tempo restante ou sucesso de todas as transferências. Bytes
recebidos pertencem somente ao arquivo atual. Recebimento, verificação e instalação
são etapas distintas; receber todos os bytes não confirma a instalação. Antes de
conhecer o inventário, a interface indica atividade sem porcentagem inventada.
Itens não suportados são informação separada dos erros de transferência.
A UI mantém apenas a observação mais recente para evitar exibir outro arquivo
por atraso de mensagens. Referência GNOME consultada em 2026-10-23:
https://developer.gnome.org/hig/patterns/feedback/progress-bars.html

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

## Planos de reconciliação SYNC-003

`DeferredUnknown` nunca chega à transferência: o executor mantém o item pendente,
preservando os motivos de revisão inválida/regressiva e identidade incompleta.
Uma reversão local posterior à preparação não converte o plano desconhecido em
autorização para instalar. `Equivalent` é um caminho sem efeitos: revalida B,
vínculo de caminho/identidade, cadeia remota e SHA-256 de ambas as versões;
não cria staging, cópia, instalação nem nova B. Detalhes e oráculos no
[contrato de reconciliação](../changes/reconciliation.md).
