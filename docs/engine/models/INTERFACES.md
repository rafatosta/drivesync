# Models — INTERFACES: contratos de dados para a engine

## 0. Escopo e interpretação

Esta revisão define a **seleção documental para o fluxo local**: `LocalFile`, seus auxiliares e as regras locais de `FileData`/`File`. A matriz da seção 2 registra as decisões por atributo. Os tipos locais e seus quatro métodos puros estão implementados; esta entrega não afirma aprovação dos campos remotos. `RemoteFile` e `ProviderFileData` continuam catálogo proposto, com revisão própria pendente. As assinaturas locais são implementadas por etapas; consultar o estado incremental abaixo. Os exemplos remotos continuam especificações não compiladas.

`models/` **não expõe uma interface operacional**. Não criar `ModelManager`, serviços de consulta, trait de persistência ou métodos de I/O. Validação pura de valores pode ser acrescentada **somente** para preservar os invariantes explicitamente definidos aqui.

**Proprietários:** `LocalFile`, `RemoteFile`, `FileData` e `File` pertencem a `models/`. `FileBatch`, IDs de lote, leases e confirmações pertencem a `detectors/`. `SyncItem` fica **reservado à revisão do Inventário**; não o introduzir nesta etapa.

## 1. Tipos auxiliares e identidades

```rust
pub struct ProviderId(pub String);
pub struct AccountId(pub String);
pub struct SyncId(pub String);
pub struct RemoteFileId(pub String);
pub struct LocalFileId(pub String);
pub struct FileId(pub String); // Observation identity, not physical file identity.

pub struct RemoteFileRef {
    pub account_id: AccountId,
    pub file_id: RemoteFileId,
}

pub struct Timestamp {
    pub unix_millis: i64, // UTC milliseconds since Unix epoch.
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

// Unix path bytes; construction validates a nonempty relative path.
pub struct LocalRelativePath(Vec<u8>);

pub enum FieldValue<T> {
    Unknown,   // Unobserved, partial or unsupported.
    Known(T),  // Observed; Known([]) is an observed empty collection.
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

Os tipos remotos/comuns do catálogo (`RemoteFileRef`, `RemoteFileId`, `FileHash`, `HashAlgorithm`, `FieldValue` e `FileKind`) não são selecionados para implementação por esta revisão local. `LocalEntryKind` e MetadataValue são os tipos efetivos do perfil local; não criar um segundo tipo de kind/estado para satisfazer o catálogo.

### Identidades no fluxo local

`AccountId`, `ProviderId` e `Timestamp` já existem em `engine/src/models.rs` e serão reutilizados sem alterar suas representações. `SyncId`, `FileId` e `LocalFileId` terão um único proprietário em Models, com strings opacas não vazias (strings só com whitespace também são inválidas); não normalizar, gerar ou converter identidades dentro do modelo. `File::validate` verifica `FileId` e `SyncId`; o escopo operacional do Detector verifica `AccountId`. `LocalFile::validate` verifica `LocalFileId` quando presente.

O coletor local desta primeira versão produz `local_id=None`: não há mecanismo aprovado de identidade lógica persistente. Device/inode são evidências amostradas, não IDs da engine. Gerar `FileId` único por nova ocorrência é obrigação do produtor; a mesma ocorrência repetida conserva ID, horário e payload. Models não consulta banco nem gera aleatoriedade para provar unicidade.

## 2. LocalFile — seleção para observação local

### 2.1. Critério e destino dos dados

A seleção cobre as necessidades documentadas de observação, diagnóstico de acesso, revalidação e comparação posterior de estados. Consumidores futuros citados abaixo são responsabilidades previstas, não integração já implementada. **Manter** significa incluir no contrato local e transportar o valor ou o estado de coleta; **sob demanda** significa consulta específica futura, fora do `LocalFile` desta versão; **descartar** significa excluir do contrato local por não haver finalidade demonstrada.

Todos os campos mantidos seguem integralmente em `FileData::Local`. Sua persistência eventual continua sendo decisão dos proprietários Detector Storage/Inventário; a presença no modelo não autoriza logs, envio ao Google, restauração de permissões ou retenção indefinida. Dados descartados e sob demanda não ganham placeholders, blobs ou structs vazias no envelope.

### 2.2. Matriz de revisão do catálogo local

| Atributo do catálogo | Decisão e destino | Finalidade/consumidor | Fonte, custo e exposição |
| --- | --- | --- | --- |
| `local_id` | Manter opcional; None no coletor inicial | Preservar possibilidade de associação comprovada sem inventá-la | Identidade de domínio, não fornecida por stat; nenhuma geração nesta etapa |
| `relative_path` | Manter em LocalFile | Localizar entrada no escopo; consumidor do evento | Enumeração/solicitação; bytes do nome, potencialmente pessoais; sem caminho absoluto |
| `kind` | Manter | Distinguir arquivo, diretório, link e especiais antes de futuras operações | stat/statx sem seguir link; consulta básica |
| `mode.raw_mode` | Manter | Evidência de tipo e permissões para diagnóstico/revalidação | stat/statx; numérico, não comprova acesso efetivo |
| `mode.permission_bits` | Manter, consistente com raw_mode | Leitura explícita de rwx e bits especiais por consumidores | Mesma consulta; nenhuma syscall extra |
| `owner.uid`, `owner.gid` | Manter | Diagnóstico de propriedade e mudança de metadados | stat/statx; IDs numéricos locais, sem resolução de nomes ou upload |
| `times.accessed_at` | Descartar | Nenhuma decisão de sync depende de acesso; leitura pode gerar ruído | atime não participa de comparação nem de precedência |
| `times.modified_at` | Manter | Pista de alteração; comparação posterior, nunca prova de igualdade | stat/statx; segundos e nanossegundos |
| `times.changed_at` | Manter | Mudanças de status e revalidação de evidência | stat/statx; ctime não é criação |
| `times.created_at` | Manter opcional à cobertura | Preservar btime quando reportado, sem substituir por ctime | statx quando disponível; não fazer consulta cara alternativa só para preencher |
| `storage.size_bytes` | Manter | Tamanho observado e triagem posterior de alteração | stat/statx; significado depende do tipo, não tratar tamanho de diretório como conteúdo |
| `storage.allocated_blocks_512b` | Sob demanda, fora do envelope | Diagnóstico futuro de alocação/arquivos esparsos | stat/statx, baixo custo; sem consumidor de detecção que justifique retenção por evento |
| `storage.preferred_block_size` | Sob demanda, fora do envelope | Eventual ajuste de I/O no operador | Dica do filesystem, não é mudança do item a sincronizar |
| `storage.hard_link_count` | Manter | Evidência de aliases; não autorizar associação ou upload automático | stat/statx; consulta básica |
| `storage.inode` | Manter | Revalidação amostrada junto ao device | stat/statx; pode ser reutilizado, não é identidade persistente |
| `storage.filesystem_device` | Manter | Distinguir evidências de inodes em filesystems distintos | stat/statx; major/minor locais, sem significado entre máquinas |
| `storage.represented_device` | Descartar | Não se pretende materializar nem sincronizar devices | rdev não é necessário para observar a existência/tipo da entrada |
| `storage.mount_id` | Sob demanda, fora do envelope | Verificação física de montagem pelo adaptador, se necessária | Dado operacional do ambiente; a segurança do percurso não depende de o incluir em File |
| `storage.subvolume_id` | Sob demanda, fora do envelope | Diagnóstico específico de filesystem | Depende de suporte; sem regra atual de sync que o consuma |
| `flags.statx_attributes`, `flags.statx_attributes_mask` | Manter opcionais à cobertura, sempre em par | Preservar flags e evidência de suporte que a consulta básica retornar | statx; sem consulta adicional obrigatória, sem inferir acesso a partir de bits |
| `flags.filesystem_flags` | Sob demanda, fora do envelope | Diagnóstico pontual de restrição de escrita pelo operador | Consulta específica; não antecipar política de operações no detector |
| `direct_io.memory_alignment`, `offset_alignment`, `read_offset_alignment` | Sob demanda, fora do envelope | Capacidade de I/O de futura implementação do operador | Dados operacionais, sem consumidor do evento; não habilitar Direct I/O por esta revisão |
| `atomic_write.unit_min`, `unit_max`, `segments_max`, `unit_max_opt` | Sob demanda, fora do envelope | Capacidade de escrita do operador | Não demonstra que uma edição observada foi atômica |
| `xattrs.entries`, `LocalXattr.name`, `LocalXattr.value` | Sob demanda, fora do envelope | Diagnóstico específico futuro, com política própria | Enumeração/leitura adicional; namespaces podem conter dados sensíveis; sem coleta automática |
| `acl.access`, `acl.default` e suas tags/permissões | Sob demanda, fora do envelope | Diagnóstico de autorização quando uma operação falhar | Consulta adicional e dados de terceiros; o kernel já aplica o acesso, sem precisar serializar ACL |
| `symlink.target` | Manter para symlink; NotApplicable nos demais | Observar link literal sem acessar destino | readlink no adaptador seguro; bytes podem revelar caminhos externos, sem resolução ou log automático |
| `extra_metadata` e `FilesystemMetadata.{namespace,name,value}` | Descartar | Blob aberto não tem consumidor ou limite definido | Evitar transporte arbitrário de dados privados/específicos do filesystem |
| Hash, MIME inferido, previews, tags de mídia, conteúdo | Descartar deste modelo | Dados derivados/transferência pertencem a outras responsabilidades | Leitura de conteúdo não é requisito da observação de metadados |

**Sob demanda não é uma API implementada ou autorizada de coleta.** Uma necessidade concreta deve definir consulta, limites, sanitização e testes no componente proprietário antes de adicionar campos ou ler esses dados. Não criar `LocalXattr`, ACLs, Direct I/O, AtomicWrite ou `extra_metadata` em Rust apenas por constarem no catálogo anterior.

### 2.3. Contrato local selecionado

```rust
pub enum MetadataValue<T> {
    Known(T),
    NotCollected,
    NotApplicable,
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
    pub raw_mode: u32,
    pub permission_bits: u16, // rwx, setuid, setgid and sticky bits.
}
pub struct LocalOwner {
    pub uid: MetadataValue<u32>,
    pub gid: MetadataValue<u32>,
}
pub struct LocalTimes {
    pub modified_at: MetadataValue<LocalTimestamp>,
    pub changed_at: MetadataValue<LocalTimestamp>,
    pub created_at: MetadataValue<LocalTimestamp>,
}
pub struct LocalStorage {
    pub size_bytes: MetadataValue<u64>,
    pub hard_link_count: MetadataValue<u64>,
    pub inode: MetadataValue<u64>,
    pub filesystem_device: MetadataValue<DeviceNumber>,
}
pub struct LocalFileFlags {
    pub statx_attributes: MetadataValue<u64>,
    pub statx_attributes_mask: MetadataValue<u64>,
}
pub struct LocalSymlink {
    pub target: MetadataValue<Vec<u8>>,
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
    pub symlink: LocalSymlink,
}
```

`MetadataValue` é dado comum. A adição de `NotApplicable` fecha a semântica local; não aprova automaticamente seu uso em campos remotos, que ainda dependem de revisão.

### 2.4. Ausência, aplicabilidade e cobertura da coleta

- `Known(0)`, `Known(false)` e `Known([])` são valores observados. `NotCollected` significa não consultado; `Unsupported`, não oferecido pelo backend/FS; `PermissionDenied`, consulta efetivamente negada; `Unavailable`, tentativa inconclusiva/falha transitória; `NotApplicable`, conceito que não se aplica à entrada. Nenhum desses estados autoriza inferir exclusão.
- Neste contrato local, `NotApplicable` é admitido apenas em `symlink.target` quando `kind` é conhecido e não é Symlink. Para Symlink, target é `Known(bytes)` ou estado de coleta/falha, nunca NotApplicable. Com kind desconhecido, não presumir que o alvo não se aplica.
- O perfil básico obrigatório compreende tipo concreto, mode, UID/GID, mtime/ctime, size, nlink, inode e device; em symlink inclui leitura literal do alvo. `Known(Unknown)` não é tipo concreto. Se qualquer dado obrigatório não for Known, a coleta tem cobertura Partial (ou Unavailable se não houver observação confiável). O objeto parcial pode ser transportado sem preencher os demais por suposição.
- btime e o par de flags são opcionais à cobertura: NotCollected/Unsupported/PermissionDenied/Unavailable nesses campos não impedem Complete do perfil básico. Complete não significa todos os metadados possíveis do Linux, integridade do conteúdo nem uma fotografia atômica. O adaptador decide a cobertura considerando também corridas; Models valida consistência estrutural, não prova ausência de corrida.
- Sem acesso aos grupos básicos, o coletor preserva estados reais e devolve cobertura apropriada. Um `LocalFile` com algum dado comprovado pode acompanhar RecheckRequired/Partial; se nenhum metadado confiável foi obtido, devolve `file=None`. Não produzir entrada conhecida apenas para satisfazer um evento específico.
- A ausência de um bit de preenchimento de statx não é valor zero nem prova automática de Unsupported: o adaptador precisa distinguir falta de coleta, suporte e falha pelo resultado efetivo. Para flags, interpretar somente bits cobertos por `statx_attributes_mask`; a máscara é preservada junto do valor. Na falta de par confiável, ambos recebem o mesmo estado não Known.
- O coletor pode usar stat sem seguir links quando statx não estiver disponível; preenche os dados efetivamente fornecidos e marca os opcionais indisponíveis, sem alterar o detector. Não executar I/O em Models.
- Timestamps locais preservam segundos assinados e nanossegundos. Não converter datas desconhecidas em relógio atual; não impor mtime <= ctime ou criação <= modificação. `Timestamp` do evento continua UTC em milissegundos e não substitui a precisão local.
- `mode.permission_bits` corresponde exatamente a `(raw_mode & 0o7777) as u16` (rwx/setuid/setgid/sticky). Tipo concreto em kind deve ser consistente com tipo reconhecido em raw_mode; números de proprietário/device e modos não são decisões de autorização. Não rejeitar nlink/inode/size zero apenas por convenção de filesystem.
- A lista selecionada não inclui conteúdo, hash, atime, xattrs ou ACL. Isso evita que o custo/privacidade dessas consultas se torne dependência de cada evento. A ausência desses campos não é coleta Partial do perfil selecionado.

### 2.5. Caminho, raiz e symlink

`LocalRelativePath` contém bytes Unix não vazios, com separador `/`, preservados sem normalização Unicode, percent encoding, conversão com perda ou resolução física. Rejeitar NUL, barra inicial/final, separadores repetidos e componentes exatamente `.`/`..`; aceitar nomes ocultos, espaços, quebras de linha, bytes não UTF-8 e nomes como `a..b` ou `..backup`. Barras invertidas são bytes de nome, não separadores Unix.

Raiz não é `LocalRelativePath([])`, `.` ou `/`: representa-se por `FileScope::WholeSync` com `data=None` e localizações ausentes. Para pedir varredura da raiz, o Detector usa Full. `.drivesync` é um caminho lexicalmente válido em Models; ignorá-lo é política do observador, não do validador puro.

O alvo de symlink é um valor literal distinto de `LocalRelativePath`: pode ser absoluto ou conter `..`, pois não é resolvido. Validar um caminho lexicalmente não prova confinamento contra symlinks, troca de inode, montagem ou corridas; essas provas pertencem ao adaptador físico.

### 2.6. Evolução e fronteira com o detector

O Detector solicita coleta por caminho e recebe `LocalFile` pronto. Não faz struct literal, enumera grupos, calcula completude por campo nem chama statx/ACL/xattr. Adicionar/remover campo muda coletor, testes e consumidores interessados; watcher, varredura e emissão transportam o objeto integral sem edição. As assinaturas da [porta local](../detectors/local/INTERFACES.md) permanecem as mesmas.

Models pode validar invariantes puras do objeto; isso não faz do detector um consumidor do esquema. A classificação de evento e a prova física de identidade continuam fora de Models. Não introduzir SyncItem, fila, baseline ou serviço de consulta.

### 2.7. Estado e referências técnicas

**Seleção local implementada em `engine/src/models/`, incluindo LocalFile/validate; envelope ainda documental.** Campos remotos e payload de provedor não foram selecionados. Antes de implementar o envelope compartilhado completo, resolver a dependência de `FileData::Remote(RemoteFile)` sem criar um RemoteFile fictício, remover variantes ou implementar automaticamente o catálogo remoto. A implementação isolada dos tipos locais não depende de I/O remoto; o envelope completo ainda depende da revisão remota.

As fontes técnicas sustentam a semântica das consultas, não as decisões de seleção do produto: [statx(2)](https://man7.org/linux/man-pages/man2/statx.2.html) documenta timestamps, máscaras e campos; [path_resolution(7)](https://man7.org/linux/man-pages/man7/path_resolution.7.html) documenta resolução física de caminhos; [xattr(7)](https://man7.org/linux/man-pages/man7/xattr.7.html) documenta atributos e namespaces. As decisões manter/sob demanda/descartar são deste contrato.

### 2.8. Validação pura e assinaturas locais

```rust
pub enum ModelError {
    EmptyIdentity,
    InvalidRelativePath,
    InvalidTimestamp,
    InconsistentMetadata,
    InconsistentEvent,
}
pub type ModelResult<T> = Result<T, ModelError>;

impl LocalRelativePath {
    pub fn new(bytes: Vec<u8>) -> ModelResult<Self>;
    pub fn as_bytes(&self) -> &[u8];
}
impl LocalTimestamp {
    pub fn new(unix_seconds: i64, nanoseconds: u32) -> ModelResult<Self>;
}
impl LocalFile {
    pub fn validate(&self) -> ModelResult<()>;
}
```

`ModelError` é tipado e não carrega nomes/caminhos, payloads ou mensagens do SO. Não criar repositório ou serviço de validação. A futura desserialização deve usar a mesma validação e não contornar o campo privado de LocalRelativePath; esta revisão não define formato de persistência.

#### `LocalRelativePath::new`

**Assinatura:** `fn new(bytes: Vec<u8>) -> ModelResult<Self>`.

**Finalidade:** construir caminho de uma entrada abaixo da raiz.

**Parâmetros:** bytes Unix integrais, segundo a seção 2.5, sem decodificar como UTF-8.

**Retorno:** caminho validado que preserva exatamente os bytes de entrada.

**Limites e erros:** InvalidRelativePath para vazio/NUL/absoluto/componentes proibidos. Nenhum acesso a disco, normalização ou teste de existência. Não rejeita nomes reservados por política do detector.

#### `LocalRelativePath::as_bytes`

**Assinatura:** `fn as_bytes(&self) -> &[u8]`.

**Finalidade:** consultar a representação sem perda.

**Parâmetros:** nenhum.

**Retorno:** empréstimo imutável dos bytes validados, sem cópia ou conversão.

**Limites e erros:** sem erro, mutação, resolução física ou efeitos externos.

#### `LocalTimestamp::new`

**Assinatura:** `fn new(unix_seconds: i64, nanoseconds: u32) -> ModelResult<Self>`.

**Finalidade:** representar timestamp local com precisão de nanossegundos.

**Parâmetros:** segundos assinados desde Unix epoch e fração de 0 a 999_999_999.

**Retorno:** valores preservados, inclusive segundos anteriores à epoch.

**Limites e erros:** InvalidTimestamp para nanoseconds >= 1_000_000_000. Não consultar relógio, ajustar fuso ou arredondar silenciosamente. Ausência de timestamp é representada por MetadataValue, não por timestamp zero.

#### `LocalFile::validate`

**Assinatura:** `fn validate(&self) -> ModelResult<()>`.

**Finalidade:** verificar consistência estrutural da observação local.

**Parâmetros:** o próprio objeto, inclusive os valores Known dos grupos selecionados.

**Retorno:** sucesso para objeto completo ou parcial coerente. Não determina cobertura física nem valida existência.

**Limites e erros:** EmptyIdentity para local_id presente e vazio; InvalidTimestamp para fração inválida em timestamp Known; InconsistentMetadata para mode/permission_bits/tipo incompatíveis, par de flags com apenas um Known ou estados não Known diferentes, uso local proibido de NotApplicable e alvo de symlink incompatível com kind. Não comparar datas para estabelecer ordem de versões; não rejeitar IDs desconhecidos, zeros reais ou estados parciais. Não resolver usuários, links ou paths no filesystem.

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
        reference: RemoteFileRef,      // Resource identity, not a complete location.
        parent_id: Option<RemoteFileId>,
        name: Option<String>,
    },
}

pub struct FileEvent {
    pub kind: FileEventKind,
    pub uncertainty: Option<ObservationUncertainty>,
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

**Estado:** regras locais selecionadas nesta revisão documental, sem implementação Rust. Prova física e classificação continuam nos detectores; dados remotos permanecem pendentes de revisão própria.

### 5.5. Combinações locais e raiz

As regras abaixo fecham o fluxo local; não selecionam campos nem política de dados remotos.

| Evento local | Dados/localização exigidos | Cobertura e escopo |
| --- | --- | --- |
| Created, Modified, MetadataChanged | Some(Local), current=Local do mesmo relative_path | Local do mesmo path; Complete ou Partial, nunca Unavailable |
| Renamed, Moved | Some(Local), previous=Local confiável e distinto de current; current=path do objeto | Local do destino; Complete ou Partial. Sem prova física da correlação, produtor usa RecheckRequired |
| Deleted | data=None, previous=Local do path conhecido, current=None | Local do mesmo path; Complete ou Partial; indício de remoção, sem confirmação para apagar |
| RecheckRequired com dados | Some(Local), current=Local do mesmo path; previous opcional local | Local do mesmo path; Complete ou Partial |
| RecheckRequired sem dados | current/previous opcionais locais e coerentes com o escopo atual | Local(path) ou WholeSync; WholeSync exige ambos ausentes; Complete/Partial/Unavailable não comprovam exclusão |
| ScopeUnavailable | data=None, current=None; previous local opcional em escopo Local | coverage=Unavailable; Local(path) ou WholeSync; WholeSync exige previous=None |

Se scope é Local e current está presente, current deve coincidir com scope. Sem current, previous presente deve coincidir com scope. Nenhuma localização remota é aceita em um File de escopo Local. WholeSync só admite RecheckRequired ou ScopeUnavailable sem dados/localizações. Eventos locais específicos não são usados para a própria raiz.

`coverage=Complete` com dados locais exige que os campos obrigatórios do perfil básico sejam Known e kind concreto; em não-symlink, o alvo é NotApplicable. Ainda assim, o validador não prova que a enumeração ocorreu ou que o filesystem permaneceu estável. Eventos parciais são válidos e não são completados com valores fabricados.

### 5.6. `File::validate`

**Assinatura:** `fn validate(&self) -> ModelResult<()>`.

**Finalidade:** preservar os sete campos e as relações entre evento, escopo, localizações e dados.

**Parâmetros:** o próprio File, sem dependências operacionais. A assinatura futura fica em `impl File`.

**Retorno:** sucesso quando identidades, dados locais e combinações das seções 5.4/5.5 são coerentes. Propaga erros puros de LocalFile quando presente.

**Limites e erros:** EmptyIdentity para FileId/SyncId inválidos; InconsistentEvent para combinação inválida, origem misturada, escopo incompatível, WholeSync específico ou Complete com perfil local obrigatório incompleto. Não gera ID, consulta relógio/banco, verifica ordem causal ou comprova correlação de rename. Compara valores, não executa classificação de watcher. Não deduplica nem transforma eventos. A validação remota só poderá ser implementada integralmente após sua revisão; não aceitar objetos remotos sem revisão por conveniência.

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
2. **Detector Local:** usa a porta de coleta estável e transporta File integral, conforme [seu contrato interno](../detectors/local/INTERFACES.md). Os modelos permanecem puros; a coleta física está implementada separadamente no módulo Local, sem eventos ou watcher.
3. **Detector Remoto:** deverá produzir `RemoteFile` e `File` sem importar detalhes Google para a fachada.
4. **Detector pai e Storage:** seus contratos documentais já preveem File completo e aceite durável; implementação e integração continuam separadas desta revisão.
5. **Inventário:** o contrato anterior ainda utiliza `GoogleFile` e será revisto em etapa própria; **`SyncItem` permanece fora desta revisão**.
6. **Tipos especializados:** ProviderFileData não cria dependência de Models para Google API.
7. **Código:** somente os tipos locais e seus validadores estão implementados; consultar a seção 9. Exemplos remotos e envelope não são código compilado.


## 9. Estado incremental do código

Implementados em `engine/src/models/`: LocalRelativePath (new/as_bytes), LocalTimestamp (new), LocalFile (validate), seus grupos selecionados, MetadataValue, ModelError/ModelResult e os wrappers SyncId/FileId/LocalFileId. AccountId/ProviderId/Timestamp existentes foram preservados com a serialização anterior.

LocalFile::validate verifica atributos conhecidos, coerência de modo/tipo, par de flags, aplicabilidade e identidade local presente sem consultar filesystem ou completar campos. Libc é usado somente para constantes de tipo de arquivo Linux; não há syscall em Models. Tipos locais novos não possuem serialização/desserialização enquanto o formato de transporte/persistência não for definido.

File, FileData, File::validate, modelos remotos, gerador de IDs, watcher e persistência permanecem não implementados. O coletor físico agora existe separadamente em `engine/src/detectors/local/`, preservando a pureza de Models. As regras de envelope das seções 5.4–5.6 continuam especificação futura; os 18 testes atuais não validam essas regras nem a cobertura física. SyncId e FileId são valores opacos, sem geração ou validação de ocorrência nesta etapa.


## 10. Envelope local compilado — parametrização do payload remoto

A implementação de eventos locais usa `File<R = std::convert::Infallible>` e `FileData<R = std::convert::Infallible>`. Os sete campos do envelope e as variantes Local/Remote permanecem os mesmos; `Remote(R)` recebe o tipo do payload somente quando sua revisão for concluída. Infallible não tem valores, portanto esta fase não constrói um RemoteFile fictício, não remove a variante remota e não cria uma representação paralela de evento local.

`File::validate` nesta fase valida File<Infallible> segundo as regras locais. FileEventKind, FileLocation, FileScope, ObservationCoverage e os identificadores RemoteFileId/RemoteFileRef são dados puros de contexto; não implementam atributos ou coleta remota. O validador remoto e eventual conversão tipada para o envelope com payload remoto ficam para sua revisão específica. Não interpretar o parâmetro genérico como aprovação do catálogo RemoteFile.

O perfil básico local tem um helper puro interno compartilhado entre validação do envelope e adaptador de metadados; isso não transfere a coleta nem a decisão de cobertura física para Models. Nenhum formato de serialização foi introduzido.


## Motivo explícito de reavaliação no envelope compilado

A normalização local acrescenta `FileEvent.uncertainty: Option<ObservationUncertainty>` ao contexto do evento, preservando os sete campos de File e o payload LocalFile integral. RecheckRequired exige Some; todos os demais tipos exigem None. O enum puro contém CaptureStarted, CaptureRebuilt, CoverageIncomplete, NativeOverflow, ResourceLimit, UnpairedMove, UnclassifiedSignal, ObservationUnstable, WriteInProgress e ConflictingSequence. A validação só verifica coerência estrutural, sem I/O, relógio ou inferência. Não há alteração do catálogo remoto.
