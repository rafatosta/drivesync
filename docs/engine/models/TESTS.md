# Models — TESTS (contratos de atributos)

## Estado e escopo

A matriz a seguir corresponde aos atributos especificados em [INTERFACES.md](INTERFACES.md), para **validação quando houver implementação**. Não há testes executados nesta revisão.

Os testes de Models devem ser **puros**, sem filesystem, SQLite, Google API, Detector ou Orquestrador. Testes de entrega, leases, retries e integração de `FileBatch` pertencem a `detectors/`.

## 1. Identidades e valores auxiliares

| ID | Alvo | Teste positivo | Teste negativo/limite |
| --- | --- | --- | --- |
| MOD-ID-01 | `RemoteFileRef` | Mesma conta + mesmo arquivo = mesma identidade | Duas contas com mesmo ID bruto ≠ mesmo recurso |
| MOD-ID-02 | `FileId` | Evento reentregue preserva identidade | Duas ocorrências distintas não compartilham ID por serem do mesmo arquivo |
| MOD-ID-03 | `LocalFileId` | Identidade verificada é preservada | Não deduzir ID de inode ou pathname |
| MOD-ID-04 | `AccountId`, `SyncId` | Reutilizar proprietário efetivo | Não criar novas definições incompatíveis |
| MOD-ID-05 | `Timestamp` | UTC unix_millis preservado | Não confundir observação com mtime |
| MOD-ID-06 | `FileHash` | Algoritmo e digest correspondem | Hash de algoritmo distinto não é considerado equivalente |
| MOD-ID-07 | `LocalRelativePath` | Preserva nomes Unix não UTF-8 | NUL, caminho absoluto, `.`, `..` são rejeitados |
| MOD-ID-08 | `FieldValue<T>` | `Unknown` e `Known([])` são distintos | `Unknown` não apaga estado conhecido |

## 2. LocalFile — atributos

| ID | Campo(s) | Casos previstos |
| --- | --- | --- |
| MOD-L-01 | `local_id` | `Some` apenas se identidade verificada; `None` permitido |
| MOD-L-02 | `relative_path` | Arquivo, subpasta, raiz vazia e bytes não UTF-8 |
| MOD-L-03 | `kind` | Regular, Directory, Symlink, ProviderNative, Other, Unknown; classificação adequada à origem |
| MOD-L-04 | `size_bytes` | `None` não se torna 0; `Some(0)` representa zero realmente observado |
| MOD-L-05 | `modified_at` | `None` não usa horário atual |
| MOD-L-06 | `hash` | `None` sem leitura de conteúdo; quando presente, algoritmo explícito |
| MOD-L-07 | isolamento | Construir/clonar/comparar dados não acessa filesystem |

## 3. RemoteFile — atributos

| ID | Campo(s) | Casos previstos |
| --- | --- | --- |
| MOD-R-01 | `reference` + `provider` | Identidade de conta qualificada, provedor consistente com conta |
| MOD-R-02 | `name` | Nome Unicode preservado sem conversão destrutiva |
| MOD-R-03 | `kind` | Arquivo normal, diretório, nativo do provedor, Unknown |
| MOD-R-04 | `mime_type` | `None` permitido; não inferir MIME |
| MOD-R-05 | `size_bytes` | `None` para tipo sem tamanho disponível; 0 só se comprovado |
| MOD-R-06 | `modified_at` | `None` permitido sem inventar instante |
| MOD-R-07 | `version` | String opaca preservada sem ordenação numérica |
| MOD-R-08 | `hash` | Não inventar hash para documento nativo ou resposta parcial |
| MOD-R-09 | `parent_ids` | `Unknown` distinto de `Known(vec![])`; preservação de IDs |
| MOD-R-10 | `provider_data` | `None` permitido; `Some` preserva envelope opaco |
| MOD-R-11 | `ProviderFileData` | Provedor interno corresponde ao remoto; formato não vazio, versão positiva |
| MOD-R-12 | `ProviderFileData` | Bytes preservados, sem interpretação por Models; rejeitar inconsistências |
| MOD-R-13 | isolamento | Construir/clonar/comparar não contata API externa |

## 4. FileData e File

| ID | Alvo | Casos previstos |
| --- | --- | --- |
| MOD-F-01 | `FileData` | Somente `Local(LocalFile)` ou `Remote(RemoteFile)`; nunca duas variantes |
| MOD-F-02 | `File.id` | Mesmo objeto mantém ID de ocorrência após cópia/serialização |
| MOD-F-03 | `File.sync_id` | Não mistura observações de duas sincronizações |
| MOD-F-04 | `event` | `ObservedChange`, `RecheckRequired`, `ScopeUnavailable` representados corretamente |
| MOD-F-05 | `observed_at` | Marca a observação, independentemente do mtime do recurso |
| MOD-F-06 | `coverage` | Complete, Partial, Unavailable distintos |
| MOD-F-07 | `scope` | Local, Remote ou WholeSync conforme evidência |
| MOD-F-08 | `data=Some(Local)` | `scope=Local(relative_path)`; qualquer contradição é inválida |
| MOD-F-09 | `data=Some(Remote)` | `scope=Remote(reference)`; qualquer contradição é inválida |
| MOD-F-10 | `data=None` | Permitido com `RecheckRequired`/`ScopeUnavailable`, nunca traduzido em exclusão |
| MOD-F-11 | `ScopeUnavailable` | Exige `coverage=Unavailable` e `data=None` |
| MOD-F-12 | `ObservedChange` | Exige `data=Some(...)` |
| MOD-F-13 | `FileScope::WholeSync` | Só sem dados de arquivo |
| MOD-F-14 | serialização futura | Round-trip conserva identidade, valores desconhecidos e dados opacos |
| MOD-F-15 | fronteira | Sem `SyncItem`, operação, credenciais ou estado de fila em `File` |

## 5. Integração posterior (fora da suíte Models)

- `detectors/local/`: criar `LocalFile` e encaminhar ocorrência completa.
- `detectors/remote/<provedor>/`: criar `RemoteFile` com os metadados e envelope opaco válidos.
- `detectors/`: compor/entregar `FileBatch`, normalmente com um `File`, mas admitindo vários.
- `detectors/storage/`: persistir **todo `File`**, sem reconstruí-lo após falha.
- `orchestrator/`: encaminhar `FileBatch` sem inspecionar variante.
- `analyzer/`: interpretar variante de `FileData`; revalidar estado antes de decidir operações.
- `inventory/`: revisão futura do `SyncItem` e contrato de persistência.

## 6. Critério de aceite futuro

A implementação precisará satisfazer **todos os campos, invariantes e testes aplicáveis**, sem atributos extras por conveniência. Casos não definidos (ex.: formato binário concreto e limite máximo de `ProviderFileData.payload`) devem ser registrados como **bloqueados antes do código**, não resolvidos por suposição.

Executar Rust/Cargo apenas em ambiente Flatpak conforme as regras do projeto. **Nenhum teste foi executado agora.**
