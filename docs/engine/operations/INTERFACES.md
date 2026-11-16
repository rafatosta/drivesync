# Operador — interfaces de execução

## 1. Escopo entregue

O módulo público `drivesync_engine::operations` fornece modelos/traits,
`DefaultOperatorFactory` e `GoogleOperator`, com as nove ações permitidas
implementadas. `delete` recusa exclusão permanente sem efeitos. Ver
[MUTATIONS.md](MUTATIONS.md) e [TRANSFERS.md](TRANSFERS.md) para pré-condições,
recuperação, integridade e limites. Operator não confirma baseline e não possui fila.

## 2. Interface pública

Cada chamada recebe requisição própria e retorna `OperationFuture<'_, T>`:
`Pin<Box<dyn Future<Output = OperatorResult<T>> + Send + '_>>`. A interface é
object-safe; futuros podem tomar emprestado o executor e recebem dados próprios.
Não depende de macros async nem de crates adicionais.

| Método obrigatório do trait `Operator` | Entrada | Resultado |
| --- | --- | --- |
| `new` | `OperatorConfig` | `OperatorResult<Self>`, com `Self: Sized` |
| `create_file` | `CreateFileRequest` | Future de `ItemResult` |
| `create_directory` | `CreateDirectoryRequest` | Future de `ItemResult` |
| `upload` | `UploadRequest` | Future de `TransferResult` |
| `download` | `DownloadRequest` | Future de `TransferResult` |
| `update_content` | `UpdateContentRequest` | Future de `TransferResult` |
| `rename` | `RenameRequest` | Future de `ItemResult` |
| `move_item` | `MoveRequest` | Future de `ItemResult` |
| `delete` | `DeleteRequest` | Future de `DeleteResult`; execução proibida |
| `trash` | `TrashRequest` | Future de `ItemResult` |
| `restore` | `RestoreRequest` | Future de `ItemResult` |

Não há métodos com corpo default. O construtor estático não integra a vtable;
o Analyzer consome um handle já composto, sem construir especialista Google.
`OperatorResult<T> = Result<T, OperatorFailure>`.

## 3. Configuração e seleção por provedor

`OperatorConfig` contém `connector: Engine`, `scope: OperatorScope` e
`journal: Arc<dyn ExecutionJournal>`, porta bloqueante implementada pelo Inventory. O escopo
contém `sync_id: SyncId`, `account_id: AccountId`, `local_root: PathBuf` e
`remote_root_id: RemoteFileId`, reutilizando IDs de Models. Não recebe token,
sessão Google, factory concreta ou provedor arbitrário informado pelo chamador.

`OperatorFactory: Send + Sync` declara:

```rust
fn create(&self, config: OperatorConfig) -> OperatorResult<Arc<dyn Operator>>;
```

A composição segue o detector remoto: resolver a conta persistida via
Engine, obter seu provedor e selecionar o especialista compatível. Provedor
incompatível deve falhar explicitamente, sem fallback. O especialista acessa
serviços externos somente pelo respectivo Provider e fará o trabalho local
necessário dentro do escopo, inclusive publicação de downloads. A seleção deve
validar conta/provedor das referências, não confiar somente nos campos recebidos.

## 4. Referências e conteúdo

| Tipo | Representação concreta |
| --- | --- |
| `ResourceRef` | `Local { relative_path: LocalRelativePath, local_id: Option<LocalFileId> }` ou `Remote { provider_id: ProviderId, reference: RemoteFileRef }` |
| `DirectoryRef` | As mesmas identidades, representando explicitamente o contêiner |
| `ItemName` | `Local(OsString)` ou `Remote(String)` |
| `ContentSource` | `LocalFile { relative_path, local_id }`, `RemoteFile { resource: ResourceRef }` ou `Bytes { data: Vec<u8> }` |
| `Preconditions` | `expected_version: Option<String>`, `expected_revision: Option<String>` |

Referências locais são relativas à raiz autorizada e reutilizam Models. O nome
local preserva bytes Unix não UTF-8. A implementação deve validar nomes,
identidades, tipos, caminhos e ambiente de origem/destino; o contrato de dados
não executa essa validação nem garante proteção TOCTOU por si só. `ResourceRef`
não é um handle aberto de filesystem. O campo `provider_id` remoto precisa ser
conferido com a conta persistida; não controla a seleção do especialista.

Nome, caminho parecido e conteúdo igual nunca associam recursos independentes.
Associação de preexistentes exige definição explícita do usuário; associação de
réplicas vem da identidade realmente criada/verificada. Nenhuma confirma baseline.

## 5. Campos das requisições

Todas as dez requisições contêm `execution: ExecutionContext` obrigatório e
`options: ExecutionOptions`. O contexto vincula ID e evidência de origem; ele não
é autorização. `delete` recusa antes de consultar contexto, journal ou credenciais.

| Requisição | Demais campos |
| --- | --- |
| `CreateFileRequest` | `parent: DirectoryRef`, `name: ItemName`, `content: Option<ContentSource>` |
| `CreateDirectoryRequest` | `parent: DirectoryRef`, `name: ItemName` |
| `UploadRequest` | `source: ResourceRef`, `destination: DirectoryRef`, `name: ItemName` |
| `DownloadRequest` | `source: ResourceRef`, `destination: DirectoryRef`, `name: ItemName` |
| `UpdateContentRequest` | `target: ResourceRef`, `source: ContentSource`, `preconditions: Preconditions` |
| `RenameRequest` | `target: ResourceRef`, `new_name: ItemName`, `preconditions: Preconditions` |
| `MoveRequest` | `target: ResourceRef`, `destination: DirectoryRef`, `preconditions: Preconditions` |
| `DeleteRequest` | `target: ResourceRef`, `preconditions: Preconditions` |
| `TrashRequest` | `target: ResourceRef`, `preconditions: Preconditions` |
| `RestoreRequest` | `target: ResourceRef`, `destination: Option<DirectoryRef>`, `preconditions: Preconditions` |

Upload/download representam criação de réplica, não atualização de destino
preexistente. Criação nunca substitui um homônimo; `update_content` usa o alvo
existente explicitamente identificado. Move não é transferência entre ambientes.
A falta de precondição não autoriza sobrescrever, associar nem escolher versão.

## 6. Progresso, resultado e falha

`ExecutionOptions` contém `cancellation: Option<Arc<dyn CancellationSignal>>` e
`progress: Option<Arc<dyn ProgressReporter>>`, independentes e opcionais.

- `CancellationSignal::is_cancelled() -> bool`: sinal cooperativo, sem implementar
  token ou prometer reversão de efeito.
- `ProgressReporter::try_report(ExecutionProgress) -> ProgressDelivery`: contrato
  não bloqueante, com retorno `Accepted` ou `Dropped`. Nenhum reporter é fornecido.
- `ExecutionProgress`: fase `Preparing`, `Transferring`, `Applying`, `Verifying`
  ou `Finalizing`, `bytes_transferred: Option<u64>` e `total_bytes: Option<u64>`.

`ItemResult` contém `resource: ResourceRef`, `metadata: Option<FileData>` de
Models e `verification: VerificationStatus` (`Verified`/`Unverified`). Campos
não observados não são fabricados; não há novo modelo duplicado de metadados.
`TransferResult` contém `item`, `bytes_transferred: u64` e `integrity`
(`Verified`/`NotAvailable`). `DeleteResult` conserva `target` e `verification`,
mas sua existência não autoriza produzir sucesso de exclusão permanente.

`OperatorFailure` contém `kind: OperatorError`, `effect: ExecutionEffect`
(`NotApplied`, `Applied`, `Unknown`) e `resource: Option<ResourceRef>`. Não contém
texto bruto do Provider, tokens ou credenciais. Categorias cobrem configuração,
entrada, escopo, provedor/operação não suportados, exclusão permanente proibida,
recurso inexistente/inacessível, permissão, destino ocupado, precondição,
integridade, espaço, I/O, rede, cancelamento, efeito indeterminado e falha interna.
Efeito desconhecido exige verificação antes de repetir, não retry presumidamente seguro.

## 7. Contrato de implementação especializada

O especialista implementa diretamente o trait `Operator`, sem segundo contrato
redundante. Google possui implementação compilada; Example permanece ilustrativo.
Exclusão permanente é proibida: `delete` retorna `PermanentDeletionProhibited` sem
I/O, autenticação ou fallback. As demais ações validam capacidade da plataforma,
escopo e precondições; um método implementado pode recusar uma capacidade ausente.

## Dados de planejamento do Analyzer

`OperationCommand` descreve nove ações permitidas como dados serializáveis:
criação de arquivo/pasta, upload, download, atualização de conteúdo, renomeação,
movimentação, lixeira e restauração. Não contém variante de exclusão permanente,
executor, callback ou credencial. `DirectoryRef::LocalRoot` identifica a raiz local
configurada. O Inventory persiste esses dados e seus bloqueios; o Analyzer despacha novas criações conforme sua política de execução.

## Recuperação durável — etapa 1

Veja [RECOVERY.md](RECOVERY.md) para identidade imutável, evidência de origem,
checkpoints com CAS, efeitos incertos e identidade após publicação. Upload/download possuem executor e seleção concreta; não há liberação de operações
de análise nesta entrega. Consulte [TRANSFERS.md](TRANSFERS.md).
