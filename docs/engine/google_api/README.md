# Google API

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
| Autenticação | `accounts/auth/` conduz o fluxo de contas e persiste credenciais; a `google_api/` conhece o protocolo e os serviços Google |
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
