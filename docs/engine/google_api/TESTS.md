# Google API — TESTS.md

## 1. Objetivo e limites

Este documento especifica **os testes a escrever antes de implementar** o módulo `google_api/` (Contract-First + TDD). O objetivo é demonstrar, **sem acessar Google real**, que cada interface envia a requisição correta e interpreta corretamente respostas, tipos, erros e estados parciais.

- **Fonte de verdade:** [INTERFACES.md](INTERFACES.md), [README.md](README.md) e referência oficial [Drive API v3](https://developers.google.com/workspace/drive/api/reference/rest/v3) / [Discovery](https://www.googleapis.com/discovery/v1/apis/drive/v3/rest), [OAuth](https://developers.google.com/identity/protocols/oauth2/native-app) e [OIDC](https://developers.google.com/identity/openid-connect/reference).
- **Cobertura obrigatória:** todos os **59 métodos REST**, os **19 métodos complementares de OAuth/OIDC/transferências/conveniências** e todos os tipos públicos declarados em `INTERFACES.md`.
- **Não executar chamadas reais**, cadastrar conta, acessar arquivos pessoais, modificar dados remotos ou exigir credenciais. Não escrever testes E2E nesta etapa.
- **Não produzir código funcional ou editar outros módulos** apenas para habilitar testes; o objetivo é a implementação independente da Google API.
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

**Regra para cada linha:** executar a matriz **M01–M12 aplicável**, além dos cenários específicos do recurso na seção 7. Esta tabela é também o checklist de cobertura: nenhuma operação documentada pode ficar sem testes.

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

**Regra:** executar a mesma matriz de contrato, adaptando M02–M05 às operações locais (configuração, validação JWT) e às operações sem body. Validar o comportamento próprio descrito na seção 7.

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

## 7. Cenários específicos por recurso

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

## 8. Cobertura de todos os tipos públicos

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

## 9. Tratamento de erros e respostas HTTP

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

## 10. Metas de cobertura e critérios de aceite

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

## 11. Execução e relatórios

- Seguir [AGENTS.md](AGENTS.md) e o `AGENTS.md` da raiz. Executar `cargo test`, `cargo fmt`, `cargo clippy` e demais comandos Rust **somente dentro do Flatpak** do projeto.
- A suíte deve executar sem credenciais, sem rede externa e sem dados do usuário.
- Testes de contrato devem emitir erros úteis de comparação: método/URL/query/body esperado versus recebido, **sem imprimir segredos**.
- Documentar por método: testes criados em RED, fixtures oficiais adotadas, resultado GREEN, cenários pendentes e limitações.
- Fazer commits por grupo lógico completo na **branch já escolhida pelo usuário**, sem criar branch por iniciativa do agente; respeitar as regras de publicação/commits do `AGENTS.md` da raiz.
- **Não afirmar** que endpoints estão funcionando no serviço Google real: testes simulados demonstram somente conformidade com a especificação e com os cenários de teste.

## 12. Critério de encerramento desta etapa documental

Este documento é o **plano dos testes**, não a execução deles. A etapa de implementação por TDD só começa quando o agente selecionar um grupo, escrever e executar testes inicialmente falhando, verificar as expectativas na documentação oficial e implementar o mínimo para fazê-los passar.
