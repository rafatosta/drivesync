# Google API

> **Posição arquitetural:** esta é a API **técnica específica Google**. A fábrica [Providers](../../INTERFACES.md) disponibiliza a implementação por meio de [Google Provider](../INTERFACES.md); os especialistas dos módulos consumidores podem usar todos os métodos/tipos Google necessários, preservando o contrato de saída do módulo pai. Esta organização **não substitui nem altera** as operações listadas abaixo.

## Objetivo

**Centralizar e abstrair toda integração específica com o Google** — Google Drive API, OAuth/OpenID Connect, URLs, endpoints, escopos, parâmetros, configurações do provedor, formatos de arquivos, limites, respostas e erros. Os outros módulos consomem **operações e tipos públicos**, sem conhecer detalhes HTTP ou constantes do Google.

## Interfaces públicas — contrato inicial

Os nomes abaixo identificam **operações conceituais**; não fixam assinaturas Rust nem a organização interna da implementação.

| Interface | Entrada | Saída |
| --- | --- | --- |
| `load_google_config` | Configuração da instalação/ambiente | Configuração validada do provedor |
| `authorization_request` | Configuração, redirecionamento, state e desafio PKCE | Solicitação de autorização pronta para o navegador |
| `exchange_authorization_code` | Código, redirect e verificador PKCE | Tokens e validade |
| `refresh_access` | Credencial de renovação | Tokens renovados e validade |
| `get_identity` | Sessão autenticada | Identidade verificada do Google |
| `get_file` | Sessão, ID remoto | Metadados do arquivo ou diretório |
| `list_children` | Sessão, pasta, paginação e escopo | Página de itens e continuação |
| `get_changes_cursor` | Sessão e escopo do Drive | Cursor inicial |
| `list_changes` | Sessão, escopo e cursor | Página de alterações e continuação |
| `download_file` | Sessão, ID e intervalo opcional | Fluxo de bytes e metadados da transferência |
| `export_google_file` | Sessão, ID de arquivo nativo e formato de exportação | Fluxo de bytes e metadados da exportação |
| `create_file` | Sessão, pai, nome, tipo e conteúdo quando aplicável | Item remoto criado |
| `update_file_content` | Sessão, ID, conteúdo e precondições | Item remoto atualizado |
| `update_file_metadata` | Sessão, ID e alterações de metadados | Item remoto atualizado |
| `move_file` | Sessão, ID e destino | Item remoto com nova localização |
| `delete_file` | Sessão, ID e modalidade explícita da ação | Resultado da operação |

## Tipos e informações públicas

| Contrato | O que expõe |
| --- | --- |
| `GoogleConfig` | Identidade do cliente OAuth e opções de integração validadas |
| `GoogleSession` | Capacidade de acesso autenticado vinculada à conta, sem exigir token bruto dos consumidores |
| `GoogleIdentity` | Identificador da conta e dados de identidade verificados |
| `GoogleFile` | ID, nome, MIME, pais, versão, tamanho, hashes, datas, lixeira e capacidades quando disponibilizados |
| `GoogleFilePage` | Itens, indicador de completude e cursor da próxima página |
| `GoogleChangePage` | Alterações, próximo cursor e conclusão da página |
| `GoogleTransfer` | Conteúdo em fluxo, comprimento e informações de retomada/verificação |
| `GoogleFileFormat` | Classificação MIME: diretório, arquivo comum e documento nativo Google |
| `GoogleExportFormat` | Formatos de exportação aceitos para documentos nativos |
| `GoogleApiError` | Categoria de erro tipada e informações seguras para recuperação |

## Erros públicos

| Categoria | Significado para o consumidor |
| --- | --- |
| `AuthenticationRequired` | Não há credencial válida para a solicitação |
| `PermissionDenied` | A conta não tem permissão para a ação |
| `NotFound` | Recurso remoto não encontrado ou indisponível |
| `ConflictOrPreconditionFailed` | O estado remoto não satisfaz a condição da operação |
| `RateLimited` | Limite de requisições atingido; pode exigir nova tentativa |
| `ProviderUnavailable` | Falha transitória no serviço do Google |
| `NetworkError` | Falha de comunicação |
| `InvalidRequest` | Parâmetros ou operação incompatíveis |
| `InvalidResponse` | Resposta incompleta, inválida ou incompatível |
| `UnsupportedFormat` | Formato ou operação não suportados |
| `Cancelled` | Operação cancelada |

Os erros não expõem tokens, respostas privadas do provedor ou URLs com credenciais. Informações de retry, quando disponíveis, fazem parte do erro tipado.

## Regras da interface

| Regra | Contrato |
| --- | --- |
| Centralização | Nenhum consumidor define URLs, endpoints, escopos, cabeçalhos, parâmetros ou constantes específicas da API do Google |
| Configuração | Identificadores de cliente, variáveis de ambiente e arquivos OAuth específicos do Google são interpretados neste módulo |
| Dados | Tipos e respostas específicos do Google são definidos aqui e expostos em formato tipado |
| Autenticação | `accounts/auth/` conduz o fluxo de contas e persiste credenciais; a `providers/google/api/` conhece o protocolo e os serviços Google |
| Paginação | Resultados paginados informam explicitamente sua continuação; uma página não é tratada como inventário completo |
| Operações | Criação, atualização, renomeação, movimentação e exclusão são operações distintas; mover ou renomear não implica reupload |
| Segurança | A API rejeita respostas excessivas ou inválidas, não vaza credenciais e não transforma falhas em sucesso |
| Limites | Limites de tamanho de resposta, timeouts e parâmetros aceitos do Google são definidos internamente |
| Responsabilidade | O módulo não identifica conflitos, não decide a sincronização e não armazena o inventário |

## Validação independente das interfaces

| Cenário de teste | Resultado esperado |
| --- | --- |
| Consultar metadados existentes | Retorna `GoogleFile` com ID e atributos disponíveis |
| Consultar recurso inexistente | Retorna `NotFound`, sem criar resultado fictício |
| Listar pasta com múltiplas páginas | Entrega páginas e cursores até conclusão |
| Ler alterações com cursor inválido | Retorna erro tipado; não interpreta como ausência de alterações |
| Renovar acesso autorizado | Retorna credenciais renovadas e validade |
| Renovação rejeitada pelo Google | Retorna erro de autenticação sem expor o corpo privado da resposta |
| Baixar intervalo de um arquivo | Retorna os bytes/indicadores do intervalo solicitado ou erro explícito |
| Mover ou renomear arquivo | Altera apenas os metadados necessários; não realiza upload de conteúdo |
| Receber limite de requisições | Retorna `RateLimited` e dados seguros de retry, quando presentes |
| Receber erro de rede, permissão ou formato | Retorna a categoria correspondente, sem registrar sucesso |

As interfaces podem ser substituídas por **implementações simuladas** nos testes dos demais módulos. Esses testes validam entradas, saídas, erros e decisões dos consumidores sem acesso real ao Google Drive.

## Endpoints oficiais e correspondência com interfaces

**Base REST Drive v3:** `https://www.googleapis.com/drive/v3`  
**Base de upload:** `https://www.googleapis.com/upload/drive/v3`

Os endpoints e os parâmetros são **detalhes encapsulados** pelo módulo. Nenhum consumidor precisa montar URLs. Métodos como renomear, mover e enviar conteúdo são interfaces de domínio distintas mesmo quando compartilham o mesmo endpoint HTTP do Google.

### Identidade e OAuth

| Interface pública | Método e URL oficiais | Parâmetros/entrada essenciais | Resposta do Google |
| --- | --- | --- | --- |
| `authorization_request` | Navegador: `https://accounts.google.com/o/oauth2/v2/auth` | `client_id`, `redirect_uri`, `response_type=code`, `scope`, `state`, `code_challenge`, `code_challenge_method=S256`, `access_type=offline` | Redirecionamento com `code` e `state` ou `error` |
| `exchange_authorization_code` | `POST https://oauth2.googleapis.com/token` | `grant_type=authorization_code`, `code`, `redirect_uri`, `client_id`, `code_verifier` | `GoogleTokenResponse` |
| `refresh_access` | `POST https://oauth2.googleapis.com/token` | `grant_type=refresh_token`, `refresh_token`, `client_id` | `GoogleTokenResponse` (não presumir novo refresh token) |
| `get_identity` | `GET https://openidconnect.googleapis.com/v1/userinfo` | Header `Authorization: Bearer …` | `GoogleUserInfo` |

A seleção de escopos faz parte da configuração do provedor. Escopos como `openid`, `email` e `https://www.googleapis.com/auth/drive` são identificadores do Google e ficam centralizados. **Apenas o tipo/contrato de tokens é exposto ao serviço de autenticação; segredos não devem circular entre os demais módulos.**

Referências: [OAuth para desktop](https://developers.google.com/identity/protocols/oauth2/native-app), [OpenID Connect](https://developers.google.com/identity/openid-connect/reference).

### Arquivos, metadados e alterações

| Interface pública | Método e endpoint Drive v3 | Entrada/parâmetros principais | Resposta do Google |
| --- | --- | --- | --- |
| `get_file` | `GET /files/{fileId}` | `fileId`, `fields`, `supportsAllDrives` | `File` |
| `list_children` | `GET /files` | `q` filtrando ID do pai, `fields`, `pageSize`, `pageToken`, `spaces`, escopo de drives | `FileList` |
| `get_changes_cursor` | `GET /changes/startPageToken` | `driveId` quando aplicável, `supportsAllDrives` | `StartPageToken` |
| `list_changes` | `GET /changes` | `pageToken`, `fields`, `pageSize`, `includeRemoved`, opções de drives compartilhados | `ChangeList` |
| `create_file` sem conteúdo | `POST /files` | Corpo `File`: `name`, `mimeType`, `parents`; pasta usa MIME específico | `File` criado |
| `update_file_metadata` | `PATCH /files/{fileId}` | Somente campos alterados, como `name` ou `trashed` | `File` atualizado |
| `move_file` | `PATCH /files/{fileId}` | `addParents` e `removeParents`, conforme o pai anterior e o destino | `File` atualizado |
| `delete_file` permanente | `DELETE /files/{fileId}` | `fileId`, `supportsAllDrives` | Resposta vazia de sucesso |
| `trash_file` (operação distinta) | `PATCH /files/{fileId}` | Corpo `{ "trashed": true }` | `File` atualizado |

**Atenção:** exclusão permanente e envio à lixeira são comportamentos diferentes. A interface deverá exigir uma modalidade explícita, sem converter silenciosamente uma na outra. Alterações de nome e pasta podem ser feitas como atualização de metadados, sem transferir bytes.

Referências: [files](https://developers.google.com/workspace/drive/api/reference/rest/v3/files), [files.list](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/list), [files.update](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/update), [files.create](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/create), [files.delete](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/delete), [changes.list](https://developers.google.com/workspace/drive/api/reference/rest/v3/changes/list).

### Conteúdo, exportação e transferências

| Interface pública | Método e endpoint | Entrada essencial | Resposta |
| --- | --- | --- | --- |
| `download_file` (arquivo binário) | `GET /files/{fileId}?alt=media` | `fileId`, intervalo HTTP quando aplicável | **Bytes** do conteúdo, não objeto `File` |
| `export_google_file` | `GET /files/{fileId}/export` | `fileId` e `mimeType` de exportação | **Bytes** exportados |
| `download_file_operation` (download assíncrono) | `POST /files/{fileId}/download` | `fileId`, `mimeType` ou `revisionId` opcionais conforme o tipo | **`Operation`**, que pode fornecer URI de download após conclusão |
| `create_file` com conteúdo | `POST /upload/drive/v3/files?uploadType=…` | `media`, `multipart` ou `resumable`; metadados e conteúdo | `File` após conclusão |
| `update_file_content` | `PATCH /upload/drive/v3/files/{fileId}?uploadType=…` | ID, modo de upload, bytes, precondições adequadas | `File` após conclusão |
| `resume_upload` (capacidade da interface de transferência) | URI da sessão retornada pela iniciação resumível | `PUT`, `Content-Range`, tamanho e bytes pendentes | `308` parcial ou `200/201` com `File` |

O fluxo resumível retorna a **URI de sessão no cabeçalho `Location`**. Em progresso, o Google pode responder `308 Resume Incomplete` e informar o intervalo recebido em `Range`; isso **não significa upload concluído**. Os blocos intermediários de upload são múltiplos de **256 KiB**. Uma URI de sessão expirada exige tratamento próprio. O limite de exportação pelo método `files.export` é **10 MB**. A API `files.download` é uma **long-running operation** e não deve ser confundida com a transferência direta de bytes via `alt=media`.

Referências: [files.get](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/get), [files.export](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/export), [files.download](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/download), [upload resumível](https://developers.google.com/workspace/drive/api/guides/manage-uploads), [operações assíncronas](https://developers.google.com/workspace/drive/api/guides/long-running-operations).

## Interfaces de dados do provedor

**Convenções:** tipos abaixo descrevem **campos retornados pela API oficial**, não modelos internos `SyncItem`. Os campos opcionais dependem do recurso, escopos, permissões e seleção explícita por `fields`. Os formatos JSON do Google não devem escapar do módulo: a API pública disponibiliza tipos validados equivalentes.

### `GoogleFile` — recurso `File`

| Campo original JSON | Tipo lógico | Observações |
| --- | --- | --- |
| `id` | string | Identificador remoto estável |
| `name` | string | Nome, não identidade |
| `mimeType` | string | Pasta, documento nativo, atalho ou arquivo comum |
| `parents` | string[] | IDs das pastas pais, quando informados |
| `trashed` | boolean | Estado de lixeira |
| `version` | int64 representado como string JSON | Versão do provedor, não relógio local |
| `modifiedTime` | string RFC 3339 | Data remota |
| `size` | int64 representado como string JSON | Pode faltar em pastas ou atalhos |
| `md5Checksum` | string | Disponível em certos arquivos binários; não presumir para Docs nativos |
| `headRevisionId` | string | Revisão principal quando suportada |
| `driveId` | string | ID de drive compartilhado, quando aplicável |
| `capabilities` | objeto | Capacidades/autorizações efetivas do usuário |
| `webViewLink` | string URL | Link oficial para abrir no navegador, quando informado |
| `webContentLink` | string URL | Link de download no navegador para conteúdos binários, quando informado |
| `shortcutDetails` | objeto | Referência ao alvo em atalhos nativos do Drive, quando aplicável |

O tipo MIME de diretório é `application/vnd.google-apps.folder`; o tipo MIME de atalho é `application/vnd.google-apps.shortcut`. Formatos nativos de Docs/Sheets/Slides são distintos de conteúdo binário normal. A classificação e as listas de formatos pertencem ao módulo.

Referência: [recurso File](https://developers.google.com/workspace/drive/api/reference/rest/v3/files).

### Respostas paginadas e mudanças

| Tipo | Campos JSON principais | Semântica |
| --- | --- | --- |
| `GoogleFileList` | `files[]: File`, `nextPageToken?: string`, `incompleteSearch?: boolean`, `kind` | Página de arquivos, não representa necessariamente listagem completa |
| `GoogleStartPageToken` | `startPageToken: string`, `kind` | Cursor inicial de alterações |
| `GoogleChange` | `fileId?: string`, `file?: File`, `removed: boolean`, `changeType?: string`, `driveId?: string`, `time?: string` | Alteração pode indicar exclusão **ou perda de acesso**; não presumir exclusão física |
| `GoogleChangeList` | `changes[]: Change`, `nextPageToken?: string`, `newStartPageToken?: string`, `kind` | Somente usar `newStartPageToken` ao concluir a paginação; havendo `nextPageToken`, continuar leitura |

Referências: [files.list](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/list), [recurso Change](https://developers.google.com/workspace/drive/api/reference/rest/v3/changes), [changes.list](https://developers.google.com/workspace/drive/api/reference/rest/v3/changes/list), [guia de mudanças](https://developers.google.com/workspace/drive/api/guides/manage-changes).

### Identidade, tokens, upload e download assíncrono

| Tipo | Campos originais relevantes | Contrato |
| --- | --- | --- |
| `GoogleTokenResponse` | `access_token`, `expires_in`, `token_type`, `scope`, `refresh_token?`, `id_token?`, `refresh_token_expires_in?` | Dados sigilosos entregues apenas ao consumidor de autenticação |
| `GoogleUserInfo` | `sub`, `email?`, `email_verified?`, `name?`, `picture?`, `hd?` | `sub` é identidade estável da conta; e-mail não substitui `sub` |
| `GoogleUploadSession` | Cabeçalho `Location`, `Range` e status HTTP | Sessão/posição e conclusão explícitas |
| `GoogleDownloadOperation` | `name`, `done?`, `metadata?`, `response?`, `error?` | Operação de longa duração; pode precisar de consulta posterior |
| `GoogleDownloadFileResponse` | `downloadUri`, `partialDownloadAllowed` | Recurso de resposta da operação concluída; não equivale a bytes já transferidos |

Referências: [OAuth para desktop](https://developers.google.com/identity/protocols/oauth2/native-app), [UserInfo](https://developers.google.com/identity/openid-connect/reference), [operações de download](https://developers.google.com/workspace/drive/api/guides/long-running-operations).

### Formatos de exportação — exemplos oficiais

| Documento Google | Formatos de saída exemplares |
| --- | --- |
| Docs | `.docx`, `.odt`, `.pdf`, `.txt`, `.md` |
| Sheets | `.xlsx`, `.ods`, `.pdf`, `.csv` |
| Slides | `.pptx`, `.odp`, `.pdf` |
| Drawings | `.png`, `.jpeg`, `.svg`, `.pdf` |
| Google Vids | **Não suporta `files.export`**; usar `files.download` para MP4 |

O módulo mapeia extensões para os MIME types oficiais e valida combinações suportadas. Não se deve construir uma conversão arbitrária a partir da extensão. Para formatos disponíveis por conta, a API também expõe `about.get` com `fields=exportFormats`.

Referência: [formatos de exportação](https://developers.google.com/workspace/drive/api/guides/ref-export-formats).

## Tratamento das respostas e erros oficiais

| Origem | Distinção necessária na interface |
| --- | --- |
| `400` | Solicitação inválida: parâmetros, formato ou operação não suportados |
| `401` | Credenciais inválidas/expiradas; autenticação ou renovação necessária |
| `403` | **Inspecionar `reason`**: pode ser permissão, limite de requisições, quota de armazenamento ou arquivo não exportável; não mapear todo 403 a `PermissionDenied` |
| `404` | Recurso inexistente **ou inacessível**; não usar isoladamente como prova de exclusão |
| `429` | Limite de requisições, potencialmente recuperável por retry/backoff |
| `500`–`504` | Falhas do serviço, normalmente transitórias |
| `308` em upload resumível | Progresso incompleto, **não é erro e não é conclusão** |
| `410` em cursor de alterações | Cursor rejeitado/inválido; requer tratamento específico no detector, sem afirmar que itens foram excluídos |
| Falha de rede, JSON inválido, resposta parcial ou cancelamento | Erro tipado explícito, sem retorno de sucesso |

A resposta de erro Google pode incluir `error.code`, `error.message` e `error.errors[].reason`. O módulo converte essas informações em `GoogleApiError` seguro, sem expor texto confidencial ou token bruto. O contrato deve distinguir erros **recuperáveis**, **dependentes de nova autenticação**, **dependentes de intervenção** e **de entrada inválida**.

Referência: [erros da Drive API](https://developers.google.com/workspace/drive/api/guides/handle-errors).

## Contratos de teste adicionais baseados na documentação oficial

| Caso | Resultado verificável com API simulada |
| --- | --- |
| `FileList` com `nextPageToken` e `incompleteSearch` | Consumidor não considera a varredura encerrada |
| `ChangeList` com `nextPageToken` | Não confirma `newStartPageToken` antes de completar páginas |
| `Change.removed=true` | Não presume exclusão física, pois também pode significar perda de acesso |
| `files.get` com `alt=media` | Retorno de bytes, nunca desserialização como `File` |
| `files.export` acima do limite | Erro explícito, sem declarar arquivo local completo |
| `files.download` com `done=false` | Operação pendente, sem declarar download concluído |
| Upload resumível retorna `308` | Preserva posição/identificador de sessão, sem confirmar arquivo |
| Erro `403` com `rateLimitExceeded` | Classifica como limitação de quota, não como erro de permissão |
| `File` sem `md5Checksum` ou `size` | Campos opcionais permanecem ausentes; nenhuma assinatura inventada |
| Renomeação por `PATCH` | Não envia conteúdo binário |
| Exclusão por lixeira versus permanente | Contrato distingue explicitamente os dois efeitos |
