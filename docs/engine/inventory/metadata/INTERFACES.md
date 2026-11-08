# Metadata — contrato interno

**Estado:** contrato proposto, não código implementado. `inventory/metadata/` persiste e reconstrói os **campos selecionados** de `LocalFile` e `RemoteFile` definidos em [Models](../../models/INTERFACES.md). Não coleta metadados nem conhece `GoogleFile`, HTTP, filesystem ou o especialista remoto.

## Responsabilidades

- **local:** referência local e caminho relativo em bytes, tipo, modo, dono, tempos, armazenamento, ACL, xattrs, links e demais grupos de `LocalFile`, **somente após revisão campo a campo**.
- **remote:** `RemoteFileRef` qualificado pela conta, `ProviderId`, grupos normalizados de `RemoteFile`, relações de pais e `ProviderFileData` sanitizado/formatado quando aprovado.
- Reconstrução das representações opcionais de `SyncItem` com **semântica explícita** para `Known`, `NotCollected`, `Unsupported`, `PermissionDenied` e `Unavailable`.
- Persistir snapshots sem atualizar `SyncBaseline` e sem modificar a identidade lógica; ligação ao item pertence a `items/`.

## Portas internas propostas

```rust
pub(crate) trait LocalMetadataRepository {
    fn apply(&self, tx: &mut InventoryTx<'_>, item_id: &SyncItemId,
             value: &LocalFile) -> InventoryResult<()>;
    fn load(&self, tx: &mut InventoryTx<'_>, item_id: &SyncItemId)
        -> InventoryResult<Option<LocalFile>>;
}

pub(crate) trait RemoteMetadataRepository {
    fn apply(&self, tx: &mut InventoryTx<'_>, item_id: &SyncItemId,
             value: &RemoteFile) -> InventoryResult<()>;
    fn load(&self, tx: &mut InventoryTx<'_>, item_id: &SyncItemId)
        -> InventoryResult<Option<RemoteFile>>;
}
```

Essas portas são **internas propostas**, não `InventoryManager.record_local/record_remote`. A fachada valida `File`, `FileData::Local/Remote`, `PresenceState`, `ObservationProof` e `expected_revision` antes de solicitar `apply`. O repositório não interpreta eventos nem decide presença.

## Contrato de atualização parcial

| Valor recebido | Persistência |
| --- | --- |
| `MetadataValue::Known(v)` | Atualiza **somente** o campo efetivamente conhecido; inclusive `0`, `false` e vetor comprovadamente vazio |
| `NotCollected` | Preserva valor anterior confiável; não escreve `NULL` para simular ausência |
| `Unsupported` | Registra incapacidade se essa informação for aprovada; não apaga valor anterior sem política explícita |
| `PermissionDenied` | Preserva a última evidência, registra indisponibilidade de acesso conforme política |
| `Unavailable` | Preserva evidência histórica e registra a limitação de observação |
| `File.data=None` | **Nenhum `apply` de metadados**; encaminhar reavaliação a fachada/Analisador |

**Ponto em aberto:** a representação persistida de um campo agora inacessível pode exigir separar o último valor conhecido do **estado de coleta mais recente**. Nenhum codec ou DDL é aprovado até essa revisão. Não afirmar round-trip completo de campos deliberadamente descartados. O retorno de `load` não pode fabricar um `Known` não persistido.

## Identidade, integridade e privacidade

`RemoteFile.reference` contém `account_id` e `file_id`; associar `provider` coerente com a conta/escopo do Inventário. Um arquivo remoto pode ter vários pais, quando observados. `ProviderFileData` não é credencial nem blob de conteúdo; manter envelope versionado **apenas se** selecionado após avaliação de privacidade, tamanho, codec e suporte. Nenhum arquivo do usuário deve ser lido para calcular hashes dentro deste submódulo; `LocalFile` não incorpora hash derivado só para persistência.

## Integração pública e testes

Atende internamente `record_local`, `record_remote`, `get_item`, `list_items`; `items/` mantém associação, `storage/` transação, `baseline/` estado confirmado. Testes de round-trip **dos atributos aprovados**, preservação de bytes de caminhos Linux, metadados parciais, remoção de acesso, unicidade de `RemoteFileRef`, vários pais, codec filtrado, rollback e reabertura do SQLite.

**Pendente:** lista final `manter/sob demanda/descartar` para cada campo, mapeamento relacional, limites de payload, codificação de `ProviderFileData` e semântica de atualização dos estados não conhecidos.
