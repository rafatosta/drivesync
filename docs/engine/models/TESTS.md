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

## 3. RemoteFile e ProviderFileData

| Caso | Atributo | Verificação |
| --- | --- | --- |
| MOD-REM-01 | `reference` | Identidade é qualificada por conta; não confundir ID do recurso com `FileId` |
| MOD-REM-02 | `provider` | Mantém o serviço da conta e corresponde ao envelope especializado quando presente |
| MOD-REM-03 | `name` | Preserva texto retornado sem normalização destrutiva |
| MOD-REM-04 | `kind` | Inclui `ProviderNative` e `Unknown` sem classificação inventada |
| MOD-REM-05 | `mime_type` | Ausência não produz tipo MIME padrão fictício |
| MOD-REM-06 | `size_bytes` | `None` permitido para nativos/ausentes; zero verdadeiro permanece `Some(0)` |
| MOD-REM-07 | `modified_at` | Preserve timestamp conhecido; não invente data para valor ausente |
| MOD-REM-08 | `version` | String é opaca, não se ordena numericamente entre provedores |
| MOD-REM-09 | `hash` | Nenhum hash inventado quando provedor não o fornece |
| MOD-REM-10 | `parent_ids` | `Unknown` não equivale a `Known([])` |
| MOD-REM-11 | `provider_data` | `None` válido; `Some` preserva metadados opacos com identidade coerente |
| MOD-PRO-01 | `ProviderFileData.provider` | Deve coincidir com `RemoteFile.provider` |
| MOD-PRO-02 | `format`, `format_version` | Formato não vazio e versão positiva |
| MOD-PRO-03 | `payload` | Preservar bytes de metadados sem interpretação em Models; jamais credencial ou conteúdo real do arquivo |
| MOD-PRO-04 | formato e limite | Teste somente após tamanho máximo e codificação **serem aprovados**; enquanto isso `BLOCKED` |

## 4. FileData e File

| Caso | Atributo/relacionamento | Verificação |
| --- | --- | --- |
| MOD-DAT-01 | `FileData::Local` | Encapsula apenas um `LocalFile`, com seus dados |
| MOD-DAT-02 | `FileData::Remote` | Encapsula apenas um `RemoteFile`, com seus dados |
| MOD-DAT-03 | variantes | Não existe estado `Both` ou enum de provedor dentro de `FileData` |
| MOD-FIL-01 | `File.id` | ID da ocorrência estável entre tentativas, distinto da identidade física |
| MOD-FIL-02 | `File.sync_id` | Um evento pertence a exatamente uma sincronização |
| MOD-FIL-03 | `File.event` | `ObservedChange`, `RecheckRequired`, `ScopeUnavailable` são eventos, não comandos de exclusão |
| MOD-FIL-04 | `File.observed_at` | Tempo de observação não substitui `LocalFile.modified_at`/`RemoteFile.modified_at` |
| MOD-FIL-05 | `File.coverage` | `Complete`, `Partial`, `Unavailable` mantêm semântica de cobertura |
| MOD-FIL-06 | `File.scope` | `Local(path)`, `Remote(ref)`, `WholeSync` mantêm suas identidades |
| MOD-FIL-07 | `File.data` | `Some(Local)` exige escopo Local correspondente; `Some(Remote)` exige escopo Remote correspondente |
| MOD-FIL-08 | `File.data=None` | Só admite `RecheckRequired` ou `ScopeUnavailable`, sem fingir exclusão |
| MOD-FIL-09 | `FileEvent::ObservedChange` | Exige `data=Some` |
| MOD-FIL-10 | `FileEvent::ScopeUnavailable` | Exige `coverage=Unavailable` e `data=None` |
| MOD-FIL-11 | `FileScope::WholeSync` | Somente permitido sem `data` |
| MOD-FIL-12 | `File` autocontido | Preserva dados observados sem precisar consultar novamente o Detector; não garante estado atual |

## 5. Integração externa e critérios de aceite

| Caso | Proprietário | Contrato a testar em sua suíte |
| --- | --- | --- |
| MOD-EXT-01 | Detector | `FileBatch.files` transporta `Vec<File>` completos mesmo quando há um único elemento |
| MOD-EXT-02 | Detector Storage | A notificação persistida conserva `File.id` e todo seu conteúdo em retry |
| MOD-EXT-03 | Orquestrador/Analisador | Orquestrador não interpreta `FileData`; Analisador pode distinguir variantes |
| MOD-EXT-04 | Inventário | Revisão futura do contrato `SyncItem`, sem modificar Models antecipadamente |

**Esses quatro casos são pontos de integração, não testes unitários do módulo Models.**

A implementação futura exige: aprovação dos atributos; resolução das pendências declaradas em INTERFACES; testes de entrada/saída, opcionais e invariantes; relatório PASS/FAIL/BLOCKED e execução no ambiente Flatpak. **Nenhum teste foi executado nesta alteração documental.**
