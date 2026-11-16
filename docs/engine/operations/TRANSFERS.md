# Upload e download de novos arquivos

Implementação: `engine/src/operations/google.rs`, especialistas privados em
`google/{download,upload}.rs`, primitivas locais em `filesystem.rs` e journal no
Inventory. Esta entrega executa transferências de **criação de blobs** nos dois
sentidos. Não substitui conteúdo existente, não exporta documentos Workspace e
não implementa as demais mutações do contrato.

## Composição e chamada

`DefaultOperatorFactory.create(OperatorConfig { connector, scope, journal })`
retorna `Arc<dyn Operator>`. A factory resolve o provedor na conta persistida via
Engine; não aceita credenciais ou provedor arbitrário. `GoogleOperator::new`
implementa o mesmo contrato. As requisições continuam sendo `UploadRequest` e
`DownloadRequest`, com `ExecutionContext` obrigatório e `ExecutionOptions`
opcional para progresso/cancelamento.

O chamador fornece um ID único por criação e conserva **a mesma requisição e o
mesmo ID** no retry. Mudar origem, observação, nome ou destino exige outra decisão,
nunca reaproveitar o ID com outros dados. A porta `ExecutionJournal` deve ser o
Inventory da sincronização, que valida os quatro componentes do escopo.

Chamadas podem ser aguardadas por GLib sem Tokio externo. Um worker próprio
executa filesystem/SQLite e dirige HTTP no runtime de Engine; GTK e a thread de
I/O não fazem esse trabalho bloqueante. O worker retém o proprietário do runtime
e a guarda da conta mesmo se o future do chamador for descartado. Cancelamento
usa o token cooperativo: descartar o future não promete desfazer efeitos.

Accounts obtém/renova a sessão antes de cada requisição e preserva as credenciais
no cofre. Uma guarda de leitura impede desconectar a conta durante a execução.
O login passa a solicitar `openid email profile` e
`https://www.googleapis.com/auth/drive`, necessário para blobs de pastas
preexistentes escolhidas pelo usuário. Contas autorizadas anteriormente apenas
para metadados precisam de novo consentimento; erro de permissão não é sucesso.

## Segurança local comum

- Abrir a raiz absoluta e cada componente por descritores, com `NOFOLLOW`.
  Recusar tipos especiais, hardlinks de arquivos, nomes inválidos e acesso ao
  namespace reservado como origem/destino de usuário.
- Usar `.drivesync/transfers/<ExecutionId>.part` para staging privado, com
  diretórios 0700, arquivo 0600 e acesso sem seguir links. Temporários e snapshots
  são preservados para recuperação; não há limpeza destrutiva de arquivos.
- Serializar execuções do escopo por `flock` não bloqueante em
  `.drivesync/operator.lock`. Outra chamada concorrente recebe
  `CheckpointConflict`; CAS do journal não é usado como lease.
- Persistir e revalidar identidades físicas da raiz, do pai e do diretório de
  staging, incluindo nascimento quando disponível. Troca de raiz/pai/diretório
  não é autorização para continuar no recurso substituído.
- Validar dispositivo/inode, tamanho, mtime/ctime, identidade disponível e
  hashes. Metadados ausentes não viram igualdade de conteúdo ou identidade.

## Download

1. Validar conta, destino local e observação remota exata. Percorrer ancestrais
   remotos por IDs até a raiz configurada; verificar recurso não enviado à lixeira,
   versão, tamanho, MIME de blob, hashes disponíveis e `canDownload`.
2. Recusar destino ocupado, inclusive symlink e arquivo com conteúdo igual.
   Não associar ou adotar homônimos.
3. Baixar em fluxo para staging privado. Exigir tamanho exato e comparar os
   checksums remotos disponíveis (SHA-256 e/ou MD5), calculando SHA-256 local.
   Sem checksum independente, recusar esta transferência com segurança.
4. Revalidar origem e diretórios, sincronizar staging e seu diretório, reler o
   conteúdo e persistir sua identidade/hash antes da publicação.
5. Persistir a possibilidade de efeito e publicar por `renameat2(NOREPLACE)`.
   Destino inserido depois do preflight não é substituído. Sincronizar os dois
   diretórios e verificar identidade física e hash do arquivo final.
6. Persistir `Completed` com identidade/metadados realmente publicados.

Fluxo parcial interrompido permanece privado e **recomeça do byte zero** no retry;
não se anuncia retomada de download por offset. Staging completo já verificado
pode ser publicado sem novo download. Um efeito publicado antes da confirmação
SQLite é recuperado pelo mesmo inode/nascimento e hash, sem adotar outro arquivo.
Um download concluído cujo destino foi depois movido/apagado pelo usuário não é
recriado pelo retry antigo.

Uma revisão explícita usa `get_revision_bytes` somente quando corresponde à
revisão atual observada; versão/tamanho/checksum continuam vinculados à origem.
Não se presume que metadados do head descrevem outra revisão histórica.

## Upload

1. Validar origem local e pasta remota, seus ancestrais e `canAddChildren`.
   Verificar colisão pelo nome sem associar recursos preexistentes.
2. Criar snapshot privado estável, calcular SHA-256 e verificar origem e snapshot.
   Persistir staging e reservar **um único ID remoto** via `files_generate_ids`
   antes de qualquer criação. `GoogleFileWrite.id` transmite esse ID.
3. Persistir `Executing/Unknown`, iniciar sessão resumable e persistir sua URI
   privada antes de enviar blocos. A URI é validada pelo Provider em cada uso;
   não há HTTP direto no Operador nem token OAuth no journal.
4. Consultar o offset confirmado pelo servidor, inclusive no retry; não inferir
   recebimento a partir dos bytes enviados. Enviar blocos de até 1 MiB, respeitando
   o alinhamento do protocolo. Recusar ranges regressivos, impossíveis ou sem
   avanço; revalidar origem/snapshot antes dos blocos e antes da conclusão.
5. Se a sessão expirou, consultar o ID reservado. Se já houver recurso, verificar
   seu conteúdo/identidade. Caso não seja encontrado, usar a operação explícita
   `reset_expired_upload` para retirar apenas a sessão antiga e iniciar outra
   **com o mesmo ID**, conservando snapshot e efeito incerto. Um 404 não é prova
   geral de ausência; o ID nunca é substituído para contornar perda de acesso ou
   uma criação cujo resultado se perdeu.
6. Só concluir com resposta final e nova consulta por ID, verificando nome, pai,
   tamanho e checksums contra o snapshot. `308` e progresso não são conclusão.

Resposta perdida após aceitar bloco ou concluir arquivo não inicia outra
identidade. Sessão inicial cuja resposta se perdeu pode ser refeita para o mesmo
ID; o retry de criação de ID já usado é recusado pelo serviço, não produz outro
arquivo. Um arquivo já concluído ou detectado por ID que foi renomeado/movido pelo
usuário não é recriado como substituto.

Não há loop de retry de rede: erros retornam ao chamador com checkpoint intacto.
O chamador pode repetir explicitamente a requisição. A recuperação de sessão
expirada dentro de uma chamada é limitada; respostas sem avanço interrompem a
execução. Não existe fila privada nem reenvio infinito em background.

## Resultados e cancelamento

Resultados contêm recurso observado, metadados, integridade e bytes da chamada.
Um retry que só recuperou o resultado tem `bytes_transferred=0`. Offset cumulativo
confirmado e bytes enviados/recebidos nesta chamada são conceitos distintos.
Fases e progresso são enviados somente como observados; reporter ausente ou que
retorna `Dropped` não altera o efeito.

Cancelar antes de publicar download conserva o destino; depois de iniciar o
cutover, verificar o efeito mesmo se houver novo pedido de cancelamento.
Upload interrompido pode ter efeito remoto incerto e conserva ID/sessão/snapshot.
Falha de gravação após efeito verificado retorna `Applied`, com recurso para
recuperação, sem declarar sincronização concluída. Incerteza nunca vira rollback
prometido ou autorização para gerar outro ID.

## Limites e integração

- A aplicação integra o Analyzer à execução de novas réplicas com reobservação e
  guardas atuais; ver [EXECUTION.md](../analyzer/EXECUTION.md). Operator não confirma
  baseline nem escolhe a próxima operação.
- Upload, download e criação de diretórios estão implementados. Os outros métodos retornam `UnsupportedOperation`; `delete`
  emite aviso estático e retorna `PermanentDeletionProhibited/NotApplied`, sem
  journal, autenticação ou HTTP.
- Documentos nativos Workspace/conversões, atualização de conteúdo, criação de
  lixeira e restauração permanecem fora desta entrega. Pais são criados como
  operações separadas, verificadas antes de executar seus filhos.
- Publicação precisa do suporte Linux a rename sem substituição e de staging e
  destino no mesmo filesystem. Submount diferente, falta de locks ou capacidade
  indisponível falham explicitamente; não há fallback que sobrescreva ou copie sobre
  um destino existente.
- Snapshots privados permanecem no disco após upload; recuperação/retirada segura
  e política de espaço devem considerar esses artefatos antes de limpeza futura.

## Evidência de validação

A suíte usa o Operador e o Provider reais, arquivos/diretórios temporários,
Inventory SQLite real e HTTP loopback com estado. Cobre conteúdo vazio/binário e
em múltiplos blocos, SHA-256/MD5, resposta truncada, offline, cancelamento, origem
alterada, destino inserido durante publicação, sessão expirada e respostas inicial
ou final perdidas. O teste da factory passa por Engine/Accounts com cofre sintético.

Subprocessos são encerrados sem unwinding antes/depois da publicação e depois de
um bloco de upload; o processo pai recupera o mesmo checkpoint e ID sem duplicar
o efeito. Isso comprova recuperação desses pontos de interrupção de processo,
**não queda de energia, Google real ou Flatpak instalado**.

Referências técnicas: [upload e retomada](https://developers.google.com/workspace/drive/api/guides/manage-uploads),
[download de blobs](https://developers.google.com/workspace/drive/api/guides/manage-downloads).
