# Models — INTERFACES (contratos de dados)

## 1. Existe interface operacional?

**Não.** O módulo `models/` é uma biblioteca de **tipos de domínio e invariantes**, não uma dependência que precisa de serviço/trait para consultas, persistência ou transportes. `INTERFACES.md` documenta os **dados públicos**, sua semântica e as operações **puras de validação/conversão de valores** que venham a ser necessárias.

Não criar `ModelManager`, `RemoteFileRepository`, `RemoteProvider` ou `get_file()` neste módulo. Interfaces operacionais pertencem a [Providers](../providers/README.md), [Detector](../detectors/INTERFACES.md), [Inventário](../inventory/INTERFACES.md) e [Operações](../operations/README.md).

As definições abaixo são **contratos conceituais em sintaxe Rust**, não código compilado nem garantia de que todas as estruturas já existem. Uso de referências, tipos de data, derives, visibilidade e serialização será resolvido ao implementar, sem comprometer os invariantes.

## 2. Identidade e escopo

```rust
pub struct ProviderId(pub String);       // "google", "onedrive" etc.
pub struct AccountId(pub String);        // identidade da conta configurada
pub struct SyncId(pub String);           // identidade da sincronização
pub struct RemoteFileId(pub String);     // identidade do arquivo DENTRO da conta
pub struct LocalFileId(pub String);      // identidade local verificada, se disponível

pub struct RemoteFileRef {
    pub account_id: AccountId,
    pub file_id: RemoteFileId,
}
```

**Regra:** `RemoteFileId` isolado não é identidade global. `RemoteFileRef` é única somente quando `AccountId` é globalmente inequívoco e preserva a associação ao provedor. O identificador do provedor pertence ao registro de conta e pode aparecer como etiqueta auxiliar em dados serializados, mas **não substitui a conta** para resolver credenciais.

`AccountId` já possui definição em `engine/src/models.rs`: a implementação deve **reutilizá-la ou migrar seu proprietário explicitamente**, nunca criar dois tipos incompatíveis com o mesmo nome. O mesmo vale para identificadores existentes do Inventário e do estado local.

Validação de identidade é local/pura: valores vazios e referências inconsistentes são inválidos; a disponibilidade de uma conta autenticada **não** é uma propriedade do modelo. Identidades não incluem tokens ou segredos.

## 3. RemoteFile — representação compartilhada

```rust
pub enum RemoteFileKind {
    File,
    Directory,
    ProviderNative,
    Other,
    Unknown,
}

pub struct RemoteFile {
    pub reference: RemoteFileRef,
    pub name: String,
    pub kind: RemoteFileKind,

    // Propriedades disponíveis quando observadas com confiabilidade.
    pub size: Option<u64>,
    pub mime_type: Option<String>,
    pub modified_at: Option<Timestamp>,
    pub version: Option<RemoteVersion>,
    pub checksum: Option<Checksum>,
    pub parent_ids: Vec<RemoteFileId>,

    // Dados específicos permanecem junto ao pacote, mas opacos
    // para consumidores comuns. Veja a seção 5.
    pub provider_data: Option<ProviderFileData>,
}
```

**Obrigatórios:** referência de arquivo, nome (inclusive raiz somente quando a representação admitir nome vazio com semântica explícita) e tipo classificável, que pode ser `Unknown`. Todos os outros campos são **observações opcionais**; `None` **não** indica ausência física, remoção de campo nem exclusão.

`RemoteVersion` não pressupõe que versões de provedores diferentes sejam numericamente comparáveis. `Checksum` deve incluir algoritmo, não apenas digest. Pais podem representar referências ainda não resolvidas; `parent_ids=[]` **não prova que a API confirmou ausência de pais** quando a observação foi parcial.

Uma lista de campos em uma resposta do provedor que não inclui `size` ou `parents` não autoriza apagar valores antes conhecidos no Inventário. A aplicação pode utilizar **uma camada de observação/atualização com máscara de campos conhecidos**, pertencente ao produtor ou ao contrato de atualização do Inventário; não converter metadados parciais diretamente em substituição integral de `RemoteFile`.

## 4. LocalFile — valor observado, não acesso ao disco

```rust
pub enum LocalFileKind {
    File,
    Directory,
    Symlink,
    Other,
    Unknown,
}

pub struct LocalFile {
    pub id: Option<LocalFileId>,
    pub relative_path: LocalRelativePath,
    pub kind: LocalFileKind,
    pub size: Option<u64>,
    pub modified_at: Option<Timestamp>,
    pub checksum: Option<Checksum>,
}
```

`LocalRelativePath` deve preservar fielmente componentes aceitos na plataforma de destino; em sistemas Unix, nomes podem não ser UTF-8. Não escolher `String` como representação universal sem documentar a política de perda. `id=None` indica identidade local ainda não comprovada; inode, caminho e identidade da engine não são automaticamente equivalentes.

`LocalFile` não faz `stat`, não abre arquivos, não examina conteúdo e não atualiza o Inventário. O Detector Local produz dados através de sua observação.

## 5. ProviderFileData — conteúdo opaco dentro de RemoteFile

A ideia de `GoogleFile` ser **um atributo transportado por `RemoteFile`** é atendida por um **envelope genérico**, sem obrigar a crate `models` a importar o tipo Google:

```rust
pub struct ProviderFileData {
    pub provider_id: ProviderId,
    pub format_id: String,
    pub format_version: u32,
    pub payload: Vec<u8>,
}
```

Na implementação Google, o especialista conhece `GoogleFile` e faz a adaptação:

```rust
// Dentro do especialista Google, NÃO dentro de models:
fn encode_google_file(file: &GoogleFile) -> Result<ProviderFileData, AdaptError>;
fn decode_google_file(data: &ProviderFileData) -> Result<GoogleFile, AdaptError>;
```

**Essas duas funções são exemplos de responsabilidades, não novas APIs do módulo models.** `ProviderFileData` é um envelope de dados opacos, tipados por provedor/formato/versão. Não contém token ou cliente. Os consumidores comuns o preservam sem desserializar. Um especialista pode validar identidade do provedor antes de interpretar. Isso permite acrescentar provedores **sem mudar o enum central**.

O armazenamento de payload exige **limite de tamanho, formato versionado, validação e política de retenção** antes da implementação. Pode-se preservar apenas os metadados específicos necessários, em vez de toda a resposta JSON; `RemoteFile` não guarda conteúdo binário do arquivo. Se o payload não estiver disponível, `provider_data=None` é válido; o consumidor não deve inventá-lo.

## 6. Tipos auxiliares e semântica

```rust
pub struct RemoteVersion(pub String); // versão opaca, sem ordenação global
pub enum ChecksumAlgorithm { Md5, Sha256, Other(String) }
pub struct Checksum {
    pub algorithm: ChecksumAlgorithm,
    pub digest: Vec<u8>,
}
pub struct Timestamp { /* instante com precisão e fuso definidos */ }
pub struct LocalRelativePath { /* componentes preservados sem perda */ }
```

O contrato de `Timestamp` e a codificação dos caminhos serão definidos de forma compatível com os tipos existentes, sem criar versões paralelas. Comparação de hashes só é válida quando algoritmo, tamanho de digest e escopo de conteúdo são compatíveis.

### Opcionalidade vs ausência comprovada

- `Option<T>::None`: atributo não disponibilizado ou não observado, conforme o contrato de origem.
- Ausência física de arquivo: **não** representada por `RemoteFile`; exige evidência de observação/estado pelo Detector e Inventário.
- Limpeza de metadados antes existentes: operação explícita via atualização `FieldUpdate` no módulo proprietário, não inferência de `None`.
- `Unknown` em tipo de arquivo: informação insuficiente; não tentar adivinhar pelo nome ou extensão.

## 7. Fronteira do módulo

Não pertencem a `models/`:

- `GoogleFile`, `OneDriveFile` e modelos HTTP concretos;
- `SyncItem`, baseline, presença, conflitos e fila de operações, definidos no Inventário ou no Analisador;
- credenciais e sessões;
- eventos, leases e lote de detecção;
- `RemoteProvider`, `RemoteDetector`, `Uploader` e `Downloader` operacionais;
- consultas à API, disco ou banco.

## 8. Pendências antes de código

Consolidar o proprietário único de `AccountId`, `SyncId`, `LocalFileId` e `Timestamp`; representação exata de `LocalRelativePath`; sinalização de metadados parciais; política de formato/limite do payload opaco; eventual integração de `RemoteFile` com o contrato legado de `InventoryManager`.

**Não há API operacional a implementar nem métodos de serviço para enumerar neste documento.**
