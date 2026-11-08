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

## 7. Definições para implementação

As estruturas ilustram a forma do contrato. Antes da implementação Rust, definir os tipos concretos de IDs e metadados, a representação de referências locais seguras, a forma de injetar a sessão remota, o tratamento de progresso/cancelamento e a compatibilidade entre capacidades de cada ambiente. Essas decisões preservam os objetivos e os métodos públicos acima.
