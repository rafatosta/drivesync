# Operador — INTERFACES

## 1. Interface pública

`Operator` disponibiliza operações explícitas sobre arquivos e diretórios locais e remotos. Cada método recebe uma requisição completa e devolve um resultado tipado. As assinaturas abaixo constituem **contrato conceitual proposto**, sujeito à definição concreta de ownership e assincronicidade na implementação.

```rust
pub type OperatorResult<T> = Result<T, OperatorError>;

impl Operator {
    pub fn new(config: OperatorConfig) -> OperatorResult<Self>;

    pub async fn create_file(&self, request: CreateFileRequest)
        -> OperatorResult<ItemResult>;
    pub async fn create_directory(&self, request: CreateDirectoryRequest)
        -> OperatorResult<ItemResult>;
    pub async fn upload(&self, request: UploadRequest)
        -> OperatorResult<TransferResult>;
    pub async fn download(&self, request: DownloadRequest)
        -> OperatorResult<TransferResult>;
    pub async fn update_content(&self, request: UpdateContentRequest)
        -> OperatorResult<TransferResult>;
    pub async fn rename(&self, request: RenameRequest)
        -> OperatorResult<ItemResult>;
    pub async fn move_item(&self, request: MoveRequest)
        -> OperatorResult<ItemResult>;
    pub async fn delete(&self, request: DeleteRequest)
        -> OperatorResult<DeleteResult>;
    pub async fn trash(&self, request: TrashRequest)
        -> OperatorResult<ItemResult>;
    pub async fn restore(&self, request: RestoreRequest)
        -> OperatorResult<ItemResult>;
}
```

## 2. Identificação de recursos e destino

```rust
pub enum ResourceRef {
    Local { path: PathBuf },
    Remote {
        account_id: AccountId,
        provider_id: ProviderId,
        file_id: RemoteFileId,
    },
}

pub enum DirectoryRef {
    Local { path: PathBuf },
    Remote {
        account_id: AccountId,
        provider_id: ProviderId,
        directory_id: RemoteFileId,
    },
}

pub enum ContentSource {
    LocalFile { path: PathBuf },
    RemoteFile { resource: ResourceRef },
    Bytes { data: Vec<u8> },
}

pub struct Preconditions {
    pub expected_version: Option<String>,
    pub expected_revision: Option<String>,
}

pub struct ExecutionOptions {
    pub cancellation: Option<CancellationToken>,
    pub progress: Option<ProgressReporter>,
}
```

`ResourceRef` identifica um recurso existente; `DirectoryRef` identifica o contêiner de criação ou movimentação. O contexto remoto deve identificar uma sessão válida e o provedor específico. O método de obtenção dessa sessão fica encapsulado na composição do Operador.

`ContentSource` representa a origem do conteúdo; a implementação pode usar leitura em fluxo para evitar carregar arquivos grandes na memória. `Preconditions` expressa versões/revisões esperadas. Ausência de precondição não equivale a permissão implícita de sobrescrita: o comando deve definir explicitamente o alvo e a intenção de substituição.

`ExecutionOptions` é contexto opcional compartilhado de cancelamento e progresso, fornecido na requisição ou em mecanismo equivalente definido na implementação. Cancelamento é uma solicitação, não prova de reversão do efeito.

## 3. Requisições

```rust
pub struct CreateFileRequest {
    pub parent: DirectoryRef,
    pub name: String,
    pub content: Option<ContentSource>,
    pub options: ExecutionOptions,
}
pub struct CreateDirectoryRequest {
    pub parent: DirectoryRef,
    pub name: String,
    pub options: ExecutionOptions,
}
pub struct UploadRequest {
    pub source: ResourceRef, // local
    pub destination: DirectoryRef, // remote
    pub name: String,
    pub options: ExecutionOptions,
}
pub struct DownloadRequest {
    pub source: ResourceRef, // remote
    pub destination: DirectoryRef, // local
    pub name: String,
    pub options: ExecutionOptions,
}
pub struct UpdateContentRequest {
    pub target: ResourceRef,
    pub source: ContentSource,
    pub preconditions: Preconditions,
    pub options: ExecutionOptions,
}
pub struct RenameRequest {
    pub target: ResourceRef,
    pub new_name: String,
    pub preconditions: Preconditions,
}
pub struct MoveRequest {
    pub target: ResourceRef,
    pub destination: DirectoryRef,
    pub preconditions: Preconditions,
}
pub struct DeleteRequest {
    pub target: ResourceRef,
    pub preconditions: Preconditions,
}
pub struct TrashRequest {
    pub target: ResourceRef,
    pub preconditions: Preconditions,
}
pub struct RestoreRequest {
    pub target: ResourceRef,
    pub destination: Option<DirectoryRef>,
    pub preconditions: Preconditions,
}
```

`UploadRequest` cria um recurso remoto a partir de um arquivo local. `DownloadRequest` cria um recurso local a partir de um arquivo remoto. Os métodos de criação exigem destino livre ou retornam erro de conflito; a operação de atualização é usada para alterar um recurso já identificado.

`UpdateContentRequest` exige um alvo existente. A origem do conteúdo pode ser local, remota ou um conjunto de bytes. A viabilidade de cada combinação é determinada pelas capacidades técnicas disponíveis, retornando erro tipado quando não suportada. `MoveRequest` exige origem e destino no mesmo ambiente e, quando remoto, no mesmo contexto compatível do serviço.

## 4. Resultados

```rust
pub enum VerificationStatus {
    Verified,
    Unverified,
}

pub struct ItemResult {
    pub resource: ResourceRef,
    pub metadata: ItemMetadata,
    pub verification: VerificationStatus,
}

pub struct TransferResult {
    pub item: ItemResult,
    pub bytes_transferred: u64,
    pub integrity: IntegrityStatus,
}

pub enum IntegrityStatus {
    Verified,
    NotAvailable,
}

pub struct DeleteResult {
    pub target: ResourceRef,
    pub verification: VerificationStatus,
}
```

`ItemMetadata` representa os atributos comprovadamente disponíveis após a execução, incluindo identidade, nome, localização, versão e informações de conteúdo quando fornecidas. Campos não obtidos são opcionais; não são preenchidos por inferência.

`Verified` exige evidência da alteração esperada. `Unverified` indica conclusão reportada sem evidência suficiente para confirmá-la. Uma execução com efeito **indeterminado** deve resultar em erro tipado apropriado e conservar informação suficiente para verificar o efeito antes de nova tentativa.

## 5. Contrato funcional por método

| Método | Entrada essencial | Resultado | Regras de execução |
| --- | --- | --- | --- |
| `new` | Configuração e interfaces de execução | `Operator` | Prepara a seleção de executores e contextos necessários |
| `create_file` | Diretório, nome, conteúdo opcional | `ItemResult` | Cria um recurso novo; nome inválido ou ocupado produz erro |
| `create_directory` | Diretório pai e nome | `ItemResult` | Cria diretório e retorna a identidade obtida |
| `upload` | Arquivo local, diretório remoto, nome | `TransferResult` | Cria arquivo remoto, transfere bytes e verifica a conclusão disponível |
| `download` | Arquivo remoto, diretório local, nome | `TransferResult` | Transfere bytes e publica o arquivo local com segurança |
| `update_content` | Recurso existente, conteúdo, precondições | `TransferResult` | Atualiza conteúdo preservando identidade do destino |
| `rename` | Recurso, novo nome, precondições | `ItemResult` | Altera o nome preservando identidade quando suportado |
| `move_item` | Recurso, novo diretório, precondições | `ItemResult` | Move dentro do ambiente, preservando identidade quando suportado |
| `delete` | Recurso, precondições | `DeleteResult` | Aplica exclusão definitiva somente ao alvo identificado |
| `trash` | Recurso, precondições | `ItemResult` | Aplica envio à lixeira quando disponível |
| `restore` | Recurso, destino opcional, precondições | `ItemResult` | Restaura recurso quando disponível e retorna estado observado |

## 6. Garantias compartilhadas

- **Identidade:** operações utilizam IDs remotos e caminhos locais validados, evitando correlação por nome como substituto da identidade.
- **Precondições:** quando fornecidas, são verificadas antes de alterações; condições remotas devem preferir mecanismos atômicos disponibilizados pelo provedor.
- **Integridade:** transferências validam conteúdo e metadados disponíveis; a verificação realizada é explicitada no resultado.
- **Atomicidade local:** substituições de conteúdo devem utilizar escrita temporária e publicação segura quando aplicável.
- **Retomada:** transferências interrompidas podem continuar a partir de progresso comprovado quando tecnicamente suportado.
- **Erros:** `OperatorError` distingue recurso inexistente, acesso negado, conflito, precondição não satisfeita, operação não suportada, falta de espaço, falha de I/O/rede, cancelamento e efeito indeterminado.
- **Progresso:** atualizações informam bytes transferidos e fases significativas quando disponíveis; ausência de progresso não implica ausência de execução.
- **Contexto remoto:** chamadas de serviços externos são realizadas pelas interfaces de `providers/`, com seleção do provedor correspondente.

## 7. Biblioteca privada de operações locais

O próprio `Operator` disponibiliza funções internas reutilizáveis para manipular arquivos e diretórios no sistema de arquivos. Essas funções compõem uma **API privada de apoio à execução**: as implementações especializadas podem utilizá-las diretamente, sem precisar de um submódulo arquitetural adicional.

A biblioteca oferece operações locais elementares, com resultados tipados e garantias de segurança. A organização física dessas funções em arquivos Rust é um detalhe de implementação.

### 7.1. Contratos internos propostos

```rust
// Sketches of crate-private contracts; concrete ownership is defined in Rust.
pub(crate) fn open_local_resource(
    scope: &LocalScope,
    path: &Path,
) -> OperatorResult<LocalResourceHandle>;

pub(crate) fn local_metadata(
    resource: &LocalResourceHandle,
) -> OperatorResult<LocalMetadata>;

pub(crate) fn read_local(
    resource: &LocalResourceHandle,
) -> OperatorResult<LocalReader>;

pub(crate) fn create_local_directory(
    parent: &LocalDirectoryHandle,
    name: &OsStr,
) -> OperatorResult<LocalDirectoryHandle>;

pub(crate) fn create_local_file(
    parent: &LocalDirectoryHandle,
    name: &OsStr,
) -> OperatorResult<LocalResourceHandle>;

pub(crate) fn create_temp_file(
    parent: &LocalDirectoryHandle,
) -> OperatorResult<TemporaryLocalFile>;

pub(crate) fn write_local(
    target: &mut TemporaryLocalFile,
    source: &mut dyn Read,
    options: &LocalIoOptions,
) -> OperatorResult<LocalWriteResult>;

pub(crate) fn atomic_replace(
    temporary: TemporaryLocalFile,
    destination: &LocalResourceHandle,
    preconditions: &LocalPreconditions,
) -> OperatorResult<LocalResourceHandle>;

pub(crate) fn rename_local(
    resource: &LocalResourceHandle,
    new_name: &OsStr,
    preconditions: &LocalPreconditions,
) -> OperatorResult<LocalResourceHandle>;

pub(crate) fn move_local(
    resource: &LocalResourceHandle,
    destination: &LocalDirectoryHandle,
    preconditions: &LocalPreconditions,
) -> OperatorResult<LocalResourceHandle>;

pub(crate) fn delete_local(
    resource: &LocalResourceHandle,
    preconditions: &LocalPreconditions,
) -> OperatorResult<LocalDeleteResult>;

pub(crate) fn verify_local_integrity(
    resource: &LocalResourceHandle,
    expected: &IntegrityEvidence,
) -> OperatorResult<IntegrityStatus>;
```

As assinaturas são **conceituais**. `LocalScope`, `LocalResourceHandle`, `LocalDirectoryHandle`, `TemporaryLocalFile`, `LocalReader`, `LocalMetadata`, `LocalPreconditions`, `LocalIoOptions`, `LocalWriteResult`, `LocalDeleteResult` e `IntegrityEvidence` representam contratos internos a definir na implementação; estes nomes não afirmam a existência dos tipos no código atual.

### 7.2. Parâmetros e resultados

| Função | Entradas | Resultado |
| --- | --- | --- |
| `open_local_resource` | Escopo autorizado e caminho do recurso | Referência local validada |
| `local_metadata` | Referência local | Metadados efetivamente obtidos |
| `read_local` | Referência local | Leitor de conteúdo em fluxo |
| `create_local_directory` | Diretório pai e nome | Referência do diretório criado |
| `create_local_file` | Diretório pai e nome | Referência do arquivo criado |
| `create_temp_file` | Diretório pai | Arquivo temporário associado ao destino |
| `write_local` | Arquivo temporário, fonte de bytes e opções de I/O | Quantidade de bytes escritos e estado da escrita |
| `atomic_replace` | Arquivo temporário, destino existente e precondições | Referência do arquivo publicado |
| `rename_local` | Recurso, novo nome e precondições | Referência atualizada do recurso |
| `move_local` | Recurso, diretório de destino e precondições | Referência atualizada do recurso |
| `delete_local` | Recurso e precondições | Resultado da exclusão |
| `verify_local_integrity` | Recurso e evidência esperada | Resultado da verificação de integridade |

`LocalScope` delimita a raiz autorizada; as referências locais validadas mantêm o vínculo com esse escopo. Nomes locais usam `OsStr` para representar corretamente nomes de arquivos Linux, inclusive quando não são UTF-8. `LocalIoOptions` pode transportar cancelamento e notificações de progresso, conforme o contrato geral de execução.

### 7.3. Garantias da biblioteca

- **Escopo e caminhos:** resolver recursos dentro da raiz autorizada, tratando caminhos relativos, componentes `..`, links simbólicos e mudanças concorrentes sem escapar do escopo. Preferir operações relativas a descritores de diretório e verificações resistentes a TOCTOU quando aplicável.
- **Identidade:** validar recurso e destino antes de alterações; nomes semelhantes não constituem prova de identidade.
- **Criação:** criar entradas com a política de colisão expressa pelo comando, evitando substituição acidental.
- **Leitura e escrita:** processar conteúdo em fluxo, incluindo arquivos grandes, com tratamento de leituras/escritas parciais e interrupções.
- **Arquivos temporários:** criar temporários com nomes seguros, permissões adequadas e limpeza controlada. Um temporário não é um arquivo final publicado.
- **Substituição segura:** concluir e verificar a escrita antes da publicação; aplicar precondições sobre o destino e utilizar substituição atômica quando a plataforma e o sistema de arquivos oferecerem essa garantia. Diferenciar atomicidade de visibilidade de durabilidade após queda de energia.
- **Renomeação e movimentação:** preservar a identidade sempre que a operação subjacente permitir; movimentações entre sistemas de arquivos podem exigir tratamento explícito de incompatibilidade.
- **Exclusão:** atuar sobre o recurso identificado e retornar evidência do efeito observado, distinguindo exclusão definitiva de operações de lixeira.
- **Integridade:** verificar tamanho ou hash quando houver evidência confiável; distinguir `Verified` de `NotAvailable` sem inventar validação.
- **Erros:** converter falhas locais para `OperatorError` preservando a causa técnica necessária ao diagnóstico, sem declarar sucesso parcial como conclusão.

A biblioteca pode oferecer funções auxiliares menores conforme a implementação exigir, mantendo essas garantias e o acesso privado ao módulo `operations/`.

## 8. Definições para implementação

As estruturas ilustram a forma do contrato. Antes da implementação Rust, definir os tipos concretos de IDs e metadados, a representação de referências locais seguras, a forma de injetar a sessão remota, o tratamento de progresso/cancelamento e a compatibilidade entre capacidades de cada ambiente. Essas decisões preservam os objetivos e os métodos públicos acima.
