# Models — núcleo de domínio da engine

## Objetivo

`models/` é o **núcleo de representação de dados do DriveSync**: os módulos funcionais compartilham os mesmos objetos de arquivo e de observação. O núcleo **não implementa serviços**; não acessa Google API, filesystem, SQLite, filas, nem resolve conflitos.

Os atributos estão especificados em [INTERFACES.md](INTERFACES.md) como **proposta fechada para revisão do responsável**. Como a implementação futura deverá reproduzir literalmente a documentação aprovada, **não considerar o código Rust ilustrativo como já implementado ou autorizado** até aprovação explícita.

## Hierarquia e proprietários

```text
detectors/
└── FileBatch
    └── files: Vec<File>
        models/
        └── File
            └── data: Option<FileData>
                ├── Local(LocalFile)
                └── Remote(RemoteFile)
```

| Tipo | Proprietário | Responsabilidade |
| --- | --- | --- |
| `LocalFile` | `models/` | Fotografia dos metadados locais realmente observados |
| `RemoteFile` | `models/` | Fotografia normalizada dos metadados remotos e dados especializados opacos |
| `FileData` | `models/` | Escolher **exatamente uma** representação, local ou remota, sem comportamento |
| `File` | `models/` | Encapsular uma **ocorrência autocontida**, com dados, contexto e escopo |
| `FileBatch` | [`detectors/`](../detectors/INTERFACES.md) | Agrupar ocorrências para entrega segura, identificação de lote e confirmação |

O **Orquestrador recebe `FileBatch` e encaminha seus `File` sem distinguir origem**. O Analisador interpreta `FileData`. O Detector pai/Storage preserva o objeto completo para reentrega; não converte a notificação em um mero caminho ou ID.

## Atributos de LocalFile

| Nome | Tipo | Origem | Semântica |
| --- | --- | --- | --- |
| `local_id` | `Option<LocalFileId>` | Detector Local | Identidade estável comprovada, quando existir |
| `relative_path` | `LocalRelativePath` | Detector Local | Caminho relativo à raiz, preservando bytes Unix |
| `kind` | `FileKind` | Detector Local | Tipo observado; pode ser `Unknown` |
| `size_bytes` | `Option<u64>` | Detector Local | Tamanho, se medido |
| `modified_at` | `Option<Timestamp>` | Detector Local | mtime observado, sem inventar instante |
| `hash` | `Option<FileHash>` | Detector Local | Hash somente se realmente calculado |

`LocalFile` é um **valor**, não um observador. A leitura do filesystem pertence ao Detector Local. `None` em atributo opcional não representa exclusão, zero ou outra afirmação que não foi observada.

## Atributos de RemoteFile

| Nome | Tipo | Origem | Semântica |
| --- | --- | --- | --- |
| `reference` | `RemoteFileRef` | Especialista remoto | Identidade qualificada por conta |
| `provider` | `ProviderId` | Contexto da conta | Serviço ao qual o recurso pertence |
| `name` | `String` | Serviço remoto | Nome observado |
| `kind` | `FileKind` | Especialista remoto | Tipo normalizado, inclusive nativo do provedor |
| `mime_type` | `Option<String>` | Serviço remoto | MIME quando conhecido |
| `size_bytes` | `Option<u64>` | Serviço remoto | Tamanho quando disponível |
| `modified_at` | `Option<Timestamp>` | Serviço remoto | Data fornecida pelo provedor |
| `version` | `Option<String>` | Serviço remoto | Revisão/ETag opaca |
| `hash` | `Option<FileHash>` | Serviço remoto | Hash conhecido com algoritmo explícito |
| `parent_ids` | `FieldValue<Vec<RemoteFileId>>` | Serviço remoto | `Unknown` ≠ `Known([])` |
| `provider_data` | `Option<ProviderFileData>` | Especialista remoto | Metadados particulares do provedor, opacos ao núcleo |

`ProviderFileData` encapsula `provider`, `format`, `format_version` e `payload`, que contém **apenas metadados especializados**, jamais o conteúdo do arquivo, credenciais ou clientes de API. A implementação Google continua proprietária de `GoogleFile` em [`providers/google/api/`](../providers/google/api/README.md).

## Atributos de FileData e File

`FileData` não possui campos: é um `enum` com **duas variantes fechadas**:

```rust
pub enum FileData {
    Local(LocalFile),
    Remote(RemoteFile),
}
```

`File` encapsula o contexto da observação, independentemente da origem:

| Nome | Tipo | Origem | Semântica |
| --- | --- | --- | --- |
| `id` | `FileId` | Produtor do evento | ID estável da ocorrência, preservado em retry |
| `sync_id` | `SyncId` | Escopo da sincronização | Impede misturar observações de raízes distintas |
| `event` | `FileEvent` | Detector produtor | `ObservedChange`, `RecheckRequired`, `ScopeUnavailable` |
| `observed_at` | `Timestamp` | Detector produtor | Momento da observação, não mtime do conteúdo |
| `coverage` | `ObservationCoverage` | Detector produtor | `Complete`, `Partial`, `Unavailable` |
| `scope` | `FileScope` | Detector produtor | `Local(path)`, `Remote(ref)` ou `WholeSync` |
| `data` | `Option<FileData>` | Detector produtor | `Some` quando há metadados observados; `None` exige reavaliação |

**Invariantes:** `ObservedChange` exige `data=Some`; `ScopeUnavailable` exige `data=None` e cobertura `Unavailable`. Quando `data=Some`, a origem e a referência devem coincidir com `scope`. `None` **não é exclusão comprovada**.

## FileBatch — contexto externo ao Models

A [interface do Detector](../detectors/INTERFACES.md) já contém a identidade do lote e a lista de eventos de seu contrato anterior. Ao ser revisada para o novo fluxo, a **única mudança conceitual obrigatória** é que a lista de entrega passe a conter **`File` completos**, mantendo identificação/reserva/confirmação de lote na responsabilidade do Detector.

Não definir nem implementar `FileBatch` dentro de `models/`. O Orquestrador recebe um lote mesmo quando houver apenas um `File`. O Storage deve preservar a **mesma ocorrência e o mesmo conteúdo** até reconhecimento seguro, com reentrega em falhas.

## Tipos auxiliares e invariantes

`ProviderId`, `AccountId`, `SyncId`, `RemoteFileId`, `RemoteFileRef`, `LocalFileId`, `FileId`, `Timestamp`, `FileKind`, `HashAlgorithm`, `FileHash`, `LocalRelativePath`, `FieldValue<T>`, `ProviderFileData`, `FileEvent`, `ObservationCoverage` e `FileScope` constam de [INTERFACES.md](INTERFACES.md), onde estão definidos **tipos, variantes e regras de validação**.

- Identidade remota é qualificada pela conta; não confundir `RemoteFileId` com `FileId` de ocorrência.
- Caminhos locais preservam bytes Unix, são relativos à raiz e não permitem escapes por `..`.
- `Unknown` é não observado; `Known([])` prova lista observada e vazia.
- Os modelos são dados autocontidos **do momento observado**, não garantia de que o arquivo continua inalterado.
- `SyncItem` e a revisão do Inventário ficam **expressamente para outra etapa**.

## Estado e documentação

**Etapa de atributos:** proposta formal para aprovação, não implementação. Estão em aberto, antes de codificar: política concreta de payload especializado (formato/limite) e compatibilidade das identidades já existentes. Nenhuma dessas pendências autoriza o agente a inventar decisões.

[INTERFACES.md](INTERFACES.md) · [TESTS.md](TESTS.md) · [AGENTS.md](AGENTS.md).
