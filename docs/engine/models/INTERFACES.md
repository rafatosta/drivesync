# Models — INTERFACES: contratos de dados para a engine

## 0. Escopo e interpretação

Esta é a **proposta fechada de atributos** para `LocalFile`, `RemoteFile`, `FileData` e `File`, sujeita à revisão do responsável pelo projeto **antes de ser considerada aprovada para implementação**. O código Rust abaixo é **especificação conceitual tipada**: nomes, variantes, relações, presença e semântica são deliberados. Não descreve código compilado.

`models/` **não expõe uma interface operacional**. Não criar `ModelManager`, serviços de consulta, trait de persistência ou métodos de I/O. Validação pura de valores pode ser acrescentada **somente** para preservar os invariantes explicitamente definidos aqui.

**Proprietários:** `LocalFile`, `RemoteFile`, `FileData` e `File` pertencem a `models/`. `FileBatch`, IDs de lote, leases e confirmações pertencem a `detectors/`. `SyncItem` fica **reservado à revisão do Inventário**; não o introduzir nesta etapa.

## 1. Tipos auxiliares e identidades

```rust
pub struct ProviderId(pub String);
pub struct AccountId(pub String);
pub struct SyncId(pub String);
pub struct RemoteFileId(pub String);
pub struct LocalFileId(pub String);
pub struct FileId(pub String); // identidade única da OBSERVAÇÃO, não do arquivo físico

pub struct RemoteFileRef {
    pub account_id: AccountId,
    pub file_id: RemoteFileId,
}

pub struct Timestamp {
    pub unix_millis: i64, // instante UTC, milissegundos desde Unix epoch
}

pub enum FileKind {
    Regular,
    Directory,
    Symlink,
    ProviderNative,
    Other,
    Unknown,
}

pub enum HashAlgorithm { Md5, Sha256, Other(String) }
pub struct FileHash {
    pub algorithm: HashAlgorithm,
    pub hex_digest: String,
}

// Caminho relativo à raiz local, preservado sem conversão UTF-8 com perda.
// A codificação de transporte é Vec<u8> com separadores Unix '/'.
// Jamais armazena um caminho absoluto ou segmentos "." / "..".
pub struct LocalRelativePath(pub Vec<u8>);

pub enum FieldValue<T> {
    Unknown,   // não observado / resposta parcial / não suportado
    Known(T),  // observado; para listas, Known([]) é lista comprovadamente vazia
}
```

| Tipo | Origem e regra |
| --- | --- |
| `ProviderId` | Identifica o serviço remoto; **não** identifica uma conta |
| `AccountId` | Identidade de conta estável; reutilizar a definição já existente em Rust (não duplicar) |
| `SyncId` | Identifica a raiz de sincronização; deve ser único no escopo da engine |
| `RemoteFileId` | Identidade opaca retornada pelo serviço; comparar **junto com** a conta |
| `RemoteFileRef` | Identidade remota qualificada por conta |
| `LocalFileId` | Identidade local estável **somente se comprovada** pelo produtor; não confundir inode ou caminho com identidade da engine |
| `FileId` | Identidade **da ocorrência encapsulada**; várias ocorrências distintas do mesmo arquivo possuem `FileId` distintos, mas reentregas da mesma ocorrência preservam o mesmo `FileId` |
| `Timestamp` | Instante normalizado UTC em ms; o produtor preserva precisão real quando conhecida e não inventa instante de modificação |
| `FileHash` | Hexadecimal de digest + algoritmo; comparar apenas hashes de mesmo algoritmo e escopo |
| `FieldValue<T>` | Diferencia **não observado** de **observado e vazio**; não significa atualização ou comando de exclusão |

`FileId` deve ser preservado **idêntico durante retries da mesma ocorrência**. Reobservação posterior é nova ocorrência, salvo deduplicação explícita da origem comprovando ser a mesma. `AccountId`/ `SyncId`/ `LocalFileId` não são reimplementados para satisfazer este documento: ao integrar, manter apenas um proprietário e reexportar o tipo existente quando necessário.

### Aviso pré-implementação: inventariar tudo, selecionar depois

**BLOQUEIO DE IMPLEMENTAÇÃO:** os campos abaixo constituem um **inventário abrangente dos metadados conhecidos**, não uma ordem para coletar e persistir todos eles.

A estratégia acordada é **considerar primeiro todos os dados disponíveis** e, depois de analisar necessidades reais da engine, **descartar deliberadamente os que não agregam valor**. Antes de iniciar a implementação de `LocalFile`, revisar **campo por campo**:

1. Para qual decisão ou operação concreta do DriveSync o dado será necessário?
2. O dado existe no filesystem/plataforma usada? Qual é a fonte e o custo de obtê-lo?
3. Há risco de privacidade, segurança, tamanho, desempenho ou incompatibilidade?
4. Deve ser coletado sempre, somente sob demanda, ou excluído do contrato final?
5. Se necessário, precisa seguir dentro de `File` até o Analisador, ser persistido pelo Inventário, ou apenas consultado temporariamente?

Registrar **manter / sob demanda / descartar**, com justificativa. Somente após essa revisão aprovar a **estrutura final implementável**, atualizar testes e alinhar os contratos consumidores. **Não interpretar a documentação detalhada a seguir como autorização para implementá-la literalmente antes da revisão.**

## 2. LocalFile — metadados reais completos do Linux

### 2.1. Escopo

`LocalFile` representa **os metadados reais da entrada do filesystem, com a maior fidelidade que o Linux permite**, e não apenas os campos mínimos necessários à sincronização. O tipo é **somente dados**: o Detector Local executa consultas e preenche o modelo; Models não faz I/O.

Não existe uma syscall que entregue tudo: `statx` cobre o núcleo; `readlink`, `listxattr/getxattr`, APIs POSIX ACL e consultas específicas do filesystem cobrem outras categorias. Alguns atributos podem ser inacessíveis, não suportados ou ainda não consultados. Portanto, **um objeto completo contém os campos e os respectivos estados de coleta, não necessariamente um valor para cada campo**.

### 2.2. Contrato conceitual tipado

```rust
pub enum MetadataValue<T> {
    Known(T),
    NotCollected,
    Unsupported,
    PermissionDenied,
    Unavailable,
}

pub enum LocalEntryKind {
    Regular, Directory, Symlink, Fifo, Socket,
    BlockDevice, CharacterDevice, Other, Unknown,
}

pub struct LocalTimestamp {
    pub unix_seconds: i64,
    pub nanoseconds: u32, // 0..=999_999_999
}
pub struct DeviceNumber { pub major: u32, pub minor: u32 }

pub struct LocalFileMode {
    pub raw_mode: u32, // tipo + bits reportados, sem perdas
    pub permission_bits: u16, // rwx, setuid, setgid, sticky
}
pub struct LocalOwner {
    pub uid: MetadataValue<u32>,
    pub gid: MetadataValue<u32>,
}
pub struct LocalTimes {
    pub accessed_at: MetadataValue<LocalTimestamp>, // atime
    pub modified_at: MetadataValue<LocalTimestamp>, // mtime
    pub changed_at: MetadataValue<LocalTimestamp>,  // ctime: mudança de status, NÃO criação
    pub created_at: MetadataValue<LocalTimestamp>,  // btime, se disponível
}
pub struct LocalStorage {
    pub size_bytes: MetadataValue<u64>,             // tamanho lógico
    pub allocated_blocks_512b: MetadataValue<u64>,  // blocos de 512 bytes
    pub preferred_block_size: MetadataValue<u32>,
    pub hard_link_count: MetadataValue<u64>,
    pub inode: MetadataValue<u64>,
    pub filesystem_device: MetadataValue<DeviceNumber>,
    pub represented_device: MetadataValue<DeviceNumber>, // rdev em device nodes
    pub mount_id: MetadataValue<u64>,
    pub subvolume_id: MetadataValue<u64>,
}
pub struct LocalFileFlags {
    pub statx_attributes: MetadataValue<u64>,
    pub statx_attributes_mask: MetadataValue<u64>,
    pub filesystem_flags: MetadataValue<u64>, // consulta específica quando disponível
}
pub struct LocalDirectIo {
    pub memory_alignment: MetadataValue<u32>,
    pub offset_alignment: MetadataValue<u32>,
    pub read_offset_alignment: MetadataValue<u32>,
}
pub struct LocalAtomicWrite {
    pub unit_min: MetadataValue<u32>,
    pub unit_max: MetadataValue<u32>,
    pub segments_max: MetadataValue<u32>,
    pub unit_max_opt: MetadataValue<u32>,
}
pub struct LocalXattr {
    pub name: Vec<u8>, // namespace e nome completos, sem transformação com perda
    pub value: MetadataValue<Vec<u8>>,
}
pub struct LocalXattrs {
    pub entries: MetadataValue<Vec<LocalXattr>>,
}
pub enum LocalAclTag {
    OwnerUser, NamedUser(u32), OwnerGroup, NamedGroup(u32),
    Mask, Other,
}
pub struct LocalAclEntry {
    pub tag: LocalAclTag,
    pub permission_bits: u8,
}
pub struct LocalAcl { pub entries: Vec<LocalAclEntry> }
pub struct LocalAcls {
    pub access: MetadataValue<LocalAcl>,
    pub default: MetadataValue<LocalAcl>, // aplicável a diretórios
}
pub struct LocalSymlink {
    pub target: MetadataValue<Vec<u8>>, // readlink, sem seguir o alvo
}
pub struct FilesystemMetadata {
    pub namespace: String,  // FS/driver/família de atributo
    pub name: String,
    pub value: Vec<u8>,
}
pub struct LocalFile {
    pub local_id: Option<LocalFileId>,
    pub relative_path: LocalRelativePath,
    pub kind: MetadataValue<LocalEntryKind>,
    pub mode: MetadataValue<LocalFileMode>,
    pub owner: LocalOwner,
    pub times: LocalTimes,
    pub storage: LocalStorage,
    pub flags: LocalFileFlags,
    pub direct_io: LocalDirectIo,
    pub atomic_write: LocalAtomicWrite,
    pub xattrs: LocalXattrs,
    pub acl: LocalAcls,
    pub symlink: LocalSymlink,
    pub extra_metadata: MetadataValue<Vec<FilesystemMetadata>>,
}
```

### Regra estrutural de evolução — independência do Detector

**Um novo campo em `LocalFile` não autoriza alterar o Detector Local.** O Detector utiliza apenas um contrato de coleta estável: entrega referência/escopo do arquivo e recebe `LocalFile` preenchido conforme os metadados disponíveis. Não deve usar inicialização literal de structs, copiar atributos individualmente, depender de sua lista de campos ou chamar diretamente rotinas de `statx`, ACL ou xattr.

O componente **coletor de metadados locais**, externo ao núcleo puro Models e isolado da lógica do Detector, será o responsável por criar/preencher `LocalFile`. Suas **funções auxiliares** poderão coletar categorias adicionais em etapas, sob demanda e com estado explícito (`Known`, `NotCollected`, etc.). Adicionar ou remover campos afeta esse coletor e os contratos de dados correspondentes, **não** o mecanismo de detecção, acompanhamento e emissão de notificações.

A assinatura interna de coleta ficará definida em [Detector Local — INTERFACES](../detectors/local/INTERFACES.md). Essa separação não dispensa a **revisão de utilidade dos metadados antes de implementar**.

### 2.3. Relação entre atributos e fontes

| Grupo | Dados reais | Fonte e observação |
| --- | --- | --- |
| Entrada/identidade | `local_id`, `relative_path`, `kind` | Detector, `lstat/statx`; o ID de domínio não é inode |
| Modo | tipo, permissões rwx e setuid/setgid/sticky | `stat/statx`; ACL não é substituída pelo mode |
| Proprietário | UID, GID numéricos | `stat/statx`; nomes de usuários/grupos dependem do ambiente, não são intrínsecos |
| Datas | atime, mtime, ctime, btime | `statx`; segundos e nanos; btime nem sempre disponível |
| Armazenamento | tamanho lógico, blocos alocados, bloco preferido | `statx`; blocos alocados em unidades de 512 B |
| Inode/links | inode, hard links, device do filesystem | `statx`; não implicam identidade estável global |
| Dispositivos especiais | represented device major/minor | `statx`/rdev; não confundir com device do filesystem |
| Montagem/subvolume | mount ID, subvolume ID | `statx`; dependem de kernel/FS |
| Flags | atributos e máscara de suporte; flags especiais | `statx` e APIs específicas; a máscara deve ser preservada |
| Direct I/O | alinhamentos de memória, offset e leitura | `statx` quando disponível |
| Escrita atômica | unidades e segmentos permitidos | `statx` quando disponível |
| Xattrs | pares nome/valor em bytes; namespaces `user.*`, `security.*`, `system.*`, `trusted.*` etc. | `listxattr/getxattr` com semântica de no-follow em symlinks |
| ACLs POSIX | entradas completas de acesso e ACL padrão | APIs ACL; ACL padrão apenas em diretórios |
| Symlink | bytes literais do destino | `readlink`; nunca seguir para fora da raiz |
| Complementos | metadados adicionais identificados por família/driver | consultas específicas; não substituir campos conhecidos por blob genérico |

**Não são metadados intrínsecos do inode:** hash de conteúdo, MIME inferido, imagens em miniatura, tags de mídia e conteúdo do arquivo. A antiga propriedade `LocalFile.hash` é removida deste contrato de **metadados reais**; qualquer análise derivada deve ser especificada separadamente.

### 2.4. Invariantes e limites

1. Em `MetadataValue<T>`, `Known(0)`, `Known(false)` e `Known([])` são valores comprovados; diferem de `NotCollected`, `Unsupported`, `PermissionDenied` e `Unavailable`.
2. `LocalFile` mantém seus campos mesmo quando algum grupo está indisponível. `NotCollected` indica coleta incompleta, **não** representa leitura completa.
3. `statx` usa `stx_mask` para informar quais campos foram preenchidos e `stx_attributes_mask` para suporte das flags. Não considerar valores numéricos sem a respectiva evidência.
4. Se o nome do xattr foi enumerado mas seu valor não pôde ser lido, manter o nome e `value=PermissionDenied` ou `Unavailable`, não `Known([])`.
5. `acl.default` em arquivo não diretório é não aplicável; essa distinção será normalizada sem inventar entradas. `MetadataValue::Unsupported` pode representar esse caso no contrato proposto.
6. A observação pode ocorrer durante mutações concorrentes: consultas múltiplas **não** produzem uma fotografia atômica garantida. O Detector deve sinalizar inconsistência e solicitar reobservação quando necessário.
7. `LocalRelativePath` preserva bytes Unix, não aceita NUL, caminho absoluto, `.` ou `..`; nunca seguir symlink externo.
8. Xattrs, ACLs e labels de segurança podem conter dados sensíveis. Ter esses campos em Models **não autoriza enviá-los ao Google Drive, registrar valores em log ou sincronizar permissões automaticamente**. Persistência/retensão e limite de payload exigem revisão de segurança.
9. `LocalFile` pode registrar todos os metadados disponíveis ao processo, mas **não promete acessar metadados privados do kernel, exclusivos de outro filesystem, bloqueados pelo Flatpak, ou ainda não enumerados**.
10. Consultas caras e opcionais (por exemplo, extents/FIEMAP) e análise do conteúdo não são obrigatórias para produzir cada notificação. Se forem incorporadas, devem ganhar campos/famílias e políticas próprias, não inferências silenciosas.
11. `FileData::Local(LocalFile)` continua com a mesma relação de encapsulamento; `SyncItem` permanece fora da revisão.

### 2.5. Pendências de implementação

- Padrão para `MetadataValue` por campos não aplicáveis, além de `Unsupported`, se for necessário distinguir `NotApplicable`.
- Limites de tamanho e retenção de Xattrs e `extra_metadata`, que podem carregar informações sensíveis.
- Política de coleta: básica/expandida/completa, sem declarar `Complete` quando houver categorias obrigatórias não consultadas.
- Compatibilização com assinaturas já documentadas do Detector Local (modelo anterior de seis atributos).

**Estado: proposta formal de atributos, ainda não implementada nem aprovada como contrato Rust final.**

## 3. RemoteFile — metadados remotos e associação ao provedor

### 3.1. Objetivo, abrangência e identidade

`RemoteFile` é o **modelo genérico de um arquivo originado em um provedor remoto**, com duas camadas complementares:

1. **Atributos normalizados**, em grupos, para que o Analisador e outros módulos possam utilizar informações comuns **sem saber se são do Google Drive ou de outro serviço**.
2. **`ProviderFileData`**, um pacote **opaco, identificado por provedor, formato e versão**, que preserva os metadados especializados do **objeto original do provedor** (por exemplo, um `GoogleFile`), sem importar `GoogleFile` na crate `models/`.

A associação é feita por **`RemoteFileRef(account_id, file_id)` + `ProviderId`**, não por nome/caminho, nome de usuário, MIME ou referência a um cliente HTTP. Renomear ou mover o arquivo **não** deve alterar seu vínculo ao recurso remoto quando o ID remoto continua o mesmo. IDs de provedores/contas diferentes não são intercambiáveis. O `provider_data` não cria uma nova identidade do arquivo.

Esta seção é um **inventário documental abrangente de campos remotos** antes da etapa de seleção. Assim como `LocalFile`, deve-se **levantar todos os metadados disponíveis primeiro e depois decidir manter, coletar sob demanda ou descartar**, por custo, privacidade, suporte e necessidade. **Não implementar automaticamente todos os campos.**

### 3.2. Contrato conceitual de RemoteFile

Os grupos abaixo são uma **proposta conceitual tipada**. A forma precisa de cada grupo, as listas, a serialização e os campos finais exigem aprovação; não são structs Rust compiladas.

```rust
pub struct RemoteFile {
    // Associação estável ao recurso original
    pub reference: RemoteFileRef, // conta + ID remoto
    pub provider: ProviderId,     // Google, OneDrive etc.

    // Metadados comuns, quando efetivamente fornecidos
    pub identity: RemoteIdentityMetadata,
    pub classification: RemoteClassificationMetadata,
    pub location: RemoteLocationMetadata,
    pub times: RemoteTimeMetadata,
    pub content: RemoteContentMetadata,
    pub state: RemoteStateMetadata,
    pub access: RemoteAccessMetadata,
    pub presentation: RemotePresentationMetadata,

    // Snapshot de metadados especializados do provedor, sem conteúdo
    pub provider_data: MetadataValue<ProviderFileData>,
}

pub struct RemoteIdentityMetadata {
    pub name: MetadataValue<String>,
    pub description: MetadataValue<String>,
    pub original_filename: MetadataValue<String>,
}

pub struct RemoteClassificationMetadata {
    pub kind: MetadataValue<FileKind>,
    pub mime_type: MetadataValue<String>,
    pub file_extension: MetadataValue<String>,
    pub full_file_extension: MetadataValue<String>,
}

pub struct RemoteLocationMetadata {
    pub parent_ids: MetadataValue<Vec<RemoteFileId>>,
    pub container_id: MetadataValue<String>, // ex.: driveId, se aplicável
}

pub struct RemoteTimeMetadata {
    pub created_at: MetadataValue<Timestamp>,
    pub modified_at: MetadataValue<Timestamp>,
    pub trashed_at: MetadataValue<Timestamp>,
    pub viewed_at: MetadataValue<Timestamp>,
}

pub struct RemoteContentMetadata {
    pub size_bytes: MetadataValue<u64>,
    pub quota_bytes_used: MetadataValue<u64>,
    pub version: MetadataValue<String>, // opaque; não ordenar entre provedores
    pub revision_id: MetadataValue<String>,
    pub hashes: MetadataValue<Vec<FileHash>>,
}

pub struct RemoteStateMetadata {
    pub trashed: MetadataValue<bool>,
    pub explicitly_trashed: MetadataValue<bool>,
    pub starred: MetadataValue<bool>,
    pub shared: MetadataValue<bool>,
    pub owned_by_me: MetadataValue<bool>,
}

pub struct RemoteAccessMetadata {
    // Somente informações que admitem semântica genérica;
    // estruturas complexas ou específicas permanecem em provider_data.
    pub can_read: MetadataValue<bool>,
    pub can_download: MetadataValue<bool>,
    pub can_edit: MetadataValue<bool>,
    pub can_delete: MetadataValue<bool>,
    pub can_rename: MetadataValue<bool>,
    pub can_move: MetadataValue<bool>,
    pub can_share: MetadataValue<bool>,
}

pub struct RemotePresentationMetadata {
    pub web_view_link: MetadataValue<String>,
    pub web_content_link: MetadataValue<String>,
    pub thumbnail_link: MetadataValue<String>,
    pub icon_link: MetadataValue<String>,
}
```

Os grupos normalizados **não são cópias integrais** do modelo Google: representam conceitos aproveitáveis por múltiplos serviços. Não duplicar em campos genéricos todos os dados de permissões, ACLs remotas, labels, restrições ou atributos de mídia quando a semântica do outro provedor não for equivalente. Esses detalhes ficam no envelope especializado. O Analisador pode usar os campos normalizados e consultar o especialista quando realmente precisar entender dados particulares.

`MetadataValue<T>`, definido na seção de `LocalFile`, distingue `Known(T)`, `NotCollected`, `Unsupported`, `PermissionDenied` e `Unavailable`. Para um campo omitido por `fields` parcial da API, usar `NotCollected`, **não** `Known(false)`, `Known(0)` ou `Known([])`. A presença de um recurso na lixeira não equivale automaticamente a autorização para excluir localmente.

### 3.3. Matriz dos metadados normalizados

| Grupo | Metadados | Origem exemplificada no GoogleFile |
| --- | --- | --- |
| Referência | `reference` e `provider` | `GoogleFile.id` + conta autenticada + provedor `google` |
| Identidade e nome | `name`, `description`, `original_filename` | `name`, `description`, `originalFilename` |
| Tipo | `kind`, MIME e extensões | `mimeType`, `fileExtension`, `fullFileExtension`; tipos nativos e atalhos requerem classificação própria |
| Localização | `parent_ids`, `container_id` | `parents`, `driveId`; parcialidade não significa raiz |
| Datas | criação, modificação, lixeira e último acesso/visualização | `createdTime`, `modifiedTime`, `trashedTime`, `viewedByMeTime` quando disponíveis |
| Conteúdo/versões | tamanho, cota, versão, revisão, hashes | `size`, `quotaBytesUsed`, `version`, `headRevisionId`, MD5/SHA-1/SHA-256 |
| Estado | lixeira, marcado, compartilhado, propriedade | `trashed`, `explicitlyTrashed`, `starred`, `shared`, `ownedByMe` |
| Capacidades comuns | leitura/download/edição/exclusão/renomeação/movimentação/compartilhamento | `capabilities` **quando fornecidas**; não inferir `can_read` só porque uma resposta chegou |
| Links de apresentação | abrir no navegador, conteúdo, miniatura e ícone | `webViewLink`, `webContentLink`, `thumbnailLink`, `iconLink` |
| Especialização | capacidades completas, permissões, atalhos, labels, metadata de mídia e políticas | `provider_data` com snapshot tipado do recurso de origem |

**Observação de precisão:** campos com nomes semelhantes podem ter semânticas diferentes entre provedores. A normalização somente é permitida se a equivalência for demonstrada; caso contrário, deixar `Unsupported`/`NotCollected` e preservar o valor original no `provider_data`. `can_move` é propositalmente conceitual: no Google há capacidades de mover **dentro** e **para fora** do drive, que não devem ser reduzidas a um único booleano sem definir a política; não fabricar `Known(true/false)`.

### 3.4. ProviderFileData — vínculo fiel ao objeto do provedor

```rust
pub struct ProviderFileData {
    pub provider: ProviderId,
    pub format: String,
    pub format_version: u32,
    pub payload: Vec<u8>,
}
```

| Campo | Função | Regra |
| --- | --- | --- |
| `provider` | Identifica quem interpreta o conteúdo | Igual a `RemoteFile.provider` e ao provedor da conta de `reference` |
| `format` | Identifica o esquema de metadados transportados | Por exemplo `google.drive.file-metadata` (identificador ilustrativo, não formato aprovado) |
| `format_version` | Identifica a versão do esquema | Nunca assumir que uma versão desconhecida é decodificável |
| `payload` | Bytes de um snapshot **filtrado de metadados do recurso do provedor** | Não transportar conteúdo binário do arquivo, tokens, sessões, credenciais ou objetos de rede |

#### Como um GoogleFile é associado

```text
providers/google/api/
└── GoogleFile (objeto da API, campos opcionais)
    │
    ▼ especialista/adaptador Google
    ├── valida id da resposta + conta provedora
    ├── preenche RemoteFile.reference (account_id + file_id)
    ├── normaliza campos comprovadamente observados
    └── codifica metadados especializados em ProviderFileData
         ├── provider: google
         ├── format: esquema do snapshot Google
         ├── format_version: versão
         └── payload: metadados filtrados e identificáveis
    │
    ▼ RemoteFile
       ├── reference + provider
       ├── identity / classification / location / times ...
       └── provider_data
    │
    ▼ FileData::Remote(RemoteFile)
       ▼ File (ocorrência + evento)
       ▼ FileBatch (Detector)
       ▼ Orquestrador
       ▼ Analisador (interpreta origem/ocorrência)
```

**Associação reversível de metadados, não de capacidade de acesso:** um especialista Google pode interpretar o `ProviderFileData` de formato conhecido, identificando o `GoogleFile` (ou a projeção sanitizada equivalente) que o originou. O `RemoteFile` guarda a **identidade do recurso**; o especialista usa a conta para recuperar autenticação quando for executar uma API — nunca usa tokens extraídos do payload.

**Fidelidade:** se `GoogleFile` foi fornecido por resposta parcial, o adaptador preserva a distinção entre campo não solicitado e campo conhecido. O `payload` não precisa carregar todos os campos da API, mas os metadados **efetivamente coletados e selecionados para retenção** devem manter identidade, semântica, tipos, valores e informação de presença, incluindo listas vazias e `false`. Uma conversão de volta não pode alegar reconstruir o `GoogleFile` completo se houve filtragem/perda.

**Resolução da identidade:** `GoogleFile.id` pode vir ausente em resposta parcial. Sem ID real **não criar `RemoteFileRef` fictício** nem usar nome/caminho como substituto; o especialista gera uma observação de reavaliação `File.data=None` com o melhor escopo conhecido. `GoogleChange.removed=true` também pode indicar **perda de acesso**, sem metadados; não construir `RemoteFile` vazio nem concluir exclusão automática.

### 3.5. Campos específicos conhecidos no GoogleFile

O contrato já existente em [Google API — INTERFACES](../providers/google/api/INTERFACES.md#91-estruturas-de-saída-e-metadados) lista os campos do recurso `GoogleFile`. Exemplos de categorias que **devem ser consideradas na revisão**, sem forçá-las ao núcleo normalizado:

| Categoria do provedor | Campos conhecidos na API Google |
| --- | --- |
| Atalhos | `shortcutDetails.targetId`, `targetMimeType`, `targetResourceKey` |
| Capacidades detalhadas | `capabilities.canEdit`, `canCopy`, `canDownload`, `canDelete`, `canRename`, `canMoveItemWithinDrive`, `canMoveItemOutOfDrive`, `canAddChildren`, `canListChildren`, `canShare`, `canReadRevisions` etc. |
| Permissões/compartilhamento | `permissions`, `permissionIds`, `owners`, `lastModifyingUser`, `sharingUser`, `writersCanShare`, `copyRequiresWriterPermission`, herança |
| Organização | `driveId`, `teamDriveId`, `spaces`, `folderColorRgb` |
| Propriedades e indexação | `properties`, `appProperties`, `contentHints` |
| Metadados de mídia | `imageMediaMetadata`, `videoMediaMetadata`, incluindo localizações de foto quando disponíveis |
| Labels | `labelInfo` e campos com valores próprios |
| Restrições/segurança | `contentRestrictions`, `downloadRestrictions`, `inheritedPermissionsDisabled`, `clientEncryptionDetails` |
| Outros | `resourceKey`, `modifiedByMeTime`, `hasThumbnail`, `ownedByMe` etc. |

**Riscos:** `resourceKey`, permissões, usuários, metadados de localização e detalhes de criptografia podem ser sensíveis. Não armazenar payload bruto com dados sensíveis sem revisão de necessidade, limpeza, retenção e controle de acesso. Em especial, **não copiar `wrappedKey` ou outras informações de descriptografia para a fila do Detector apenas porque aparecem no objeto da API**. A presença do campo no catálogo não é autorização para coletar ou persistir.

### 3.6. Isolamento do Detector Remoto e coleta especializada

**Alterações nos campos de `RemoteFile` não podem exigir modificações na lógica do Detector Remoto, na fachada de Detector ou no Orquestrador.** Assim como `LocalFile`, o Detector utiliza uma **interface estável de adaptação/coleta de metadados** e encaminha `RemoteFile` como valor opaco, sem inicializá-lo campo a campo nem inspecionar propriedades para entregar notificações.

O **adaptador especializado do provedor**, por exemplo o adaptador Google, sabe traduzir `GoogleFile` para `RemoteFile`, preencher grupos genéricos, formar `ProviderFileData` e consultar dados adicionais por rotinas auxiliares quando necessário. A interface de integração concreta pertence ao [Detector Remoto](../detectors/remote/INTERFACES.md), e **não** a um serviço dentro de `models/`.

Revisões de atributos podem alterar o adaptador, os testes e consumidores específicos que realmente utilizam os campos, mas **não** o mecanismo de polling, cursor, scan, fila ou notificações. Se uma classificação de evento exigir comparar metadados, usar uma porta especializada responsável pela classificação, sem acoplar o Detector ao esquema de `RemoteFile`.

### 3.7. Revisão obrigatória antes da implementação

**NÃO IMPLEMENTAR A LISTA INTEGRAL AUTOMATICAMENTE.** A estratégia é primeiro catalogar todos os metadados disponíveis no provedor e **somente depois descartar ou coletar sob demanda aqueles que não precisam participar do fluxo**.

Para cada grupo/campo, registrar: finalidade para sincronização, origem/capacidade do provedor, custo e escopos de API, caráter sensível, necessidade de retenção e destinatário (`File`, Inventário, especialista ou consulta pontual). Concluir com **manter / sob demanda / descartar** e justificativa.

Não implementar `RemoteFile` e `ProviderFileData` como contrato literal antes de decidir codificação, versão, limite de payload, sanitização e compatibilidade de identidades. **`SyncItem` permanece reservado à revisão posterior do Inventário.**

## 4. FileData — uma variante, nenhum comportamento

```rust
pub enum FileData {
    Local(LocalFile),
    Remote(RemoteFile),
}
```

| Variante | Conteúdo | Significado |
| --- | --- | --- |
| `Local(LocalFile)` | Um `LocalFile` completo nos campos observados | A observação foi produzida a partir do filesystem |
| `Remote(RemoteFile)` | Um `RemoteFile` completo nos campos observados | A observação foi produzida a partir de um serviço remoto |

**Exclusividade:** exatamente uma variante por `FileData`; não existem `Both`, `Unknown`, `Google` nem `OneDrive`. `FileData` não contém tipo de evento, status de entrega, histórico ou decisão de sincronização.

## 5. File — observação autocontida e atributos do evento

`File` é o **envelope unitário** que chega ao Orquestrador, sem obrigá-lo a conhecer se a origem é local ou remota. O Analisador recebe o mesmo `File`, identifica `FileData` e avalia a mudança em conjunto com o estado persistido. **O evento é uma classificação da observação, não uma operação de sincronização autorizada.**

### 5.1 Tipos de evento

```rust
pub enum FileEventKind {
    Created,
    Modified,
    Deleted,
    Renamed,
    Moved,
    MetadataChanged,
    RecheckRequired,
    ScopeUnavailable,
}
```

| Variante | Sinal observado | Evidência/contexto necessário |
| --- | --- | --- |
| `Created` | Item novo observado | Localização atual e `FileData`, quando confirmáveis |
| `Modified` | Conteúdo ou versão modificada | Referência do item e metadados atuais; comparação final cabe ao Analisador |
| `Deleted` | Indício de remoção | Última localização/identidade conhecida; pode não haver dados atuais |
| `Renamed` | Nome mudou | Localização/nome anterior e atual **quando a correlação for segura** |
| `Moved` | Diretório/pai/caminho mudou | Origem e destino, com identidade correlacionada |
| `MetadataChanged` | Metadados mudaram sem comprovar alteração do conteúdo | Metadados observados e identidade conhecida |
| `RecheckRequired` | Sinal ambíguo, dados insuficientes ou reobservação necessária | Melhor escopo possível; não inventar mudança específica |
| `ScopeUnavailable` | Escopo não acessível | Qual escopo falhou; não equivale a exclusão |

Renomear e mover podem ocorrer juntos; **a classificação de um único evento não significa que apenas um aspecto mudou**. Quando não for possível correlacionar com segurança antes/depois, usar `RecheckRequired` em vez de forçar `Renamed` ou `Moved`. O contrato não tenta ser um histórico completo de todas as operações intermediárias.

### 5.2 Localizações e contexto anterior/atual

```rust
pub enum FileLocation {
    Local(LocalRelativePath),
    Remote {
        reference: RemoteFileRef,      // identidade do arquivo, não localização por si só
        parent_id: Option<RemoteFileId>,
        name: Option<String>,
    },
}

pub struct FileEvent {
    pub kind: FileEventKind,
    pub previous: Option<FileLocation>,
    pub current: Option<FileLocation>,
}

pub enum ObservationCoverage { Complete, Partial, Unavailable }

pub enum FileScope {
    Local(LocalRelativePath),
    Remote(RemoteFileRef),
    WholeSync,
}
```

- `previous` contém a última **localização anterior conhecida pelo produtor**, não uma cópia completa dos metadados do Inventário. `None` significa que não houve referência anterior confiável; **não** prova criação.
- `current` contém a localização observada **após** a ocorrência, quando conhecida. `None` não é prova automática de exclusão.
- Em `FileLocation::Remote`, somente o ID remoto não identifica mudança de nome/pasta: `parent_id` e `name` descrevem a localização **quando disponíveis**. `None` nesses campos significa desconhecido, não ausência comprovada.
- `FileScope` identifica o recurso/escopo que pode precisar de reavaliação. `FileLocation` guarda a informação **antes/depois** de um evento. São conceitos relacionados, mas não duplicações: `scope` permanece útil mesmo sem localizações completas.
- `FileEvent` não contém `LocalFile` nem `RemoteFile`; o objeto `File.data` contém os metadados observados.

### 5.3 File

```rust
pub struct File {
    pub id: FileId,
    pub sync_id: SyncId,
    pub event: FileEvent,
    pub observed_at: Timestamp,
    pub coverage: ObservationCoverage,
    pub scope: FileScope,
    pub data: Option<FileData>,
}
```

| Campo | Tipo | Obrigatório? | Origem e interpretação |
| --- | --- | --- | --- |
| `id` | `FileId` | Sim | Identifica **a ocorrência**, não o arquivo; permanece idêntico em retries |
| `sync_id` | `SyncId` | Sim | Escopo da sincronização |
| `event` | `FileEvent` | Sim | Classificação da ocorrência + localizações observadas |
| `observed_at` | `Timestamp` | Sim | Instante da observação; distinto de `modified_at` |
| `coverage` | `ObservationCoverage` | Sim | Completude da observação sobre o escopo, não prova de exclusão |
| `scope` | `FileScope` | Sim | Recurso/escopo a reavaliar, inclusive sem arquivo observável |
| `data` | `Option<FileData>` | Campo sim; valor não | Metadados locais ou remotos quando observados; `None` não comprova ausência |

### 5.4 Invariantes e combinações admitidas

1. `data=Some(Local(x))` exige `scope=Local(x.relative_path)`; `data=Some(Remote(x))` exige `scope=Remote(x.reference)`. `WholeSync` só é válido sem `data`.
2. `Created`, `Modified`, `Renamed`, `Moved` e `MetadataChanged` exigem `data=Some(...)` e `current=Some(...)`, com origem e identidade compatíveis com `data`. Para `Renamed` e `Moved`, `previous=Some(...)` também é obrigatório; não classificar quando a correlação não foi observada.
3. `Deleted` admite `data=None`, `previous=Some(...)` e `current=None`. **A classificação é uma pista de remoção, nunca confirmação para apagar na outra ponta.** Quando só existe um sinal de watcher sem identidade anterior confiável, usar `RecheckRequired`.
4. `RecheckRequired` admite `data=None` e localizações parciais ou desconhecidas; `scope` identifica a reavaliação necessária. Pode também acompanhar dados parciais conhecidos.
5. `ScopeUnavailable` exige `coverage=Unavailable`, `data=None` e não inventa `current` observável. Nenhum erro de permissão ou desmontagem vira `Deleted`.
6. `previous` e `current` precisam pertencer à mesma origem local/remota para `Renamed`/`Moved`; o vínculo de identidade deve ter evidência, não mera semelhança de nomes.
7. `coverage=Complete` descreve a inspeção do **escopo declarado**; por si só não valida ausência ou exclusão. `data=None` também não.
8. `File` **não** contém `SyncItem`, baseline, comando de upload/download, decisão de conflito, lease ou estado de fila.
9. Não executar decisões destrutivas diretamente com base no `FileEventKind`. O Analisador compara observações e estados confiáveis antes de decidir.
10. `File` preserva todos os metadados **efetivamente coletados** sem garantir que o estado físico permaneça igual na hora do processamento.

**Estado:** especificação conceitual para revisão, não implementação Rust. A atribuição exata dos eventos pelos detectores e a integração com Storage/Orquestrador serão detalhadas nos respectivos contratos, sem ampliar o papel de `models/`.

## 6. FileBatch — fronteira externa, propriedade do Detector

`FileBatch` **não é definido em Models**. Seu vínculo conceitual obrigatório é:

```text
FileBatch (detectors/)
└── files: Vec<File> (models/)
```

ID do lote, tamanho máximo, lease, reserva e ack pertencem a [`detectors/INTERFACES.md`](../detectors/INTERFACES.md). O Orquestrador recebe a lista sem interpretar `FileData`; o Analisador diferencia Local/Remote. Entrega imediata pode ter um único `File`; falha exige reentrega da **mesma ocorrência identificada**, sem perda da observação.

## 7. Ausência, observações parciais e integridade

- `Option::None` num atributo de arquivo = valor não conhecido/não disponível; **nunca** significa zero, hash vazio ou exclusão.
- `FieldValue::Unknown` = atributo composto não observado; `Known([])` = vazio realmente observado.
- `File.data=None` = metadados do arquivo não obtidos; o evento carrega motivo + escopo para reavaliação.
- Nenhum evento de remoção de watcher nem flag do provedor autoriza excluir sem análise de identidade, escopo, baseline e cobertura.
- A mesma ocorrência persistida não pode mudar silenciosamente de ID ou conteúdo durante retry.
- Campos de horário não devem ser preenchidos com valores atuais como substitutos de dados remotos ou locais desconhecidos.

## 8. Limites e pontos de integração

1. **Identidades já existentes:** reutilizar/migrar explicitamente os tipos do código real; não produzir tipos incompatíveis duplicados.
2. **Detector Local:** seu `LocalNotice` atual é um contrato anterior e precisará se adaptar a `File`; isso **não** foi alterado aqui.
3. **Detector Remoto:** deverá produzir `RemoteFile` e `File` sem importar detalhes Google para a fachada.
4. **Detector pai e Storage:** precisarão transportar/persistir `File` completo (por `FileBatch`) e reconhecer entrega segura; seus contratos anteriores ainda não foram revisados.
5. **Inventário:** o contrato anterior ainda utiliza `GoogleFile` e será revisto em etapa própria; **`SyncItem` permanece fora desta revisão**.
6. **Tipos especializados:** ProviderFileData não cria dependência de Models para Google API.
7. **Código:** nenhuma estrutura aqui é afirmada como implementada. Não alterar Rust nesta fase.
