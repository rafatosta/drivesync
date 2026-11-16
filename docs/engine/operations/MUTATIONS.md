# Execução completa do Operator

O especialista Google implementa criação de arquivo e diretório, upload, download,
atualização de conteúdo, renomeação, movimentação, lixeira e restauração. `delete`
recusa exclusão permanente antes de qualquer I/O. Os métodos não escolhem políticas,
resolvem conflitos, confirmam baseline ou iniciam uma fila própria.

## Conteúdo e criação

`create_file` aceita conteúdo ausente (arquivo vazio), `Bytes`, `LocalFile` e
`RemoteFile`, para destino local ou remoto dentro do escopo. Conteúdo é materializado
em staging privado, com cópia/stream delimitado, SHA-256 e checksums remotos quando
existirem. Recursos externos exigem a `SourceEvidence` exata. Nenhuma criação
substitui um homônimo. Arquivos nativos Google exigem operação de exportação/conversão
própria; atualização binária não finge suportar esse formato.

Upload/download continuam representando criação. `update_content` usa o alvo
existente explicitamente identificado. Atualização remota conserva seu ID e exige
versão esperada. Criação e atualização reaproveitam o protocolo resumable, com
checagem do offset pelo servidor, ID estável e recuperação de sessão expirada.
Queries e blocos de substituição levam `If-Match`, inclusive com corpo vazio.

Atualização local exige `local_id` e `expected_revision = local_revision(metadata)`.
O fingerprint contém somente metadados comuns, sem vazar `SyncItem` para Operator.
O executor reobserva o alvo, prepara conteúdo e usa `RENAME_EXCHANGE`: os bytes
anteriores ficam no arquivo privado `.drivesync/transfers/<id>.part`, sem unlink.
A identidade física resultante é observada e retornada, sem copiar o inode antigo.
Modo de acesso é conservado; não prometer conservação de ACLs/xattrs não coletados.
O alvo deve ser arquivo regular, de único hardlink e pertencente ao usuário atual.

Linux não oferece CAS de rename por inode. O executor revalida identidade e versão
imediatamente antes da troca e verifica o inode deslocado depois. Se detectar uma
corrida, conserva os dois conteúdos e retorna efeito indeterminado/aplicado; nunca
faz rollback que possa sobrescrever outra alteração nem declara `Verified` falso.

## Renomeação e movimentação

`ExecutionContext.source` contém o snapshot do próprio recurso que será alterado,
com ID físico local ou ID/versão remotos. Preconditions locais usam
`local_revision`; remotas usam `version` e revisão explícita quando fornecida.
Nenhuma política associa itens por nome ou hash.

Renomeação/movimentação local usa descritores sem seguir symlinks e rename sem
substituição. A identidade é verificada no destino. Diretórios não podem ser movidos
para descendentes. Mudança de filesystem não faz fallback por cópia/remoção.
Diretórios pais são presos por identidade; reabertura confere os caminhos atuais.

Mutações remotas leem metadados e ETag forte da mesma resposta e enviam PATCH
condicional. Revalidam conta, árvore de origem/destino, capacidades, versão,
colisão e ciclos. ETag ausente/fraco bloqueia com `UnsupportedOperation`; não
converter `version` em ETag nem repetir PATCH sem condição. Um servidor que ignore
condições não oferece a garantia necessária; testes HTTP não comprovam esse suporte
em Google real. A resposta é seguida de leitura por ID e conferência do resultado.

## Lixeira e restauração

A lixeira local é **gerenciada pelo aplicativo**, dentro da raiz protegida:
`.drivesync/trash/<execution-id>/item`. Não é a lixeira desktop/Freedesktop.
O manifesto privado conserva snapshot e caminho de origem, além da identidade do
pai. O recurso é movido atomicamente sem remoção definitiva. O resultado contém
a referência real na lixeira, usada por `restore`; não inventar a referência antiga.

Restauração sem destino usa a origem do manifesto e exige o mesmo pai físico;
com destino explícito, preserva o nome original. Destino ocupado bloqueia.
Somente `restore` pode receber uma referência nesse endereço interno, e somente
`trash` pode produzir esse endereço, vinculado ao ID do próprio checkpoint.
Referências privadas continuam proibidas para os demais comandos e para planos
genéricos do Analyzer. A lixeira fica no mesmo filesystem; submount incompatível
ou lixeira indisponível bloqueia. Não há esvaziamento permanente.

Lixeira remota usa `trashed=true`; restauração usa `trashed=false`, com destino
opcional. Ambas conservam o ID e exigem condição e capacidades verificadas.
A raiz configurada não pode ser renomeada, movida ou enviada à lixeira.

## Recuperação, cancelamento e limites

Persistir `Executing/Unknown` antes do efeito. Resposta perdida é reconciliada por
ID e pós-condições, nunca por repetir cegamente a chamada. `Completed` não recria
recurso movido, editado ou removido posteriormente pelo usuário. Confirmação no banco
que falhe depois do efeito retorna `Applied`; uma nova chamada conserva o ID e
verifica o efeito existente. Cancelamento é cooperativo e não promete desfazer nada.

Staging e versões anteriores permanecem recuperáveis. Não há limpeza automática,
portanto o consumo de espaço requer política futura explícita. O esquema SQLite
existente é preservado; contextos e resultados novos usam os contratos existentes.

O Analyzer chama Operator automaticamente para novas réplicas, conforme
[EXECUTION.md](../analyzer/EXECUTION.md). Disponibilizar outras ações no Operator não
autoriza executá-las automaticamente nem resolver conflitos ou descartar dados.

## Validação

Filesystem temporário real, Inventory SQLite e Provider real com HTTP loopback:
criação com quatro modalidades de conteúdo, substituição em ambos os ambientes,
resposta perdida, sessões expiradas, preservação de versões locais anteriores,
renomeação/movimentação, ciclos, lixeira/restauração de arquivos e diretórios,
colisões, evidência obsoleta, condição rejeitada e falha de confirmação após efeito.
Os testes anteriores de interrupção de processo e transferências permanecem ativos.
Aceitação com Google real e instalação do Flatpak são verificações separadas.

## Resultado da aceitação real de 2026-11-16

O teste explícito `operations::google::live_tests::live_complete_operator_on_new_resources`
usa somente uma raiz temporária privada e recursos remotos criados pelo teste.
Criação/atualização/rename/move/lixeira/restauração locais passaram pela factory real;
criação de pasta e arquivo remotos também ocorreu. O endpoint Drive v3 testado não
retornou ETag forte, portanto a aceitação das mutações remotas **não passou**: elas
continuam bloqueadas antes de PATCH/atualização, sem fallback incondicional.
A pasta sintética `DriveSync Operator acceptance ...` foi preservada nessa falha.

A documentação oficial [v2/v3](https://developers.google.com/workspace/drive/api/guides/v2-to-v3-reference)
mostra `Files.etag` na v2 e sem correspondente na v3. Validar eventual uso dedicado
da v2 exige orientação adicional do usuário sobre a versão do protocolo e prova
de rejeição de versão obsoleta no servidor real, não somente testes locais.
Não afirmar que todas as ações remotas estão habilitadas em produção.
