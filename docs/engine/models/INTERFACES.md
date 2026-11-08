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

## 3. RemoteFile — dados de uma observação remota

```rust
pub struct RemoteFile {
    pub reference: RemoteFileRef,
    pub provider: ProviderId,
    pub name: String,
    pub kind: FileKind,
    pub mime_type: Option<String>,
    pub size_bytes: Option<u64>,
    pub modified_at: Option<Timestamp>,
    pub version: Option<String>,
    pub hash: Option<FileHash>,
    pub parent_ids: FieldValue<Vec<RemoteFileId>>,
    pub provider_data: Option<ProviderFileData>,
}
```

| Atributo | Tipo | Obrigatório? | Origem e semântica |
| --- | --- | --- | --- |
| `reference` | `RemoteFileRef` | Sim | ID do recurso qualificado pela conta, fornecido pelo especialista |
| `provider` | `ProviderId` | Sim | Serviço selecionado pela conta; precisa corresponder ao registro da conta |
| `name` | `String` | Sim | Nome retornado pelo serviço; manter Unicode sem normalização destrutiva |
| `kind` | `FileKind` | Sim | Classificação conhecida; `ProviderNative` para itens nativos do serviço; `Unknown` se indeterminado |
| `mime_type` | `Option<String>` | Campo sim; valor não | MIME quando retornado; ausência não implica `application/octet-stream` |
| `size_bytes` | `Option<u64>` | Campo sim; valor não | Tamanho disponível; documentos nativos podem não possuir tamanho em bytes |
| `modified_at` | `Option<Timestamp>` | Campo sim; valor não | Data fornecida pelo serviço, sem inventar precisão |
| `version` | `Option<String>` | Campo sim; valor não | Versão/ETag **opaca**; não ordenar nem comparar entre provedores |
| `hash` | `Option<FileHash>` | Campo sim; valor não | Digest retornado ou verificado; não inventar hash para documento nativo |
| `parent_ids` | `FieldValue<Vec<RemoteFileId>>` | Sim | `Unknown` = lista não observada; `Known([])` = ausência de pais realmente conhecida |
| `provider_data` | `Option<ProviderFileData>` | Campo sim; valor não | Metadados especializados opacos; `None` quando não recebidos/necessários |

`RemoteFile` nunca carrega conteúdo do arquivo, sessão, token, cliente HTTP, cursor de mudanças nem comando de exclusão. Um `RemoteFileId` bruto não é identidade única entre contas.

### 3.1 ProviderFileData — metadados especializados

```rust
pub struct ProviderFileData {
    pub provider: ProviderId,
    pub format: String,
    pub format_version: u32,
    pub payload: Vec<u8>,
}
```

| Atributo | Tipo | Obrigatório? | Regra |
| --- | --- | --- | --- |
| `provider` | `ProviderId` | Sim | Deve coincidir com `RemoteFile.provider` |
| `format` | `String` | Sim | Identificador não vazio do formato do payload |
| `format_version` | `u32` | Sim | Versão positiva da codificação específica |
| `payload` | `Vec<u8>` | Sim | **Somente metadados** especializados, jamais conteúdo do arquivo ou credenciais |

O envelope é opaco para Models/Orquestrador. Somente o especialista que conhece o provedor interpreta seu conteúdo e valida seu formato. Limite de tamanho e codificação concreta do payload devem ser fixados **antes da implementação**; não armazenar respostas HTTP brutas sem filtragem de segredos.

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

## 5. File — envelope unitário e autocontido de observação

```rust
pub enum FileEvent {
    ObservedChange,  // mudança ou metadados observados; categoria ainda genérica
    RecheckRequired,  // observação insuficiente: reavaliar o escopo
    ScopeUnavailable, // raiz ou subescopo inacessível
}

pub enum ObservationCoverage { Complete, Partial, Unavailable }

pub struct File {
    pub id: FileId,
    pub sync_id: SyncId,
    pub event: FileEvent,
    pub observed_at: Timestamp,
    pub coverage: ObservationCoverage,
    pub scope: FileScope,
    pub data: Option<FileData>,
}

pub enum FileScope {
    Local(LocalRelativePath),   // escopo local conhecido, [] = raiz
    Remote(RemoteFileRef),      // recurso remoto identificado
    WholeSync,                  // escopo completo ou localização inconclusiva
}
```

| Atributo | Tipo | Obrigatório? | Origem e semântica |
| --- | --- | --- | --- |
| `id` | `FileId` | Sim | Identidade estável **da ocorrência**, preservada em retry/reentrega |
| `sync_id` | `SyncId` | Sim | Sincronização à qual a observação pertence |
| `event` | `FileEvent` | Sim | Motivo que exige análise; não é uma ordem de operação |
| `observed_at` | `Timestamp` | Sim | Quando o Detector construiu a observação; não é mtime do arquivo |
| `coverage` | `ObservationCoverage` | Sim | Qualidade/completude da observação do escopo especificado |
| `scope` | `FileScope` | Sim | Localização/identidade a reavaliar, mesmo sem dados de arquivo |
| `data` | `Option<FileData>` | Campo sim; valor não | `Some` inclui dados realmente observados; `None` exige reavaliação, **não** significa exclusão |

**Regras de consistência:**

1. `data=Some(Local(x))` exige `scope=Local(x.relative_path)`; `data=Some(Remote(x))` exige `scope=Remote(x.reference)`. `WholeSync` só é permitido sem `data`.
2. `data=None` é permitido somente com `RecheckRequired` ou `ScopeUnavailable`. Não fabricar `LocalFile`/ `RemoteFile` quando não houve leitura confiável.
3. `event=ScopeUnavailable` exige `coverage=Unavailable` e `data=None`.
4. `event=ObservedChange` exige `data=Some(...)`, mas **não** implica confirmação da alteração frente ao baseline.
5. `coverage=Complete` descreve o escopo observado e nunca comprova, sozinho, ausência/deleção do arquivo.
6. `File` não contém `SyncItem`, decisão do Analisador, payload de transferência, lease, estado de fila ou confirmação.
7. `File` encapsula todas as informações **efetivamente observadas**, não garante que o filesystem/Drive permaneça idêntico na hora da análise.

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
