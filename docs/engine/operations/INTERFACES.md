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

## 7. Contrato de implementação especializada

Cada provedor possui um executor que implementa **todos os métodos** definidos pela interface `OperationExecutor`. A fachada pública identifica o provedor e a conta da sessão e encaminha a operação para a implementação correspondente.

```rust
// Assinaturas conceituais; a estratégia de async e despacho será definida em Rust.
pub(crate) trait OperationExecutor: Send + Sync {
    async fn create_file(&self, request: CreateFileRequest) -> OperatorResult<ItemResult>;
    async fn create_directory(&self, request: CreateDirectoryRequest) -> OperatorResult<ItemResult>;
    async fn upload(&self, request: UploadRequest) -> OperatorResult<TransferResult>;
    async fn download(&self, request: DownloadRequest) -> OperatorResult<TransferResult>;
    async fn update_content(&self, request: UpdateContentRequest) -> OperatorResult<TransferResult>;
    async fn rename(&self, request: RenameRequest) -> OperatorResult<ItemResult>;
    async fn move_item(&self, request: MoveRequest) -> OperatorResult<ItemResult>;
    async fn delete(&self, request: DeleteRequest) -> OperatorResult<DeleteResult>;
    async fn trash(&self, request: TrashRequest) -> OperatorResult<ItemResult>;
    async fn restore(&self, request: RestoreRequest) -> OperatorResult<ItemResult>;
}
```

**Conformidade:** todas as operações fazem parte do contrato. Se o serviço não permitir uma delas, o método correspondente retorna um erro tipado de operação não suportada, preservando os dados. Não simular sucesso nem inventar uma implementação alternativa de efeito destrutivo.

**Sessão e identidade:** cada execução é vinculada ao provedor e à conta da requisição; a implementação valida o recurso remoto pelo ID correspondente. Upload e download representam criação por transferência, enquanto `update_content` altera um recurso existente e preserva sua identidade.

**Execução:** operações remotas utilizam exclusivamente as funções do respectivo [Provider](../providers/README.md). Os executores podem reutilizar a biblioteca privada de filesystem documentada abaixo.

**Validação:** a mesma [suíte de conformidade](TESTS.md#conformidade-dos-executores-especializados) se aplica a todos os executores concretos.

## 8. Biblioteca privada de operações locais

O próprio `Operator` disponibiliza funções internas reutilizáveis para manipular arquivos e diretórios no sistema de arquivos. Essas funções compõem uma **API privada de apoio à execução**: as implementações especializadas podem utilizá-las diretamente, sem precisar de um submódulo arquitetural adicional.

A biblioteca oferece operações locais elementares, com resultados tipados e garantias de segurança. A organização física dessas funções em arquivos Rust é um detalhe de implementação.

### 8.1. Contratos internos propostos

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

### 8.2. Parâmetros e resultados

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

### 8.3. Garantias da biblioteca

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

### 8.4. Comportamento verificável de cada função

Os contratos abaixo detalham o efeito esperado de cada função da biblioteca privada. Todas as funções retornam `OperatorResult<T>`: a implementação deve distinguir entrada inválida, recurso inexistente, acesso negado, colisão, precondição divergente e erro de I/O conforme aplicável.

#### `open_local_resource(scope, path)`

Resolve `path` dentro da raiz de `scope`, valida a existência do recurso e retorna uma referência associada ao objeto efetivamente aberto. A referência deve conservar o escopo e permitir operações seguras contra troca de caminhos entre validação e uso. Falha quando o recurso não existe, está fora da raiz ou não pode ser aberto com segurança. Links simbólicos que escapem do escopo não podem ser seguidos.

#### `local_metadata(resource)`

Consulta os atributos efetivamente disponíveis do recurso identificado, incluindo tipo, tamanho e informações temporais/identidade quando fornecidas pelo sistema. A saída distingue campos conhecidos de dados não disponíveis. Falha em caso de referência inválida, acesso negado ou erro de leitura; não substitui metadados ausentes por valores presumidos.

#### `read_local(resource)`

Abre uma fonte de leitura em fluxo para o recurso identificado, sem carregar seu conteúdo inteiro na memória. Deve ler bytes sem alterá-los e propagar erros de leitura, permissão e mudanças incompatíveis de recurso. Diretórios e tipos não legíveis como conteúdo regular produzem erro tipado.

#### `create_local_directory(parent, name)`

Cria exatamente um diretório filho de `parent`, com `name` validado como componente individual, e retorna referência ao diretório criado. Não atravessa componentes de caminho no nome, não substitui entradas existentes e informa colisões ou falta de permissão. A criação recursiva de ancestrais, quando necessária, exige chamadas explícitas ou contrato adicional.

#### `create_local_file(parent, name)`

Cria um arquivo regular vazio no diretório pai, com criação exclusiva e permissões adequadas, e devolve a referência ao novo arquivo. Uma entrada existente deve permanecer intacta; nomes inválidos e destinos indisponíveis retornam erro.

#### `create_temp_file(parent)`

Cria um arquivo temporário exclusivo no diretório indicado, em condições apropriadas para posterior publicação no mesmo filesystem. Retorna um identificador que permite escrever, verificar, publicar ou descartar o temporário. Em falhas anteriores à publicação, o arquivo final permanece intacto; a retenção de temporários para retomada deve ser explícita e controlada.

#### `write_local(target, source, options)`

Copia bytes de `source` para o arquivo temporário, em fluxo, tratando leituras/escritas parciais e interrupções de I/O. Retorna a quantidade **efetivamente escrita** e informações observadas da operação. Progresso é baseado em bytes confirmados pela escrita; cancelamento interrompe novas gravações e produz resultado apropriado, preservando o estado consistente do temporário para limpeza ou retomada segura.

#### `atomic_replace(temporary, destination, preconditions)`

Publica o temporário como conteúdo do destino identificado, após finalizar a escrita e validar as precondições aplicáveis ao recurso atual. Quando a plataforma oferecer publicação atômica no mesmo filesystem, leitores observam a versão anterior ou a nova, nunca um arquivo final parcialmente escrito. Uma precondição divergente mantém o destino anterior. A função informa o estado efetivamente publicado e erros de durabilidade quando aplicáveis; atomicidade de visibilidade não equivale automaticamente a persistência após queda de energia.

#### `rename_local(resource, new_name, preconditions)`

Renomeia somente o componente final do recurso dentro do diretório pai, validando o novo nome e as precondições. Preserva o conteúdo e a identidade do recurso quando o filesystem permitir; retorna referência atualizada. Colisão, troca concorrente do recurso ou nome inválido não autorizam sobrescrita.

#### `move_local(resource, destination, preconditions)`

Move o recurso identificado para um diretório validado dentro do escopo permitido. Preserva conteúdo e identidade quando a movimentação do filesystem permitir; retorna a nova referência. Impede mover um diretório para dentro de si, sobrescrever uma entrada ocupada ou atravessar limites de escopo. Se a movimentação exigir cópia entre filesystems, retorna uma condição de operação não suportada até que exista contrato seguro específico.

#### `delete_local(resource, preconditions)`

Remove a entrada exata identificada, após validar as precondições sobre o recurso atual. A função distingue arquivos de diretórios e informa o efeito observado; a política sobre remoção de diretórios não vazios deve ser definida explicitamente antes da implementação. Referências obsoletas, recurso já ausente ou acesso negado retornam erros tipados, sem remover outras entradas.

#### `verify_local_integrity(resource, expected)`

Obtém a evidência aplicável sobre o conteúdo local (por exemplo, tamanho ou hash) e compara com `expected`. Retorna `Verified` somente quando a comparação exigida foi realizada e corresponde. Evidência insuficiente resulta em `NotAvailable`; divergência comprovada deve ser representada por falha de integridade tipada, preservando o diagnóstico necessário.

#### Semântica das referências internas

As referências retornadas são vinculadas ao recurso efetivamente validado, não apenas a uma string de caminho. A implementação deve determinar como garantir essa propriedade no Linux com handles, descritores relativos e verificações de identidade; qualquer impossibilidade técnica precisa resultar em erro seguro e não em sucesso presumido.

## 9. Definições para implementação

As estruturas ilustram a forma do contrato. Antes da implementação Rust, definir os tipos concretos de IDs e metadados, a representação de referências locais seguras, a forma de injetar a sessão remota, o tratamento de progresso/cancelamento e a compatibilidade entre capacidades de cada ambiente. Essas decisões preservam os objetivos e os métodos públicos acima.
