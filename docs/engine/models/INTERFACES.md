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

## 2. LocalFile — dados de uma observação local

```rust
pub struct LocalFile {
    pub local_id: Option<LocalFileId>,
    pub relative_path: LocalRelativePath,
    pub kind: FileKind,
    pub size_bytes: Option<u64>,
    pub modified_at: Option<Timestamp>,
    pub hash: Option<FileHash>,
}
```

| Atributo | Tipo | Obrigatório? | Origem e semântica |
| --- | --- | --- | --- |
| `local_id` | `Option<LocalFileId>` | Campo sim; valor não | Detector Local; `None` quando não existe identidade estável comprovada |
| `relative_path` | `LocalRelativePath` | Sim | Nome/caminho **relativo à raiz sincronizada**, em bytes Unix; a raiz é `[]` |
| `kind` | `FileKind` | Sim | Tipo verificado no filesystem; usar `Unknown` quando não determinado |
| `size_bytes` | `Option<u64>` | Campo sim; valor não | Tamanho de conteúdo, quando medido; `None` não significa tamanho zero |
| `modified_at` | `Option<Timestamp>` | Campo sim; valor não | mtime observado; `None` não significa momento atual |
| `hash` | `Option<FileHash>` | Campo sim; valor não | Só preencher quando realmente calculado; detecção não exige leitura integral |

**Invariantes:** não seguir symlink para fora da raiz; `size_bytes` e `hash` só descrevem arquivo regular quando mensuráveis; caminho não pode conter byte NUL, iniciar com `/` nem conter segmentos `.` ou `..`. O objeto é valor imutável do instante observado; ele próprio não lê disco. Não usar `local_id` como `SyncItemId`.

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
