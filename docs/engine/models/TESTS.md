# Models — TESTS: matriz baseada nos atributos propostos

## Estado

O escopo é **conceitual** e a especificação de atributos em [INTERFACES.md](INTERFACES.md) **aguarda aprovação**. Os cenários abaixo são a tradução verificável **dos campos efetivamente documentados**, não testes implementados nem executados. Não criar métodos extras, regras adicionais ou formatos definitivos fora do contrato.

Os testes de `models/` são **puros**: sem rede, filesystem real, SQLite, serviços Google, InventoryManager, Orquestrador ou operação de sincronização.

## 1. Identidades e tipos auxiliares

| Caso | Atributos/objeto | Sucesso esperado | Rejeição/invariante |
| --- | --- | --- | --- |
| MOD-ID-01 | `RemoteFileRef { account_id, file_id }` | Dois arquivos com mesmo ID bruto em contas diferentes são distintos | ID remoto sozinho não identifica recurso global |
| MOD-ID-02 | `File.id: FileId` | Mesmo evento reentregue mantém seu ID; nova ocorrência recebe novo ID | Retry não gera nova identidade |
| MOD-ID-03 | `AccountId`, `SyncId`, `LocalFileId` | Um único tipo proprietário, reutilizado/reexportado | Nenhum tipo homônimo incompatível |
| MOD-ID-04 | `Timestamp.unix_millis` | Preserva valor em UTC e precisão expressa em ms | Não converter `None` de mtime em relógio atual |
| MOD-ID-05 | `FileHash` | Retém algoritmo + digest hexadecimal válido | Não declarar equivalência entre algoritmos distintos |
| MOD-ID-06 | `LocalRelativePath` | Raiz `[]`; nomes Unix não UTF-8 preservados byte a byte | Rejeitar NUL, caminho absoluto, `.` e `..` |
| MOD-ID-07 | `FieldValue<T>` | `Unknown`, `Known([])` e `Known([id])` mantêm significados distintos | Não transformar `Unknown` em `Known([])` |
| MOD-ID-08 | `FileKind` | Cada variante mantém significado, incluindo `Unknown` | Não adivinhar `Regular` pelo nome/extensão |

## 2. LocalFile — cenários para metadados reais Linux

| Caso | Campo/grupo | Critério verificável |
| --- | --- | --- |
| MOD-LOC-01 | `local_id`, `relative_path` | Caminho byte-exato, sem escapes; ID opcional não vira inode |
| MOD-LOC-02 | `kind` | Regular, diretório, symlink, FIFO, socket, devices, outro, desconhecido |
| MOD-LOC-03 | `mode` | Bits brutos, rwx, setuid, setgid e sticky preservados |
| MOD-LOC-04 | `owner` | UID e GID reais, inclusive zero, sem resolver nomes |
| MOD-LOC-05 | `times` | atime, mtime, ctime, btime; nanossegundos e ausência diferenciados; ctime não é criação |
| MOD-LOC-06 | `storage` | Tamanho lógico, blocos 512 B, bloco preferido, nlink, inode, dev e rdev independentes |
| MOD-LOC-07 | `mount_id`, `subvolume_id` | Ausência ou suporte explícito, sem valor fabricado |
| MOD-LOC-08 | `flags` | Preservar statx_attributes, máscara e flags filesystem sem misturá-las |
| MOD-LOC-09 | `direct_io`, `atomic_write` | Todos os limites/alinhamentos preservados ou identificados como não suportados |
| MOD-LOC-10 | `xattrs.entries` | Nomes e bytes de valores preservados; Known([]) somente após enumeração vazia |
| MOD-LOC-11 | `LocalXattr.value` | Falha de leitura de um xattr preserva nome e motivo |
| MOD-LOC-12 | `acl` | ACL acesso/padrão com tags e máscara, conforme suporte e aplicabilidade |
| MOD-LOC-13 | `symlink.target` | Bytes do link preservados, sem seguir alvo |
| MOD-LOC-14 | `extra_metadata` | Nome, namespace/driver e bytes mantidos sem reinterpretar |
| MOD-LOC-15 | `MetadataValue<T>` | Distingue Known(0), Known([]), NotCollected, Unsupported, PermissionDenied, Unavailable |
| MOD-LOC-16 | modelo puro | Nenhum acesso ao filesystem, SQLite, API ou conteúdo para construir/validar objeto |
| MOD-LOC-17 | consistência | Coletas parciais e concorrentes não são declaradas fotografias atômicas completas |
| MOD-LOC-18 | confidencialidade | Xattrs/ACL/labels sensíveis não são automaticamente enviados a logs ou nuvem |
| MOD-LOC-19 | metadados derivados | Não incluir hash de conteúdo nem MIME inferido como dado nativo de statx |

A suíte **Models** valida os tipos e invariantes, sem executar syscalls. A suíte **Detector Local** deve validar a coleta física com `statx`, APIs de xattr/ACL e `readlink` em filesystem temporário, respeitando as permissões da execução Flatpak. Casos de recursos não suportados devem ser tratados como resultados explícitos, não falhas do teste.

## 3. RemoteFile — identidade, metadados e vínculo com provedor

| Caso | Atributo/contrato | Teste conceitual |
| --- | --- | --- |
| MOD-REM-01 | `reference` | ID é associado à conta; mesmo ID bruto em contas distintas não colide |
| MOD-REM-02 | `provider` | Serviço coincide com provedor da conta e do envelope |
| MOD-REM-03 | `identity` | Nome, descrição e nome original conhecidos ou individualmente não coletados |
| MOD-REM-04 | `classification` | Tipos regulares, pasta, nativo e atalhos não são confundidos |
| MOD-REM-05 | `location.parent_ids` | `Known([])` é diferente de `NotCollected`; IDs pais não viram caminhos inventados |
| MOD-REM-06 | `location.container_id` | Suporte a drive compartilhado sem presumir que todo provedor usa driveId |
| MOD-REM-07 | `times` | Datas remotas preservadas sem substituição por instante atual |
| MOD-REM-08 | `content` | Tamanho, quota, versão opaca, revisão e hashes não inventados |
| MOD-REM-09 | `state` | Trashed/explicitlyTrashed/Shared etc. não são presumidos falsos por ausência |
| MOD-REM-10 | `access` | Capacidades só são Known se equivalência e evidência forem válidas; canMove não simplifica capacidades distintas de modo falso |
| MOD-REM-11 | `presentation` | Links preservados quando disponibilizados; ausência não produz URL falsa |
| MOD-REM-12 | `provider_data` | `NotCollected` distingue payload não obtido de payload conhecido; envelope mantém ID do provedor |
| MOD-PRO-01 | `ProviderFileData.provider` | Coerente com RemoteFile e provedor da conta |
| MOD-PRO-02 | `format` e `format_version` | Formato conhecido/versionado; versão desconhecida não é decodificada silenciosamente |
| MOD-PRO-03 | `payload` | Preserva metadados escolhidos do recurso original, sem conteúdo do arquivo nem credenciais |
| MOD-PRO-04 | `GoogleFile` parcial | Mantém distinção entre valor recebido, false, zero, lista vazia e campo não solicitado |
| MOD-PRO-05 | `GoogleFile.id` ausente | Não criar RemoteFileRef fictício; sinalizar reavaliação sem dados do arquivo |
| MOD-PRO-06 | `GoogleChange.removed` | Remoção ou perda de acesso não vira deleção confirmada |
| MOD-PRO-07 | renomeação/movimento | Identidade por conta + ID é preservada, embora nome/pais mudem |
| MOD-PRO-08 | atalhos/itens nativos | Preservar target/formatos específicos sem converter a arquivo comum por hipótese |
| MOD-PRO-09 | recursos sensíveis | Não incluir tokens, wrapped keys, usuários e geo em logs/filas por padrão sem revisão |
| MOD-PRO-10 | formato, tamanho e política | BLOCKED até serem formalizadas codificação, limites e retenção do envelope |
| MOD-REM-13 | evolução de esquema | Detector Remoto permanece inalterado quando adaptador ganha/perde atributo |
| MOD-REM-14 | independência | Models não importa GoogleFile, cliente API, detector ou inventário |

**Responsabilidade dos testes:** Models valida dados puros e invariantes; o **adaptador Google** testa mapeamento fiel de `GoogleFile`, campos opcionais, filtros, codificação e sanitização; o Detector Remoto testa polling/cursor/entrega usando adaptador fake; o Storage testa a persistência integral da observação.

**Pré-requisito de implementação:** aprovar matriz por campo **manter / sob demanda / descartar** e regras de segurança. Estes são testes especificados para o futuro, não executados.

## 4. FileData, FileEvent e File

| Caso | Objeto/campo | Verificação |
| --- | --- | --- |
| MOD-DAT-01 | `FileData::Local` | Encapsula exatamente um `LocalFile` sem perda de metadados |
| MOD-DAT-02 | `FileData::Remote` | Encapsula exatamente um `RemoteFile` sem perda de metadados |
| MOD-DAT-03 | `FileData` | Somente variantes `Local` e `Remote`; Orquestrador não interpreta variante |
| MOD-FIL-01 | `File.id` | ID da ocorrência estável entre retries, distinto do ID de arquivo |
| MOD-FIL-02 | `File.sync_id` | Cada ocorrência pertence à sincronização correta |
| MOD-FIL-03 | `File.event.kind` | Oito variantes: Created, Modified, Deleted, Renamed, Moved, MetadataChanged, RecheckRequired, ScopeUnavailable |
| MOD-FIL-04 | `File.observed_at` | Instante de observação, não mtime |
| MOD-FIL-05 | `File.coverage` | Complete, Partial e Unavailable mantêm a semântica de escopo |
| MOD-FIL-06 | `File.scope` | Local(path), Remote(ref) ou WholeSync sem inferir exclusão |
| MOD-FIL-07 | `File.data` | `Some(Local)` e `Some(Remote)` exigem escopo compatível |
| MOD-EVT-01 | `Created` | Tem `current` + `data`; `previous=None` não é prova isolada de criação |
| MOD-EVT-02 | `Modified` | Tem `current` + `data`; não determina sozinho comparação de conteúdo |
| MOD-EVT-03 | `Deleted` | Pode conter `data=None` e `previous=Some` com `current=None`; não autoriza exclusão automática |
| MOD-EVT-04 | `Renamed` | Exige `previous`, `current`, `data` e correlação de identidade comprovada |
| MOD-EVT-05 | `Moved` | Exige origem/destino, `data` e correlação de identidade comprovada |
| MOD-EVT-06 | `MetadataChanged` | Tem metadados atuais; não presume conteúdo alterado |
| MOD-EVT-07 | `RecheckRequired` | Aceita `data=None`, escopo conhecido, informações parciais |
| MOD-EVT-08 | `ScopeUnavailable` | Exige `coverage=Unavailable`, `data=None`, sem `current` inventado |
| MOD-EVT-09 | `FileLocation::Local` | Preserva bytes de caminho anterior e atual |
| MOD-EVT-10 | `FileLocation::Remote` | Qualifica ID por conta; parent e name opcionais distinguem desconhecido de observado |
| MOD-EVT-11 | transição | Não misturar localização local/remota entre `previous` e `current` para Move/Rename |
| MOD-EVT-12 | mudança simultânea | Renomear + mover não gera falsa afirmação de exclusividade; inconclusivo usa RecheckRequired |
| MOD-EVT-13 | `WholeSync` | Permitido apenas com `data=None` |
| MOD-EVT-14 | integridade do evento | Reentrega preserva objeto completo; `FileEvent` não executa operação nem decide conflito |

**Regra dos testes:** verificar a estrutura e consistência dos dados em Models; teste de classificação de eventos por watcher/API pertence aos Detectores; teste de confirmação e retry pertence ao Detector/Storage; tomada de decisão pertence ao Analisador.

## 5. Integração externa e critérios de aceite

| Caso | Proprietário | Contrato a testar em sua suíte |
| --- | --- | --- |
| MOD-EXT-01 | Detector | `FileBatch.files` transporta `Vec<File>` completos mesmo quando há um único elemento |
| MOD-EXT-02 | Detector Storage | A notificação persistida conserva `File.id` e todo seu conteúdo em retry |
| MOD-EXT-03 | Orquestrador/Analisador | Orquestrador não interpreta `FileData`; Analisador pode distinguir variantes |
| MOD-EXT-04 | Inventário | Revisão futura do contrato `SyncItem`, sem modificar Models antecipadamente |

**Esses quatro casos são pontos de integração, não testes unitários do módulo Models.**

A implementação futura exige: aprovação dos atributos; resolução das pendências declaradas em INTERFACES; testes de entrada/saída, opcionais e invariantes; relatório PASS/FAIL/BLOCKED e execução no ambiente Flatpak. **Nenhum teste foi executado nesta alteração documental.**
