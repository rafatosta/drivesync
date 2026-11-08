# Google — TESTS

> **Escopo:** testes técnicos do mapeamento direto da API Google; não separam um módulo `api/` nem testam gerenciamento de transferências da aplicação.

## 1. Objetivo e limites

Este documento especifica **os testes a escrever antes de implementar** o módulo `providers/google/` (Contract-First + TDD). O objetivo é demonstrar, **sem acessar Google real**, que cada interface envia a requisição correta e interpreta corretamente respostas, tipos, erros e estados parciais.

- **Fonte de verdade:** [INTERFACES.md](INTERFACES.md), [README.md](README.md) e referência oficial [Drive API v3](https://developers.google.com/workspace/drive/api/reference/rest/v3) / [Discovery](https://www.googleapis.com/discovery/v1/apis/drive/v3/rest), [OAuth](https://developers.google.com/identity/protocols/oauth2/native-app) e [OIDC](https://developers.google.com/identity/openid-connect/reference).
- **Cobertura obrigatória:** todos os **59 métodos REST**, os **19 métodos complementares de OAuth/OIDC/transferências/conveniências** e todos os tipos públicos declarados em `INTERFACES.md`.
- **Não executar chamadas reais**, cadastrar conta, acessar arquivos pessoais, modificar dados remotos ou exigir credenciais. Não escrever testes E2E nesta etapa.
- **Não alterar contratos externos ao escopo desta API** apenas para habilitar testes; verificar isoladamente a implementação de cada operação Google.
- **Cuidado com a validade:** uma simulação só confirma que a implementação respeita o **contrato configurado**, não prova que o Google real responderá assim. Fixtures, URLs e tipos devem ser derivados da especificação oficial, **não** do código que está sendo testado.

### Natureza dos testes

| Camada | Verifica | Técnica |
| --- | --- | --- |
| Unidade | Formatação e validação de IDs, parâmetros, caminhos, field masks, corpos, mapeamento de erro, modelos e serialização | Funções puras + fixtures |
| Contrato HTTP em isolamento | Método, host, path, query, headers e body enviados; status e payload recebidos | Cliente real apontado a **servidor HTTP local simulado** |
| Tipos/compilação | Exposição das assinaturas, tipos públicos, campos e erros | Testes de compilação e exemplos compiláveis |
| Testes negativos | Entradas inválidas, campos omitidos, respostas incompletas, falhas e ausência de vazamento | Fixtures negativas e servidor simulado |

Os testes HTTP em servidor simulado são de **integração local do módulo**, embora não sejam integração com o serviço Google. Um *mock* que implemente o mesmo trait, sem executar o cliente HTTP, **não valida** a montagem de requisições.

## 2. Organização do TDD

| Fase | Entrega | Condição para avançar |
| --- | --- | --- |
| RED | Criar teste de compilação + testes do contrato usando fixtures independentes | Falha pelo comportamento ausente; não por especificação equivocada |
| GREEN | Implementar o mínimo que atende ao teste | Casos positivos, negativos e de erro do grupo passam |
| REFACTOR | Simplificar lógica e manter a interface | Todos os testes anteriores continuam passando |
| Próximo grupo | Repetir para a próxima operação e tipo | Commit da unidade lógica validada; não acumular tudo no final |

Os testes são escritos **antes** da implementação. Não relaxar assertivas para obter sucesso; corrigir divergência normativa na documentação e nos testes antes de editar a implementação.

## 3. Matriz obrigatória para cada método

Para **cada método listado nas seções 5 e 6**, criar casos independentes que verifiquem:

| ID | Teste | Resultado esperado |
| --- | --- | --- |
| M01 | Compila e é invocável usando assinatura pública | Argumentos, retorno e erro correspondem ao `INTERFACES.md` |
| M02 | Requisição válida com parâmetros mínimos | Método HTTP, host e caminho exatamente esperados; nenhum parâmetro ou body indevido |
| M03 | Todas as opções documentadas, uma por vez | Nomes de query, headers, encoding, máscara `fields` e valores corretos |
| M04 | Combinação de opções e argumentos | Entradas não se sobrescrevem ou se misturam; opcionais ausentes não são transmitidos |
| M05 | Corpo correto quando há escrita | JSON/bytes compatíveis, MIME e headers; apenas campos alterados em PATCH |
| M06 | Resposta de sucesso mínima | Desserialização para o tipo público e preservação dos campos recebidos |
| M07 | Resposta de sucesso rica/aninhada | Tipos, listas, propriedades, int64 como string, opcionais e estruturas aninhadas |
| M08 | Entrada inválida/inconsistente | Erro tipado local **se o contrato exigir validação local**; senão testar erro HTTP simulado, sem inventar validação |
| M09 | Erro remoto com status e `reason` específico | `GoogleApiFailure` correto, sem interpretar apenas o status |
| M10 | Falha de rede, timeout e payload inválido | Falha tipada; nenhuma resposta parcial anunciada como sucesso |
| M11 | Segurança e isolamento de sessão | Token da conta correta, sem segredos nos erros/logs e sem acesso cruzado |
| M12 | Idempotência, retry e cancelamento quando aplicáveis | Não duplicar efeitos nem afirmar sucesso sem confirmação |

**Aplicabilidade:** M05 é obrigatório apenas nos métodos com body; paginação, token, streaming, upload e operação longa têm cenários adicionais abaixo. Para métodos cujo retorno é `()`, verificar o **status de sucesso oficial** e resposta vazia apropriada. Não exigir presença de campos que o Google documenta como opcionais.

## 4. Assertivas HTTP e fixtures independentes

- O endereço base oficial é `https://www.googleapis.com/drive/v3`, com base de upload `https://www.googleapis.com/upload/drive/v3`. A implementação poderá usar injeção do endpoint **somente nos testes**, sem permitir que URLs arbitrárias de usuário sejam aceitas como destino de produção.
- Em teste HTTP local, o servidor captura requisições e as compara contra **valores esperados fixados pela referência oficial**: host lógico, verbo, path, query, headers, JSON e bytes. Não gerar expectativa chamando o próprio builder da implementação.
- Verificar percent-encoding de identificadores, espaços, Unicode, parâmetros reservados e expansão correta de `{fileId}`, `{driveId}`, `{revisionId}` etc.
- Testar `fields` com máscara parcial: campo não retornado **não vira valor vazio, falso nem exclusão**.
- Comparar JSON semanticamente onde ordem de chaves é irrelevante; comparar **bytes exatos** em upload/download.
- Respostas simuladas devem incluir sucesso mínimo, sucesso completo, listas vazias, paginação, campos desconhecidos adicionais, JSON ausente/malformado, erro com `reason` e respostas sem body.
- Não enviar `Authorization` a host inesperado nem seguir redirecionamento com vazamento de credenciais.
- Testes devem ser repetíveis, isolados, sem relógio ou rede externos, sem depender da ordem de execução.

## 5. Inventário de métodos REST e testes por endpoint

**Regra para cada linha:** executar a matriz **M01–M12 aplicável**, além dos cenários específicos do recurso na seção 8. Esta tabela é também o checklist de cobertura: nenhuma operação documentada pode ficar sem testes.

| Recurso | Método público | REST oficial a conferir | Retorno | Testes obrigatórios |
| --- | --- | --- | --- | --- |
| about | `about_get` | `GET /about` | `GoogleAbout` | M01–M12 aplicáveis + grupo about |
| accessproposals | `access_proposals_get` | `GET /files/{fileId}/accessproposals/{proposalId}` | `GoogleAccessProposal` | M01–M12 aplicáveis + grupo accessproposals |
| accessproposals | `access_proposals_list` | `GET /files/{fileId}/accessproposals` | `GoogleAccessProposalList` | M01–M12 aplicáveis + grupo accessproposals |
| accessproposals | `access_proposals_resolve` | `POST /files/{fileId}/accessproposals/{proposalId}:resolve` | `()` | M01–M12 aplicáveis + grupo accessproposals |
| approvals | `approvals_approve` | `POST /files/{fileId}/approvals/{approvalId}:approve` | `GoogleApproval` | M01–M12 aplicáveis + grupo approvals |
| approvals | `approvals_cancel` | `POST /files/{fileId}/approvals/{approvalId}:cancel` | `GoogleApproval` | M01–M12 aplicáveis + grupo approvals |
| approvals | `approvals_comment` | `POST /files/{fileId}/approvals/{approvalId}:comment` | `GoogleApproval` | M01–M12 aplicáveis + grupo approvals |
| approvals | `approvals_decline` | `POST /files/{fileId}/approvals/{approvalId}:decline` | `GoogleApproval` | M01–M12 aplicáveis + grupo approvals |
| approvals | `approvals_get` | `GET /files/{fileId}/approvals/{approvalId}` | `GoogleApproval` | M01–M12 aplicáveis + grupo approvals |
| approvals | `approvals_list` | `GET /files/{fileId}/approvals` | `GoogleApprovalList` | M01–M12 aplicáveis + grupo approvals |
| approvals | `approvals_reassign` | `POST /files/{fileId}/approvals/{approvalId}:reassign` | `GoogleApproval` | M01–M12 aplicáveis + grupo approvals |
| approvals | `approvals_start` | `POST /files/{fileId}/approvals:start` | `GoogleApproval` | M01–M12 aplicáveis + grupo approvals |
| apps | `apps_get` | `GET /apps/{appId}` | `GoogleApp` | M01–M12 aplicáveis + grupo apps |
| apps | `apps_list` | `GET /apps` | `GoogleAppList` | M01–M12 aplicáveis + grupo apps |
| changes | `changes_get_start_page_token` | `GET /changes/startPageToken` | `GoogleStartPageToken` | M01–M12 aplicáveis + grupo changes |
| changes | `changes_list` | `GET /changes` | `GoogleChangeList` | M01–M12 aplicáveis + grupo changes |
| changes | `changes_watch` | `POST /changes/watch` | `GoogleChannel` | M01–M12 aplicáveis + grupo changes |
| channels | `channels_stop` | `POST /channels/stop` | `()` | M01–M12 aplicáveis + grupo channels |
| comments | `comments_create` | `POST /files/{fileId}/comments` | `GoogleComment` | M01–M12 aplicáveis + grupo comments |
| comments | `comments_delete` | `DELETE /files/{fileId}/comments/{commentId}` | `()` | M01–M12 aplicáveis + grupo comments |
| comments | `comments_get` | `GET /files/{fileId}/comments/{commentId}` | `GoogleComment` | M01–M12 aplicáveis + grupo comments |
| comments | `comments_list` | `GET /files/{fileId}/comments` | `GoogleCommentList` | M01–M12 aplicáveis + grupo comments |
| comments | `comments_update` | `PATCH /files/{fileId}/comments/{commentId}` | `GoogleComment` | M01–M12 aplicáveis + grupo comments |
| drives | `drives_create` | `POST /drives` | `GoogleDrive` | M01–M12 aplicáveis + grupo drives |
| drives | `drives_delete` | `DELETE /drives/{driveId}` | `()` | M01–M12 aplicáveis + grupo drives |
| drives | `drives_get` | `GET /drives/{driveId}` | `GoogleDrive` | M01–M12 aplicáveis + grupo drives |
| drives | `drives_hide` | `POST /drives/{driveId}/hide` | `GoogleDrive` | M01–M12 aplicáveis + grupo drives |
| drives | `drives_list` | `GET /drives` | `GoogleDriveList` | M01–M12 aplicáveis + grupo drives |
| drives | `drives_unhide` | `POST /drives/{driveId}/unhide` | `GoogleDrive` | M01–M12 aplicáveis + grupo drives |
| drives | `drives_update` | `PATCH /drives/{driveId}` | `GoogleDrive` | M01–M12 aplicáveis + grupo drives |
| files | `files_copy` | `POST /files/{fileId}/copy` | `GoogleFile` | M01–M12 aplicáveis + grupo files |
| files | `files_create` | `POST /files` | `GoogleFile` | M01–M12 aplicáveis + grupo files |
| files | `files_delete` | `DELETE /files/{fileId}` | `()` | M01–M12 aplicáveis + grupo files |
| files | `files_download` | `POST /files/{fileId}/download` | `GoogleOperation` | M01–M12 aplicáveis + grupo files |
| files | `files_empty_trash` | `DELETE /files/trash` | `()` | M01–M12 aplicáveis + grupo files |
| files | `files_export` | `GET /files/{fileId}/export` | `GoogleByteStream` | M01–M12 aplicáveis + grupo files |
| files | `files_generate_cse_token` | `GET /files/generateCseToken` | `GoogleCseToken` | M01–M12 aplicáveis + grupo files |
| files | `files_generate_ids` | `GET /files/generateIds` | `GoogleGeneratedIds` | M01–M12 aplicáveis + grupo files |
| files | `files_get` | `GET /files/{fileId}` | `GoogleFile` | M01–M12 aplicáveis + grupo files |
| files | `files_list` | `GET /files` | `GoogleFileList` | M01–M12 aplicáveis + grupo files |
| files | `files_list_labels` | `GET /files/{fileId}/listLabels` | `GoogleLabelList` | M01–M12 aplicáveis + grupo files |
| files | `files_modify_labels` | `POST /files/{fileId}/modifyLabels` | `GoogleModifyLabelsResponse` | M01–M12 aplicáveis + grupo files |
| files | `files_update` | `PATCH /files/{fileId}` | `GoogleFile` | M01–M12 aplicáveis + grupo files |
| files | `files_watch` | `POST /files/{fileId}/watch` | `GoogleChannel` | M01–M12 aplicáveis + grupo files |
| operations | `operations_get` | `GET /operations/{name}` | `GoogleOperation` | M01–M12 aplicáveis + grupo operations |
| permissions | `permissions_create` | `POST /files/{fileId}/permissions` | `GooglePermission` | M01–M12 aplicáveis + grupo permissions |
| permissions | `permissions_delete` | `DELETE /files/{fileId}/permissions/{permissionId}` | `()` | M01–M12 aplicáveis + grupo permissions |
| permissions | `permissions_get` | `GET /files/{fileId}/permissions/{permissionId}` | `GooglePermission` | M01–M12 aplicáveis + grupo permissions |
| permissions | `permissions_list` | `GET /files/{fileId}/permissions` | `GooglePermissionList` | M01–M12 aplicáveis + grupo permissions |
| permissions | `permissions_update` | `PATCH /files/{fileId}/permissions/{permissionId}` | `GooglePermission` | M01–M12 aplicáveis + grupo permissions |
| replies | `replies_create` | `POST /files/{fileId}/comments/{commentId}/replies` | `GoogleReply` | M01–M12 aplicáveis + grupo replies |
| replies | `replies_delete` | `DELETE /files/{fileId}/comments/{commentId}/replies/{replyId}` | `()` | M01–M12 aplicáveis + grupo replies |
| replies | `replies_get` | `GET /files/{fileId}/comments/{commentId}/replies/{replyId}` | `GoogleReply` | M01–M12 aplicáveis + grupo replies |
| replies | `replies_list` | `GET /files/{fileId}/comments/{commentId}/replies` | `GoogleReplyList` | M01–M12 aplicáveis + grupo replies |
| replies | `replies_update` | `PATCH /files/{fileId}/comments/{commentId}/replies/{replyId}` | `GoogleReply` | M01–M12 aplicáveis + grupo replies |
| revisions | `revisions_delete` | `DELETE /files/{fileId}/revisions/{revisionId}` | `()` | M01–M12 aplicáveis + grupo revisions |
| revisions | `revisions_get` | `GET /files/{fileId}/revisions/{revisionId}` | `GoogleRevision` | M01–M12 aplicáveis + grupo revisions |
| revisions | `revisions_list` | `GET /files/{fileId}/revisions` | `GoogleRevisionList` | M01–M12 aplicáveis + grupo revisions |
| revisions | `revisions_update` | `PATCH /files/{fileId}/revisions/{revisionId}` | `GoogleRevision` | M01–M12 aplicáveis + grupo revisions |

## 6. OAuth/OIDC, transferência e métodos auxiliares

**Regra:** executar a mesma matriz de contrato, adaptando M02–M05 às operações locais (configuração, validação JWT) e às operações sem body. Validar o comportamento próprio descrito na seção 8.

| Grupo | Método público | Alvo ou efeito esperado | Retorno |
| --- | --- | --- | --- |
| OAuth | `load_google_config` | Configuração de fontes e precedência | `GoogleOAuthConfig` |
| OAuth | `authorization_request` | GET accounts.google.com/o/oauth2/v2/auth (URL de navegação) | `GoogleAuthorizationUrl` |
| OAuth | `exchange_authorization_code` | POST oauth2.googleapis.com/token (authorization_code) | `GoogleTokenResponse` |
| OAuth | `refresh_access_token` | POST oauth2.googleapis.com/token (refresh_token) | `GoogleTokenResponse` |
| OAuth | `revoke_token` | POST oauth2.googleapis.com/revoke | `()` |
| OIDC | `get_userinfo` | GET openidconnect.googleapis.com/v1/userinfo | `GoogleUserInfo` |
| OIDC | `validate_id_token` | Validação local de JWT e chaves oficiais | `GoogleIdClaims` |
| OIDC | `get_oidc_discovery` | GET accounts.google.com/.well-known/openid-configuration | `GoogleOidcDiscovery` |
| OIDC | `get_oidc_keys` | GET do jwks_uri informado no Discovery | `GoogleJwkSet` |
| transferência | `get_file_bytes` | GET /files/{fileId}?alt=media | `GoogleDownloadStream` |
| transferência | `get_revision_bytes` | GET /files/{fileId}/revisions/{revisionId}?alt=media | `GoogleDownloadStream` |
| transferência | `export_file_bytes` | GET /files/{fileId}/export?mimeType=… | `GoogleDownloadStream` |
| transferência | `start_resumable_create` | POST /upload/drive/v3/files?uploadType=resumable | `GoogleUploadSession` |
| transferência | `start_resumable_update` | PATCH /upload/drive/v3/files/{fileId}?uploadType=resumable | `GoogleUploadSession` |
| transferência | `upload_chunk` | PUT URI de sessão + Content-Range | `GoogleUploadProgress` |
| transferência | `query_upload_progress` | PUT URI de sessão + Content-Range bytes */total | `GoogleUploadProgress` |
| conveniência | `trash_file` | PATCH /files/{fileId} (trashed=true) | `GoogleFile` |
| conveniência | `rename_file` | PATCH /files/{fileId} (name) | `GoogleFile` |
| conveniência | `move_file` | PATCH /files/{fileId} (addParents/removeParents) | `GoogleFile` |

## 7. Casos de teste por método público

Esta seção é a **descrição executável dos cenários** que complementa as tabelas das seções 5 e 6. Para cada operação há um caso positivo, variantes negativas, interpretação do retorno e asserções HTTP ou locais. A matriz **M01–M12** da seção 3 continua obrigatória quando aplicável. Os tipos, URLs, campos opcionais e regras são os de [INTERFACES.md](INTERFACES.md), sem modificar contratos.

**Montagem do teste:** usar cliente real do módulo contra servidor HTTP simulado e fixtures independentes; para operações locais como construção de URL e validação de JWT, usar entradas determinísticas sem rede. Primeiro conferir a **assinatura pública** e depois o comportamento.

### 7.1 Métodos REST oficiais (59)

#### API-01 — `about_get`

**Contrato testado:** `GET /about`, retorno `GoogleResult<GoogleAbout>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **consulta** do recurso `about`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleAbout` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. simular resposta com `storageQuota`, `exportFormats`, `importFormats` e `user` quando incluídos em `fields`; repetir com cada campo omitido para confirmar que `None` não vira zero ou ausência comprovada. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-02 — `access_proposals_get`

**Contrato testado:** `GET /files/{fileId}/accessproposals/{proposalId}`, retorno `GoogleResult<GoogleAccessProposal>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **consulta** do recurso `accessproposals`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleAccessProposal` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. validar arquivo e proposta por ID; testar proposta não encontrada, listagem vazia/paginada e rejeição de ação de resolução inválida. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-03 — `access_proposals_list`

**Contrato testado:** `GET /files/{fileId}/accessproposals`, retorno `GoogleResult<GoogleAccessProposalList>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **listagem** do recurso `accessproposals`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada. Para listas, incluir página vazia, primeira página com `nextPageToken` e continuação até o fim. 

**Retorno e invariantes:** comparar itens, campos opcionais e token de continuação; lista vazia é sucesso válido. validar arquivo e proposta por ID; testar proposta não encontrada, listagem vazia/paginada e rejeição de ação de resolução inválida. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-04 — `access_proposals_resolve`

**Contrato testado:** `POST /files/{fileId}/accessproposals/{proposalId}:resolve`, retorno `GoogleResult<()>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **resolução** do recurso `accessproposals`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** confirmar o status HTTP de sucesso e a ausência de corpo de recurso, sem tentar desserializar um objeto. validar arquivo e proposta por ID; testar proposta não encontrada, listagem vazia/paginada e rejeição de ação de resolução inválida. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-05 — `approvals_approve`

**Contrato testado:** `POST /files/{fileId}/approvals/{approvalId}:approve`, retorno `GoogleResult<GoogleApproval>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **aprovação** do recurso `approvals`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleApproval` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. preparar fixture para cada ação de aprovação, com corpo específico, estado resultante e falta de permissão; não reutilizar payload incorreto de outra ação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-06 — `approvals_cancel`

**Contrato testado:** `POST /files/{fileId}/approvals/{approvalId}:cancel`, retorno `GoogleResult<GoogleApproval>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **cancelamento** do recurso `approvals`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleApproval` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. preparar fixture para cada ação de aprovação, com corpo específico, estado resultante e falta de permissão; não reutilizar payload incorreto de outra ação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-07 — `approvals_comment`

**Contrato testado:** `POST /files/{fileId}/approvals/{approvalId}:comment`, retorno `GoogleResult<GoogleApproval>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **comentário** do recurso `approvals`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleApproval` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. preparar fixture para cada ação de aprovação, com corpo específico, estado resultante e falta de permissão; não reutilizar payload incorreto de outra ação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-08 — `approvals_decline`

**Contrato testado:** `POST /files/{fileId}/approvals/{approvalId}:decline`, retorno `GoogleResult<GoogleApproval>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **recusa** do recurso `approvals`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleApproval` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. preparar fixture para cada ação de aprovação, com corpo específico, estado resultante e falta de permissão; não reutilizar payload incorreto de outra ação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-09 — `approvals_get`

**Contrato testado:** `GET /files/{fileId}/approvals/{approvalId}`, retorno `GoogleResult<GoogleApproval>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **consulta** do recurso `approvals`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleApproval` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. preparar fixture para cada ação de aprovação, com corpo específico, estado resultante e falta de permissão; não reutilizar payload incorreto de outra ação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-10 — `approvals_list`

**Contrato testado:** `GET /files/{fileId}/approvals`, retorno `GoogleResult<GoogleApprovalList>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **listagem** do recurso `approvals`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada. Para listas, incluir página vazia, primeira página com `nextPageToken` e continuação até o fim. 

**Retorno e invariantes:** comparar itens, campos opcionais e token de continuação; lista vazia é sucesso válido. preparar fixture para cada ação de aprovação, com corpo específico, estado resultante e falta de permissão; não reutilizar payload incorreto de outra ação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-11 — `approvals_reassign`

**Contrato testado:** `POST /files/{fileId}/approvals/{approvalId}:reassign`, retorno `GoogleResult<GoogleApproval>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **reatribuição** do recurso `approvals`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleApproval` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. preparar fixture para cada ação de aprovação, com corpo específico, estado resultante e falta de permissão; não reutilizar payload incorreto de outra ação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-12 — `approvals_start`

**Contrato testado:** `POST /files/{fileId}/approvals:start`, retorno `GoogleResult<GoogleApproval>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **início** do recurso `approvals`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleApproval` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. preparar fixture para cada ação de aprovação, com corpo específico, estado resultante e falta de permissão; não reutilizar payload incorreto de outra ação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-13 — `apps_get`

**Contrato testado:** `GET /apps/{appId}`, retorno `GoogleResult<GoogleApp>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **consulta** do recurso `apps`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleApp` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. testar app ausente, tipos suportados, extensões, MIME e capacidades quando disponíveis, preservando campos omitidos. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-14 — `apps_list`

**Contrato testado:** `GET /apps`, retorno `GoogleResult<GoogleAppList>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **listagem** do recurso `apps`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada. Para listas, incluir página vazia, primeira página com `nextPageToken` e continuação até o fim. 

**Retorno e invariantes:** comparar itens, campos opcionais e token de continuação; lista vazia é sucesso válido. testar app ausente, tipos suportados, extensões, MIME e capacidades quando disponíveis, preservando campos omitidos. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-15 — `changes_get_start_page_token`

**Contrato testado:** `GET /changes/startPageToken`, retorno `GoogleResult<GoogleStartPageToken>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **aquisição do cursor inicial** do recurso `changes`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleStartPageToken` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar `startPageToken`, `nextPageToken`, `newStartPageToken`, remoção e perda de acesso; paginar até o cursor final e simular cursor expirado. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-16 — `changes_list`

**Contrato testado:** `GET /changes`, retorno `GoogleResult<GoogleChangeList>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **listagem** do recurso `changes`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada. Para listas, incluir página vazia, primeira página com `nextPageToken` e continuação até o fim. Confirmar especialmente que `removed=true` pode indicar perda de acesso e não apenas exclusão.

**Retorno e invariantes:** comparar itens, campos opcionais e token de continuação; lista vazia é sucesso válido. exercitar `startPageToken`, `nextPageToken`, `newStartPageToken`, remoção e perda de acesso; paginar até o cursor final e simular cursor expirado. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-17 — `changes_watch`

**Contrato testado:** `POST /changes/watch`, retorno `GoogleResult<GoogleChannel>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **assinatura de notificações** do recurso `changes`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  Confirmar especialmente que abrir canal não significa detectar nem aplicar alterações.

**Retorno e invariantes:** desserializar `GoogleChannel` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar `startPageToken`, `nextPageToken`, `newStartPageToken`, remoção e perda de acesso; paginar até o cursor final e simular cursor expirado. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-18 — `channels_stop`

**Contrato testado:** `POST /channels/stop`, retorno `GoogleResult<()>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **cancelamento de canal** do recurso `channels`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** confirmar o status HTTP de sucesso e a ausência de corpo de recurso, sem tentar desserializar um objeto. verificar `id` e `resourceId`, encerramento do canal e erros de identificação sem inventar notificação entregue. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-19 — `comments_create`

**Contrato testado:** `POST /files/{fileId}/comments`, retorno `GoogleResult<GoogleComment>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **criação** do recurso `comments`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleComment` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. validar conteúdo, `fields`, comentários eliminados, resposta com campos parciais, lista paginada e permissão negada. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-20 — `comments_delete`

**Contrato testado:** `DELETE /files/{fileId}/comments/{commentId}`, retorno `GoogleResult<()>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **exclusão** do recurso `comments`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** confirmar o status HTTP de sucesso e a ausência de corpo de recurso, sem tentar desserializar um objeto. validar conteúdo, `fields`, comentários eliminados, resposta com campos parciais, lista paginada e permissão negada. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-21 — `comments_get`

**Contrato testado:** `GET /files/{fileId}/comments/{commentId}`, retorno `GoogleResult<GoogleComment>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **consulta** do recurso `comments`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleComment` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. validar conteúdo, `fields`, comentários eliminados, resposta com campos parciais, lista paginada e permissão negada. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-22 — `comments_list`

**Contrato testado:** `GET /files/{fileId}/comments`, retorno `GoogleResult<GoogleCommentList>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **listagem** do recurso `comments`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada. Para listas, incluir página vazia, primeira página com `nextPageToken` e continuação até o fim. 

**Retorno e invariantes:** comparar itens, campos opcionais e token de continuação; lista vazia é sucesso válido. validar conteúdo, `fields`, comentários eliminados, resposta com campos parciais, lista paginada e permissão negada. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-23 — `comments_update`

**Contrato testado:** `PATCH /files/{fileId}/comments/{commentId}`, retorno `GoogleResult<GoogleComment>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **atualização** do recurso `comments`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleComment` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. validar conteúdo, `fields`, comentários eliminados, resposta com campos parciais, lista paginada e permissão negada. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-24 — `drives_create`

**Contrato testado:** `POST /drives`, retorno `GoogleResult<GoogleDrive>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **criação** do recurso `drives`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleDrive` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar permissões administrativas, restrições, ocultação, reexibição, exclusão protegida, request ID e paginação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-25 — `drives_delete`

**Contrato testado:** `DELETE /drives/{driveId}`, retorno `GoogleResult<()>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **exclusão** do recurso `drives`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  Confirmar especialmente que exclusão de drive é diferente de ocultação.

**Retorno e invariantes:** confirmar o status HTTP de sucesso e a ausência de corpo de recurso, sem tentar desserializar um objeto. exercitar permissões administrativas, restrições, ocultação, reexibição, exclusão protegida, request ID e paginação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-26 — `drives_get`

**Contrato testado:** `GET /drives/{driveId}`, retorno `GoogleResult<GoogleDrive>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **consulta** do recurso `drives`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleDrive` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar permissões administrativas, restrições, ocultação, reexibição, exclusão protegida, request ID e paginação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-27 — `drives_hide`

**Contrato testado:** `POST /drives/{driveId}/hide`, retorno `GoogleResult<GoogleDrive>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **ocultação** do recurso `drives`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleDrive` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar permissões administrativas, restrições, ocultação, reexibição, exclusão protegida, request ID e paginação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-28 — `drives_list`

**Contrato testado:** `GET /drives`, retorno `GoogleResult<GoogleDriveList>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **listagem** do recurso `drives`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada. Para listas, incluir página vazia, primeira página com `nextPageToken` e continuação até o fim. 

**Retorno e invariantes:** comparar itens, campos opcionais e token de continuação; lista vazia é sucesso válido. exercitar permissões administrativas, restrições, ocultação, reexibição, exclusão protegida, request ID e paginação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-29 — `drives_unhide`

**Contrato testado:** `POST /drives/{driveId}/unhide`, retorno `GoogleResult<GoogleDrive>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **reexibição** do recurso `drives`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleDrive` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar permissões administrativas, restrições, ocultação, reexibição, exclusão protegida, request ID e paginação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-30 — `drives_update`

**Contrato testado:** `PATCH /drives/{driveId}`, retorno `GoogleResult<GoogleDrive>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **atualização** do recurso `drives`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleDrive` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar permissões administrativas, restrições, ocultação, reexibição, exclusão protegida, request ID e paginação. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-31 — `files_copy`

**Contrato testado:** `POST /files/{fileId}/copy`, retorno `GoogleResult<GoogleFile>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **cópia** do recurso `files`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleFile` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar IDs, `q`, `corpora`, `fields`, estruturas de pais, nomes, MIME e opções; distinguir PATCH parcial, renomeação/movimentação de metadados, lixeira e exclusão permanente. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-32 — `files_create`

**Contrato testado:** `POST /files`, retorno `GoogleResult<GoogleFile>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **criação** do recurso `files`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleFile` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar IDs, `q`, `corpora`, `fields`, estruturas de pais, nomes, MIME e opções; distinguir PATCH parcial, renomeação/movimentação de metadados, lixeira e exclusão permanente. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-33 — `files_delete`

**Contrato testado:** `DELETE /files/{fileId}`, retorno `GoogleResult<()>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **exclusão** do recurso `files`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  Confirmar especialmente que exclusão permanente não é equivalente a `trashed=true`.

**Retorno e invariantes:** confirmar o status HTTP de sucesso e a ausência de corpo de recurso, sem tentar desserializar um objeto. exercitar IDs, `q`, `corpora`, `fields`, estruturas de pais, nomes, MIME e opções; distinguir PATCH parcial, renomeação/movimentação de metadados, lixeira e exclusão permanente. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-34 — `files_download`

**Contrato testado:** `POST /files/{fileId}/download`, retorno `GoogleResult<GoogleOperation>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **download assíncrono** do recurso `files`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  Confirmar especialmente que `GoogleOperation` não é um stream de bytes.

**Retorno e invariantes:** validar estado de operação assíncrona e não interpretá-la como download concluído. exercitar IDs, `q`, `corpora`, `fields`, estruturas de pais, nomes, MIME e opções; distinguir PATCH parcial, renomeação/movimentação de metadados, lixeira e exclusão permanente. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-35 — `files_empty_trash`

**Contrato testado:** `DELETE /files/trash`, retorno `GoogleResult<()>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **esvaziamento da lixeira** do recurso `files`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  Confirmar especialmente que esvaziar lixeira pode afetar vários recursos e exige cenário de autorização.

**Retorno e invariantes:** confirmar o status HTTP de sucesso e a ausência de corpo de recurso, sem tentar desserializar um objeto. exercitar IDs, `q`, `corpora`, `fields`, estruturas de pais, nomes, MIME e opções; distinguir PATCH parcial, renomeação/movimentação de metadados, lixeira e exclusão permanente. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-36 — `files_export`

**Contrato testado:** `GET /files/{fileId}/export`, retorno `GoogleResult<GoogleByteStream>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **exportação** do recurso `files`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  Confirmar especialmente que conversão MIME não é download binário direto.

**Retorno e invariantes:** verificar bytes e informações de transferência sem tratar leitura parcial como conclusão. exercitar IDs, `q`, `corpora`, `fields`, estruturas de pais, nomes, MIME e opções; distinguir PATCH parcial, renomeação/movimentação de metadados, lixeira e exclusão permanente. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-37 — `files_generate_cse_token`

**Contrato testado:** `GET /files/generateCseToken`, retorno `GoogleResult<GoogleCseToken>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **obtenção de token CSE** do recurso `files`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleCseToken` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar IDs, `q`, `corpora`, `fields`, estruturas de pais, nomes, MIME e opções; distinguir PATCH parcial, renomeação/movimentação de metadados, lixeira e exclusão permanente. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-38 — `files_generate_ids`

**Contrato testado:** `GET /files/generateIds`, retorno `GoogleResult<GoogleGeneratedIds>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **geração de IDs** do recurso `files`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleGeneratedIds` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar IDs, `q`, `corpora`, `fields`, estruturas de pais, nomes, MIME e opções; distinguir PATCH parcial, renomeação/movimentação de metadados, lixeira e exclusão permanente. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-39 — `files_get`

**Contrato testado:** `GET /files/{fileId}`, retorno `GoogleResult<GoogleFile>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **consulta** do recurso `files`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleFile` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar IDs, `q`, `corpora`, `fields`, estruturas de pais, nomes, MIME e opções; distinguir PATCH parcial, renomeação/movimentação de metadados, lixeira e exclusão permanente. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-40 — `files_list`

**Contrato testado:** `GET /files`, retorno `GoogleResult<GoogleFileList>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **listagem** do recurso `files`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada. Para listas, incluir página vazia, primeira página com `nextPageToken` e continuação até o fim. 

**Retorno e invariantes:** comparar itens, campos opcionais e token de continuação; lista vazia é sucesso válido. exercitar IDs, `q`, `corpora`, `fields`, estruturas de pais, nomes, MIME e opções; distinguir PATCH parcial, renomeação/movimentação de metadados, lixeira e exclusão permanente. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-41 — `files_list_labels`

**Contrato testado:** `GET /files/{fileId}/listLabels`, retorno `GoogleResult<GoogleLabelList>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **listagem de rótulos** do recurso `files`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada. Para listas, incluir página vazia, primeira página com `nextPageToken` e continuação até o fim. 

**Retorno e invariantes:** comparar itens, campos opcionais e token de continuação; lista vazia é sucesso válido. exercitar IDs, `q`, `corpora`, `fields`, estruturas de pais, nomes, MIME e opções; distinguir PATCH parcial, renomeação/movimentação de metadados, lixeira e exclusão permanente. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-42 — `files_modify_labels`

**Contrato testado:** `POST /files/{fileId}/modifyLabels`, retorno `GoogleResult<GoogleModifyLabelsResponse>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **modificação de rótulos** do recurso `files`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleModifyLabelsResponse` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar IDs, `q`, `corpora`, `fields`, estruturas de pais, nomes, MIME e opções; distinguir PATCH parcial, renomeação/movimentação de metadados, lixeira e exclusão permanente. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-43 — `files_update`

**Contrato testado:** `PATCH /files/{fileId}`, retorno `GoogleResult<GoogleFile>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **atualização** do recurso `files`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleFile` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar IDs, `q`, `corpora`, `fields`, estruturas de pais, nomes, MIME e opções; distinguir PATCH parcial, renomeação/movimentação de metadados, lixeira e exclusão permanente. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-44 — `files_watch`

**Contrato testado:** `POST /files/{fileId}/watch`, retorno `GoogleResult<GoogleChannel>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **assinatura de notificações** do recurso `files`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleChannel` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. exercitar IDs, `q`, `corpora`, `fields`, estruturas de pais, nomes, MIME e opções; distinguir PATCH parcial, renomeação/movimentação de metadados, lixeira e exclusão permanente. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-45 — `operations_get`

**Contrato testado:** `GET /operations/{name}`, retorno `GoogleResult<GoogleOperation>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **consulta** do recurso `operations`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** validar estado de operação assíncrona e não interpretá-la como download concluído. testar `done=false` (ainda pendente), `done=true` com `response` ou `error`, e nome de operação inválido. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-46 — `permissions_create`

**Contrato testado:** `POST /files/{fileId}/permissions`, retorno `GoogleResult<GooglePermission>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **criação** do recurso `permissions`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  Confirmar especialmente que notificação e papel concedido devem refletir as opções, sem superestimar permissões.

**Retorno e invariantes:** desserializar `GooglePermission` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. testar tipos `user`, `group`, `domain`, `anyone`, papéis, avisos, propriedade, expiração e erros de permissão. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-47 — `permissions_delete`

**Contrato testado:** `DELETE /files/{fileId}/permissions/{permissionId}`, retorno `GoogleResult<()>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **exclusão** do recurso `permissions`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** confirmar o status HTTP de sucesso e a ausência de corpo de recurso, sem tentar desserializar um objeto. testar tipos `user`, `group`, `domain`, `anyone`, papéis, avisos, propriedade, expiração e erros de permissão. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-48 — `permissions_get`

**Contrato testado:** `GET /files/{fileId}/permissions/{permissionId}`, retorno `GoogleResult<GooglePermission>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **consulta** do recurso `permissions`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GooglePermission` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. testar tipos `user`, `group`, `domain`, `anyone`, papéis, avisos, propriedade, expiração e erros de permissão. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-49 — `permissions_list`

**Contrato testado:** `GET /files/{fileId}/permissions`, retorno `GoogleResult<GooglePermissionList>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **listagem** do recurso `permissions`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada. Para listas, incluir página vazia, primeira página com `nextPageToken` e continuação até o fim. 

**Retorno e invariantes:** comparar itens, campos opcionais e token de continuação; lista vazia é sucesso válido. testar tipos `user`, `group`, `domain`, `anyone`, papéis, avisos, propriedade, expiração e erros de permissão. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-50 — `permissions_update`

**Contrato testado:** `PATCH /files/{fileId}/permissions/{permissionId}`, retorno `GoogleResult<GooglePermission>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **atualização** do recurso `permissions`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  Confirmar especialmente que mudança de papel ou titularidade exige verificar campos do request, não inferir a partir do ID.

**Retorno e invariantes:** desserializar `GooglePermission` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. testar tipos `user`, `group`, `domain`, `anyone`, papéis, avisos, propriedade, expiração e erros de permissão. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-51 — `replies_create`

**Contrato testado:** `POST /files/{fileId}/comments/{commentId}/replies`, retorno `GoogleResult<GoogleReply>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **criação** do recurso `replies`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleReply` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. verificar encadeamento arquivo→comentário→resposta, conteúdo da resposta, ações e listagem paginada. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-52 — `replies_delete`

**Contrato testado:** `DELETE /files/{fileId}/comments/{commentId}/replies/{replyId}`, retorno `GoogleResult<()>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **exclusão** do recurso `replies`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** confirmar o status HTTP de sucesso e a ausência de corpo de recurso, sem tentar desserializar um objeto. verificar encadeamento arquivo→comentário→resposta, conteúdo da resposta, ações e listagem paginada. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-53 — `replies_get`

**Contrato testado:** `GET /files/{fileId}/comments/{commentId}/replies/{replyId}`, retorno `GoogleResult<GoogleReply>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **consulta** do recurso `replies`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleReply` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. verificar encadeamento arquivo→comentário→resposta, conteúdo da resposta, ações e listagem paginada. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-54 — `replies_list`

**Contrato testado:** `GET /files/{fileId}/comments/{commentId}/replies`, retorno `GoogleResult<GoogleReplyList>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **listagem** do recurso `replies`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada. Para listas, incluir página vazia, primeira página com `nextPageToken` e continuação até o fim. 

**Retorno e invariantes:** comparar itens, campos opcionais e token de continuação; lista vazia é sucesso válido. verificar encadeamento arquivo→comentário→resposta, conteúdo da resposta, ações e listagem paginada. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-55 — `replies_update`

**Contrato testado:** `PATCH /files/{fileId}/comments/{commentId}/replies/{replyId}`, retorno `GoogleResult<GoogleReply>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **atualização** do recurso `replies`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleReply` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. verificar encadeamento arquivo→comentário→resposta, conteúdo da resposta, ações e listagem paginada. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-56 — `revisions_delete`

**Contrato testado:** `DELETE /files/{fileId}/revisions/{revisionId}`, retorno `GoogleResult<()>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **exclusão** do recurso `revisions`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** confirmar o status HTTP de sucesso e a ausência de corpo de recurso, sem tentar desserializar um objeto. testar revisão ausente, atualizável/não atualizável, export links e restrições conforme tipo de arquivo. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-57 — `revisions_get`

**Contrato testado:** `GET /files/{fileId}/revisions/{revisionId}`, retorno `GoogleResult<GoogleRevision>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **consulta** do recurso `revisions`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleRevision` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. testar revisão ausente, atualizável/não atualizável, export links e restrições conforme tipo de arquivo. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-58 — `revisions_list`

**Contrato testado:** `GET /files/{fileId}/revisions`, retorno `GoogleResult<GoogleRevisionList>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **listagem** do recurso `revisions`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para consultas sem body, verificar que não é enviado conteúdo indevido. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada. Para listas, incluir página vazia, primeira página com `nextPageToken` e continuação até o fim. 

**Retorno e invariantes:** comparar itens, campos opcionais e token de continuação; lista vazia é sucesso válido. testar revisão ausente, atualizável/não atualizável, export links e restrições conforme tipo de arquivo. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

#### API-59 — `revisions_update`

**Contrato testado:** `PATCH /files/{fileId}/revisions/{revisionId}`, retorno `GoogleResult<GoogleRevision>`, segundo a assinatura e as opções de `INTERFACES.md`.

**Sucesso:** criar sessão fictícia autorizada e simular a **atualização** do recurso `revisions`. Confirmar que o cliente usa exatamente o verbo e o caminho descritos, IDs codificados corretamente e apenas os parâmetros permitidos; para métodos com body, comparar o JSON/corpo exato, distinguindo omissão de limpeza explícita. Validar uma resposta mínima e outra completa conforme a especificação do recurso.

**Negativos e opcionais:** repetir omitindo cada opção facultativa e depois fornecendo-a; campos `fields` ausentes não devem virar valores falsos. Simular acesso negado, recurso inexistente, parâmetros inválidos conforme o contrato, quota/rede/timeout e resposta malformada.  

**Retorno e invariantes:** desserializar `GoogleRevision` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. testar revisão ausente, atualizável/não atualizável, export links e restrições conforme tipo de arquivo. Em todos os casos, conferir `GoogleApiFailure` com status/reason apropriado, isolamento da `GoogleSession` e ausência de segredos em erro ou log; não executar chamadas externas.

### 7.2 OAuth 2.0 e OpenID Connect (9)

#### API-60 — `load_google_config`

**Contrato testado:** `Configuração de fontes e precedência`; retorno `GoogleResult<GoogleOAuthConfig>`. Os parâmetros e tipos permanecem exatamente os de `INTERFACES.md`.

**Sucesso:** Montar fontes de configuração isoladas com precedência conhecida; confirmar escolha do `client_id`, redirects e escopos previstos.

**Negativos e opcionais:** Fonte inválida, precedência conflitante e segredo ausente quando exigido devem resultar em erro ou escolha definida pelo contrato, sem vazar conteúdo. Respostas com campos opcionais ausentes devem preservar a distinção entre não informado e inválido.

**Retorno e invariantes:** desserializar `GoogleOAuthConfig` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. Confirmar resultado, erro tipado, isolamento por sessão/fluxo e que credenciais jamais aparecem em logs ou mensagens. Onde houver endpoint, capturar a requisição no servidor simulado; quando não houver rede, validar a função local sem inventar chamada HTTP.

#### API-61 — `authorization_request`

**Contrato testado:** `GET accounts.google.com/o/oauth2/v2/auth (URL de navegação)`; retorno `GoogleResult<GoogleAuthorizationUrl>`. Os parâmetros e tipos permanecem exatamente os de `INTERFACES.md`.

**Sucesso:** Gerar URL sem executar navegação; comparar host, path e parâmetros OAuth, especialmente `state`, PKCE S256 e redirect URI.

**Negativos e opcionais:** PKCE inválido, `state` ausente quando exigido, redirect URI inadequado e consentimento negado no retorno do fluxo. Respostas com campos opcionais ausentes devem preservar a distinção entre não informado e inválido.

**Retorno e invariantes:** desserializar `GoogleAuthorizationUrl` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. Confirmar resultado, erro tipado, isolamento por sessão/fluxo e que credenciais jamais aparecem em logs ou mensagens. Onde houver endpoint, capturar a requisição no servidor simulado; quando não houver rede, validar a função local sem inventar chamada HTTP.

#### API-62 — `exchange_authorization_code`

**Contrato testado:** `POST oauth2.googleapis.com/token (authorization_code)`; retorno `GoogleResult<GoogleTokenResponse>`. Os parâmetros e tipos permanecem exatamente os de `INTERFACES.md`.

**Sucesso:** Responder no servidor simulado de tokens a uma troca de código e verificar o corpo, PKCE, redirect URI e retorno tipado.

**Negativos e opcionais:** Código expirado/consumido, redirect URI divergente, verifier inválido e `invalid_grant` retornam falha tipada. Respostas com campos opcionais ausentes devem preservar a distinção entre não informado e inválido.

**Retorno e invariantes:** verificar expiração, escopos e ausência legítima de `refresh_token`, com segredos ocultos. Confirmar resultado, erro tipado, isolamento por sessão/fluxo e que credenciais jamais aparecem em logs ou mensagens. Onde houver endpoint, capturar a requisição no servidor simulado; quando não houver rede, validar a função local sem inventar chamada HTTP.

#### API-63 — `refresh_access_token`

**Contrato testado:** `POST oauth2.googleapis.com/token (refresh_token)`; retorno `GoogleResult<GoogleTokenResponse>`. Os parâmetros e tipos permanecem exatamente os de `INTERFACES.md`.

**Sucesso:** Responder com novo token de acesso sem `refresh_token` novo; preservar o anterior no consumidor sem inventar campo.

**Negativos e opcionais:** `invalid_grant`, timeout, token revogado e resposta sem access token válido não são sucesso. Respostas com campos opcionais ausentes devem preservar a distinção entre não informado e inválido.

**Retorno e invariantes:** verificar expiração, escopos e ausência legítima de `refresh_token`, com segredos ocultos. Confirmar resultado, erro tipado, isolamento por sessão/fluxo e que credenciais jamais aparecem em logs ou mensagens. Onde houver endpoint, capturar a requisição no servidor simulado; quando não houver rede, validar a função local sem inventar chamada HTTP.

#### API-64 — `revoke_token`

**Contrato testado:** `POST oauth2.googleapis.com/revoke`; retorno `GoogleResult<()>`. Os parâmetros e tipos permanecem exatamente os de `INTERFACES.md`.

**Sucesso:** Simular revogação aceita e verificar verbo, destino, envio seguro e retorno `()`.

**Negativos e opcionais:** Revogação recusada, rede indisponível e resposta inválida geram falha tipada; não divulgar token. Respostas com campos opcionais ausentes devem preservar a distinção entre não informado e inválido.

**Retorno e invariantes:** confirmar o status HTTP de sucesso e a ausência de corpo de recurso, sem tentar desserializar um objeto. Confirmar resultado, erro tipado, isolamento por sessão/fluxo e que credenciais jamais aparecem em logs ou mensagens. Onde houver endpoint, capturar a requisição no servidor simulado; quando não houver rede, validar a função local sem inventar chamada HTTP.

#### API-65 — `get_userinfo`

**Contrato testado:** `GET openidconnect.googleapis.com/v1/userinfo`; retorno `GoogleResult<GoogleUserInfo>`. Os parâmetros e tipos permanecem exatamente os de `INTERFACES.md`.

**Sucesso:** Simular a rota de UserInfo, desserializar usuário com identidade `sub` e dados opcionais.

**Negativos e opcionais:** Sessão sem autorização e UserInfo incompleto/inesperado não produzem identidade fictícia. Respostas com campos opcionais ausentes devem preservar a distinção entre não informado e inválido.

**Retorno e invariantes:** desserializar `GoogleUserInfo` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. Confirmar resultado, erro tipado, isolamento por sessão/fluxo e que credenciais jamais aparecem em logs ou mensagens. Onde houver endpoint, capturar a requisição no servidor simulado; quando não houver rede, validar a função local sem inventar chamada HTTP.

#### API-66 — `validate_id_token`

**Contrato testado:** `Validação local de JWT e chaves oficiais`; retorno `GoogleResult<GoogleIdClaims>`. Os parâmetros e tipos permanecem exatamente os de `INTERFACES.md`.

**Sucesso:** Validar JWT com assinatura verificável localmente e claims válidos, conforme a configuração e chaves fornecidas.

**Negativos e opcionais:** Assinatura incorreta, `alg` não permitido, emissor/audiência divergentes, expiração, `nonce` incorreto e JWT malformado falham. Respostas com campos opcionais ausentes devem preservar a distinção entre não informado e inválido.

**Retorno e invariantes:** desserializar `GoogleIdClaims` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. Confirmar resultado, erro tipado, isolamento por sessão/fluxo e que credenciais jamais aparecem em logs ou mensagens. Onde houver endpoint, capturar a requisição no servidor simulado; quando não houver rede, validar a função local sem inventar chamada HTTP.

#### API-67 — `get_oidc_discovery`

**Contrato testado:** `GET accounts.google.com/.well-known/openid-configuration`; retorno `GoogleResult<GoogleOidcDiscovery>`. Os parâmetros e tipos permanecem exatamente os de `INTERFACES.md`.

**Sucesso:** Simular o documento OIDC de discovery e verificar metadados/endpoints obrigatórios.

**Negativos e opcionais:** Discovery com endpoints inválidos, JSON incompleto e falha de rede geram erro tipado. Respostas com campos opcionais ausentes devem preservar a distinção entre não informado e inválido.

**Retorno e invariantes:** desserializar `GoogleOidcDiscovery` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. Confirmar resultado, erro tipado, isolamento por sessão/fluxo e que credenciais jamais aparecem em logs ou mensagens. Onde houver endpoint, capturar a requisição no servidor simulado; quando não houver rede, validar a função local sem inventar chamada HTTP.

#### API-68 — `get_oidc_keys`

**Contrato testado:** `GET do jwks_uri informado no Discovery`; retorno `GoogleResult<GoogleJwkSet>`. Os parâmetros e tipos permanecem exatamente os de `INTERFACES.md`.

**Sucesso:** Simular JWKS com múltiplas chaves e selecionar informações por `kid` para validação posterior.

**Negativos e opcionais:** JWKS malformado, chave inesperada, `kid` inexistente e falha de acesso são rejeitados. Respostas com campos opcionais ausentes devem preservar a distinção entre não informado e inválido.

**Retorno e invariantes:** desserializar `GoogleJwkSet` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. Confirmar resultado, erro tipado, isolamento por sessão/fluxo e que credenciais jamais aparecem em logs ou mensagens. Onde houver endpoint, capturar a requisição no servidor simulado; quando não houver rede, validar a função local sem inventar chamada HTTP.

### 7.3 Transferências e conveniências (10)

#### API-69 — `get_file_bytes`

**Contrato testado:** `GET /files/{fileId}?alt=media`; retorno `GoogleResult<GoogleDownloadStream>`. Usar assinatura pública e opções já definidas.

**Sucesso:** Simular resposta binária `alt=media` com conteúdo completo e com faixa de bytes, comparando payload e headers.

**Negativos e opcionais:** Range inválido, resposta `200` quando era necessária faixa `206`, tamanho divergente, truncamento e MIME não baixável. Testar ausência/presença de opções facultativas e resposta parcial com os dados efetivamente fornecidos.

**Retorno e invariantes:** verificar bytes e informações de transferência sem tratar leitura parcial como conclusão. Verificar requisição e bytes exatos quando aplicável, falhas tipadas, proteção da URI de upload, ausência de vazamento de credenciais e que conclusão de etapa não é anunciada como operação global concluída.

#### API-70 — `get_revision_bytes`

**Contrato testado:** `GET /files/{fileId}/revisions/{revisionId}?alt=media`; retorno `GoogleResult<GoogleDownloadStream>`. Usar assinatura pública e opções já definidas.

**Sucesso:** Simular download binário de revisão pelo ID do arquivo e da revisão, com e sem Range.

**Negativos e opcionais:** Revisão indisponível, Range insatisfatório e fluxo interrompido. Testar ausência/presença de opções facultativas e resposta parcial com os dados efetivamente fornecidos.

**Retorno e invariantes:** verificar bytes e informações de transferência sem tratar leitura parcial como conclusão. Verificar requisição e bytes exatos quando aplicável, falhas tipadas, proteção da URI de upload, ausência de vazamento de credenciais e que conclusão de etapa não é anunciada como operação global concluída.

#### API-71 — `export_file_bytes`

**Contrato testado:** `GET /files/{fileId}/export?mimeType=…`; retorno `GoogleResult<GoogleDownloadStream>`. Usar assinatura pública e opções já definidas.

**Sucesso:** Simular exportação de documento Google no MIME solicitado, garantindo bytes exatos.

**Negativos e opcionais:** Tipo MIME de exportação não suportado, recurso não exportável e bytes incompletos. Testar ausência/presença de opções facultativas e resposta parcial com os dados efetivamente fornecidos.

**Retorno e invariantes:** verificar bytes e informações de transferência sem tratar leitura parcial como conclusão. Verificar requisição e bytes exatos quando aplicável, falhas tipadas, proteção da URI de upload, ausência de vazamento de credenciais e que conclusão de etapa não é anunciada como operação global concluída.

#### API-72 — `start_resumable_create`

**Contrato testado:** `POST /upload/drive/v3/files?uploadType=resumable`; retorno `GoogleResult<GoogleUploadSession>`. Usar assinatura pública e opções já definidas.

**Sucesso:** Iniciar sessão de upload de criação e verificar URI `Location` opaca, MIME, metadados e tamanho opcional.

**Negativos e opcionais:** `Location` ausente/inválida, tamanho incoerente, falha de quota e resposta parcial. Testar ausência/presença de opções facultativas e resposta parcial com os dados efetivamente fornecidos.

**Retorno e invariantes:** desserializar `GoogleUploadSession` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. Verificar requisição e bytes exatos quando aplicável, falhas tipadas, proteção da URI de upload, ausência de vazamento de credenciais e que conclusão de etapa não é anunciada como operação global concluída.

#### API-73 — `start_resumable_update`

**Contrato testado:** `PATCH /upload/drive/v3/files/{fileId}?uploadType=resumable`; retorno `GoogleResult<GoogleUploadSession>`. Usar assinatura pública e opções já definidas.

**Sucesso:** Iniciar sessão de upload para ID existente, verificando PATCH da iniciação e opções.

**Negativos e opcionais:** Arquivo ausente, pré-condição remota falha, `Location` inválida e token exposto em erro. Testar ausência/presença de opções facultativas e resposta parcial com os dados efetivamente fornecidos.

**Retorno e invariantes:** desserializar `GoogleUploadSession` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. Verificar requisição e bytes exatos quando aplicável, falhas tipadas, proteção da URI de upload, ausência de vazamento de credenciais e que conclusão de etapa não é anunciada como operação global concluída.

#### API-74 — `upload_chunk`

**Contrato testado:** `PUT URI de sessão + Content-Range`; retorno `GoogleResult<GoogleUploadProgress>`. Usar assinatura pública e opções já definidas.

**Sucesso:** Enviar bloco com offset e `Content-Range`; simular `308` parcial e conclusão final.

**Negativos e opcionais:** Offset fora do esperado, range incoerente, sessão expirada, timeout após envio com resultado desconhecido e `308` sem conclusão. Testar ausência/presença de opções facultativas e resposta parcial com os dados efetivamente fornecidos.

**Retorno e invariantes:** desserializar `GoogleUploadProgress` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. Verificar requisição e bytes exatos quando aplicável, falhas tipadas, proteção da URI de upload, ausência de vazamento de credenciais e que conclusão de etapa não é anunciada como operação global concluída.

#### API-75 — `query_upload_progress`

**Contrato testado:** `PUT URI de sessão + Content-Range bytes */total`; retorno `GoogleResult<GoogleUploadProgress>`. Usar assinatura pública e opções já definidas.

**Sucesso:** Consultar status via URI da sessão e `Content-Range` de sondagem, sem reenviar bytes inexistentes.

**Negativos e opcionais:** Sessão expirada, posição confirmada incompatível, resposta inválida ou timeout. Testar ausência/presença de opções facultativas e resposta parcial com os dados efetivamente fornecidos.

**Retorno e invariantes:** desserializar `GoogleUploadProgress` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. Verificar requisição e bytes exatos quando aplicável, falhas tipadas, proteção da URI de upload, ausência de vazamento de credenciais e que conclusão de etapa não é anunciada como operação global concluída.

#### API-76 — `trash_file`

**Contrato testado:** `PATCH /files/{fileId} (trashed=true)`; retorno `GoogleResult<GoogleFile>`. Usar assinatura pública e opções já definidas.

**Sucesso:** Traduzir ação para atualização `trashed=true` do arquivo; não utilizar exclusão permanente.

**Negativos e opcionais:** Arquivo sem permissão, inexistente ou resposta sem estado compatível; não chamar DELETE. Testar ausência/presença de opções facultativas e resposta parcial com os dados efetivamente fornecidos.

**Retorno e invariantes:** desserializar `GoogleFile` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. Verificar requisição e bytes exatos quando aplicável, falhas tipadas, proteção da URI de upload, ausência de vazamento de credenciais e que conclusão de etapa não é anunciada como operação global concluída.

#### API-77 — `rename_file`

**Contrato testado:** `PATCH /files/{fileId} (name)`; retorno `GoogleResult<GoogleFile>`. Usar assinatura pública e opções já definidas.

**Sucesso:** Enviar PATCH de `name` somente e confirmar que não há upload de conteúdo.

**Negativos e opcionais:** Nome inválido, acesso negado e falha de PATCH; não reenviar bytes. Testar ausência/presença de opções facultativas e resposta parcial com os dados efetivamente fornecidos.

**Retorno e invariantes:** desserializar `GoogleFile` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. Verificar requisição e bytes exatos quando aplicável, falhas tipadas, proteção da URI de upload, ausência de vazamento de credenciais e que conclusão de etapa não é anunciada como operação global concluída.

#### API-78 — `move_file`

**Contrato testado:** `PATCH /files/{fileId} (addParents/removeParents)`; retorno `GoogleResult<GoogleFile>`. Usar assinatura pública e opções já definidas.

**Sucesso:** Transmitir `addParents` e `removeParents` por metadados e confirmar que não há upload.

**Negativos e opcionais:** Pais inconsistentes, falta de permissão e falha de atualização; não alterar conteúdo. Testar ausência/presença de opções facultativas e resposta parcial com os dados efetivamente fornecidos.

**Retorno e invariantes:** desserializar `GoogleFile` com campos mínimos e completos, preservando campos opcionais sem fabricar valores. Verificar requisição e bytes exatos quando aplicável, falhas tipadas, proteção da URI de upload, ausência de vazamento de credenciais e que conclusão de etapa não é anunciada como operação global concluída.

### Critérios de execução para todos os 78 métodos

Para cada caso **API-01 a API-78**, o teste deve registrar: nome do método, fixture independente, parâmetros usados, requisição capturada ou comportamento local, resposta simulada, estrutura pública resultante, campo opcional ausente/presente, erro negativo e resultado **PASS/FAIL** após execução. Um teste de compilação que apenas chama o trait não substitui o teste de contrato HTTP. Um teste com servidor fake não comprova disponibilidade real do serviço Google.

## 8. Cenários específicos por recurso

| Recurso | Cenários adicionais obrigatórios |
| --- | --- |
| about | Retorno de `storageQuota`, `exportFormats`, `importFormats`, `user` selecionados por `fields`; campos omitidos |
| accessproposals | Proposta inexistente, lista vazia/paginada, resolução com ação inválida, retorno vazio de sucesso quando previsto |
| approvals | Aprovação, cancelamento, comentário, recusa, leitura, listagem, reatribuição e início; validar body diferente por ação, status e acesso negado |
| apps | App inexistente, filtros de extensão/MIME, nome e capacidades de criação/abertura |
| changes | `startPageToken`, `nextPageToken`, `newStartPageToken`, várias páginas; `removed=true` inclui perda de acesso; cursor inválido/expirado |
| channels | Stop com `id` e `resourceId`, status de sucesso e erro de canal inválido |
| comments | Criação e atualização de conteúdo, `fields`, paginação, comentários excluídos, permissão negada |
| drives | Criação com request ID, listagem paginada, ocultação/desocultação, restrições, acesso de administrador, exclusão protegida |
| files | Filtros `q`, `corpora`, `pageSize`, `fields`; criar/copiar/atualizar; PATCH parcial; renomear/mover por metadados; lixeira diferente de exclusão permanente; labels, CSE, ID gerado e watch |
| operations | `done=false` é pendente; `done=true` com `response` ou `error`; nome de operação inválido |
| permissions | Tipo `user/group/domain/anyone`, papéis, compartilhamento, transferência de propriedade, notificação, expiração, `403` por direitos insuficientes |
| replies | Criação, resposta, alteração, exclusão, paginação, `action`, conteúdo omitido |
| revisions | Leitura, listagem, atualização, exclusão, export links, revisão inexistente e restrições de tipo de arquivo |
| OAuth | PKCE S256, state, redirect URI, escopos, consentimento negado, troca de código, refresh com ausência de novo `refresh_token`, revogação, `invalid_grant`, segredo ausente/mascarado |
| OIDC | Descoberta de endpoints, URL de chaves, `kid`, assinatura, `iss`, `aud`, `exp`, `iat`, nonce, algoritmo não esperado, token malformado e identidade por `sub` |
| download | `alt=media` retorna bytes; `Range` válido/fora do limite; `206` e `Content-Range`; respostas com `200` inesperado; exportação em MIME inválido; download assíncrono distinto de fluxo direto |
| upload resumível | `Location` de sessão, envio parcial, `Content-Range`, bytes e offsets; `308` incompleto, confirmação final, sessão expirada, retry sem duplicar conteúdo, tamanho inconsistente |
| conveniências | `trash_file`, `rename_file`, `move_file` traduzidas a `files.update`, sem tráfego de bytes do conteúdo |

## 9. Cobertura de todos os tipos públicos

**Critério obrigatório:** cada `pub struct`, `pub enum` e `pub type` do `INTERFACES.md` deve ter testes, mesmo quando ainda não é utilizado por um consumidor externo. A cobertura é de **campos, conversões e contratos**, não apenas de linhas executadas.

| Categoria | Testes obrigatórios |
| --- | --- |
| IDs e tipos simples | Novo tipo aceita identificador válido e preserva conteúdo; não confundir IDs entre recursos; Unicode; serialização |
| Opções e requests | Campo obrigatório, opcionais `None`, todos os campos preenchidos, formato da query/body, valores inválidos onde aplicável |
| Recursos de resposta | JSON mínimo, JSON completo, campos aninhados, omitidos, desconhecidos e dados em formatos inválidos |
| Listas | Lista vazia, múltiplos itens, próximo token, tipos aninhados e ausência de token final |
| Enums | Cada variante e seus dados, mapeamento/serialização quando for parte do protocolo |
| OAuth/tokens | `expires_in`, `refresh_token?`, `id_token?`, URL de autorização, OIDC claims, sanitização de segredos |
| Transferências | Stream de bytes, intervalos, offsets, sessões, progresso incompleto e final |
| Falhas | Todas as variantes do erro, `http_status?`, `provider_reason?`, `retry_after_seconds?`, `retryable` |

### Checklist estático dos tipos declarados

Gerar um teste de conformidade que percorra as **declarações públicas** de `INTERFACES.md` como manifesto, e verifique que cada tipo possui pelo menos um teste de contrato correspondente. O inventário inicial extraído do documento é:

- `FileId`, `DriveId`, `ApprovalId`, `ProposalId`, `AppId`, `CommentId`, `ReplyId`, `PermissionId`, `RevisionId`, `OperationName`, `ChannelId`, `RequestId`
- `MimeType`, `FieldMask`, `GoogleSession`, `PageOptions`, `GooglePage`, `GoogleApiError`, `GoogleApiFailure`, `GoogleAbout`, `GoogleUser`, `GoogleStorageQuota`, `GoogleFile`, `GoogleShortcutDetails`
- `GoogleFileCapabilities`, `GoogleContentHints`, `GoogleThumbnail`, `GoogleImageMediaMetadata`, `GoogleGeoLocation`, `GoogleVideoMediaMetadata`, `GoogleContentRestriction`, `GoogleDownloadRestrictions`, `GoogleDownloadRestriction`, `GoogleClientEncryptionDetails`, `GoogleDecryptionMetadata`, `GoogleLabelInfo`
- `GoogleLabel`, `GoogleLabelField`, `GoogleFileList`, `GoogleChange`, `GoogleChangeList`, `GoogleStartPageToken`, `GoogleDrive`, `GoogleDriveRestrictions`, `GoogleDriveCapabilities`, `GooglePermission`, `GooglePermissionDetails`, `GoogleRevision`
- `GoogleComment`, `GoogleQuotedContent`, `GoogleReply`, `GoogleChannel`, `GoogleApp`, `GoogleAccessProposal`, `GoogleRoleAndView`, `GoogleApproval`, `GoogleApprovalReviewerResponse`, `GoogleOperation`, `GoogleOperationMetadata`, `GoogleRpcStatus`
- `GoogleDownloadFileResponse`, `GoogleCseToken`, `GoogleGeneratedIds`, `GoogleModifyLabelsResponse`, `GooglePatch`, `GoogleFileWrite`, `GoogleDriveWrite`, `GooglePermissionWrite`, `GoogleCommentWrite`, `GoogleReplyWrite`, `GoogleRevisionWrite`, `GoogleAccessProposalList`
- `GoogleAppList`, `GoogleApprovalList`, `GoogleCommentList`, `GoogleDriveList`, `GoogleLabelList`, `GooglePermissionList`, `GoogleReplyList`, `GoogleRevisionList`, `AboutOptions`, `AppListOptions`, `ChangeScopeOptions`, `ChangeListOptions`
- `ChangeWatchOptions`, `CommentOptions`, `CommentListOptions`, `CseTokenOptions`, `DriveDeleteOptions`, `DriveGetOptions`, `DriveListOptions`, `DriveUpdateOptions`, `EmptyTrashOptions`, `FileCopyOptions`, `FileCreateOptions`, `FileDeleteOptions`
- `FileDownloadOptions`, `FileGetOptions`, `FileListOptions`, `FileUpdateOptions`, `FileWatchOptions`, `GenerateIdsOptions`, `LabelListOptions`, `PermissionCreateOptions`, `PermissionDeleteOptions`, `PermissionGetOptions`, `PermissionListOptions`, `PermissionUpdateOptions`
- `ReplyOptions`, `ReplyListOptions`, `RevisionGetOptions`, `RevisionListOptions`, `GoogleChannelRequest`, `GoogleChannelStopRequest`, `GoogleModifyLabelsRequest`, `GoogleLabelModification`, `GoogleLabelFieldModification`, `ApproveApprovalRequest`, `CancelApprovalRequest`, `CommentApprovalRequest`
- `DeclineApprovalRequest`, `ReassignApprovalRequest`, `StartApprovalRequest`, `GoogleApprovalReviewer`, `ResolveAccessProposalRequest`, `GoogleConfigSources`, `GoogleOAuthConfig`, `GoogleAuthorizationRequest`, `GoogleAuthorizationUrl`, `GoogleTokenResponse`, `GoogleUserInfo`, `GoogleIdTokenValidation`
- `GoogleIdClaims`, `GoogleOidcDiscovery`, `GoogleJwkSet`, `GoogleJwk`, `GoogleByteRange`, `GoogleDownloadStream`, `GoogleUploadSession`, `GoogleUploadChunk`, `GoogleUploadProgress`

Para cada campo de cada tipo, gerar cenários **presente / omitido / inválido / null quando permitido**, com atenção especial aos tipos `GoogleFile`, `GoogleChange`, `GooglePermission`, `GoogleRevision`, `GoogleApproval` e aos tipos aninhados. **Não** exigir `null` onde a documentação oficial não o aceita.

### Casos especiais de precisão e ausência

- `size`, `version`, `quotaBytesUsed` e outros `int64` serializados como strings; testar valores maiores que 2⁵³, zero e entrada inválida.
- Datas RFC 3339, fusos, milissegundos e ausências legítimas.
- `GoogleFile` de pasta, atalho, binário e documento nativo Google: `size` e checksums podem não existir.
- `fields` com resposta parcial **não confirma** exclusão de campos nem ausência de arquivo.
- `GooglePatch::Unchanged`, `Set`, `Clear`: verificar ausência do campo no body, valor presente e limpeza **apenas onde suportada**.
- `GoogleApiFailure` deve preservar o código de motivo seguro para diagnóstico sem vazar token, sessão ou body privado.

## 10. Tratamento de erros e respostas HTTP

| Caso injetado no servidor simulado | Expectativa |
| --- | --- |
| `400` com parâmetros inválidos | `InvalidArgument`; não retry sem alteração da entrada |
| OAuth `invalid_grant` | `InvalidGrant` ou `ReauthorizationRequired`, conforme contrato |
| `401` | `AuthenticationRequired`, sem apagar conta nem invalidar sessões alheias |
| `403 insufficientPermissions` | `InsufficientScope` ou `PermissionDenied` conforme reason |
| `403 rateLimitExceeded/userRateLimitExceeded` ou `429` | `RateLimited`, propagando `Retry-After` quando existir |
| `403 storageQuotaExceeded` | `StorageQuotaExceeded` |
| `403 fileNotDownloadable` | `FileNotDownloadable` |
| `404` | `NotFoundOrInaccessible` — não interpretar automaticamente como exclusão |
| `409` / `412` | `PreconditionFailed`; sem repetir escrita cegamente |
| `410` de changes | `ExpiredCursor` |
| `500`–`504` | `ProviderUnavailable`; retry limitado apenas quando seguro |
| Falha de conexão / timeout / cancelamento | `Network`, `Timeout`, `Cancelled`, sem sucesso falso |
| JSON inesperado ou truncado | `InvalidResponse`; sem aceitar como recurso válido |
| Upload `308` | `GoogleUploadProgress::Incomplete`; **não é falha nem sucesso final** |
| Operation `done=false` | Operação pendente; **não é sucesso final** |

Testar `reason` desconhecido e respostas de erro sem `reason`: deve existir erro público estável e seguro, sem `panic`. Mensagem humana do Google não deve ser utilizada para classificação.

## 11. Metas de cobertura e critérios de aceite

| Métrica | Critério mínimo |
| --- | --- |
| Métodos REST | **59/59** com testes de contrato HTTP e retornos/erros |
| Métodos complementares | **19/19** com teste positivo, negativo e validação das entradas e saídas aplicáveis |
| Tipos públicos | **100%** das declarações de tipos com teste de estrutura/serialização/validação conforme sua natureza |
| Campos de resposta/entrada | **100%** dos campos documentados com pelo menos caso presente e caso omitido/ausente quando permitido |
| Variantes de erro | **100%** das variantes com pelo menos um cenário de mapeamento/propagação ou validação local |
| Branches da lógica de transporte/parser | **Meta de 90%** de cobertura de branches mensuráveis, sem substituir o checklist de comportamento |
| Casos críticos | **100%** dos cenários de segurança, paginação, upload parcial, `fields` e bytes listados aqui |
| Dependência de Google real | **Zero**; toda a suíte deve funcionar offline |

Cobertura por porcentagem **não substitui assertivas de contrato**. Se uma operação não tiver comportamento simulado validado, registrar como pendente; não marcar como coberta por um teste de mock que somente devolve resposta fixa.

## 12. Execução e relatórios

- Seguir [AGENTS.md](AGENTS.md) e o `AGENTS.md` da raiz. Executar `cargo test`, `cargo fmt`, `cargo clippy` e demais comandos Rust **somente dentro do Flatpak** do projeto.
- A suíte deve executar sem credenciais, sem rede externa e sem dados do usuário.
- Testes de contrato devem emitir erros úteis de comparação: método/URL/query/body esperado versus recebido, **sem imprimir segredos**.
- Documentar por método: testes criados em RED, fixtures oficiais adotadas, resultado GREEN, cenários pendentes e limitações.
- Fazer commits por grupo lógico completo na **branch já escolhida pelo usuário**, sem criar branch por iniciativa do agente; respeitar as regras de publicação/commits do `AGENTS.md` da raiz.
- **Não afirmar** que endpoints estão funcionando no serviço Google real: testes simulados demonstram somente conformidade com a especificação e com os cenários de teste.

## 13. Critério de encerramento desta etapa documental

Este documento é o **plano dos testes**, não a execução deles. A etapa de implementação por TDD só começa quando o agente selecionar um grupo, escrever e executar testes inicialmente falhando, verificar as expectativas na documentação oficial e implementar o mínimo para fazê-los passar.
