# Items — contrato interno

**Estado:** contrato proposto para a implementação futura de `inventory/items/`. Não descreve Rust existente nem adiciona métodos públicos a `InventoryManager`.

## Propriedade

Administra **identidade lógica, vínculos, presença, diagnóstico e revisão** de `SyncItem`. Registra os atributos estruturais dos itens conforme os dados e as evidências recebidos pela interface.

### Tipos compartilhados

Usa de [Inventário](../INTERFACES.md) `SyncItemId`, `SyncItem`, `SyncStatus`, `PresenceState`, `ItemKind`, `ItemSelector`, `ItemFilter`, `ItemLinksChange`, `ItemStatusChange`, `ObservationTarget`, `FieldUpdate<T>`, `Pagination`, `Page<T>`; de [Models](../../models/INTERFACES.md), `LocalFileId`, `RemoteFileRef`, `LocalRelativePath`. **Não redefine nenhum tipo.**

## Porta interna proposta

Os nomes abaixo são **assinaturas de projeto** para orientar TDD; não são métodos públicos atuais. A transação é fornecida por `storage/`; o mecanismo de reconstrução de `SyncItem` completo é coordenado com `metadata/` e `baseline/`.

```rust
pub(crate) trait ItemRepository {
    fn resolve(&self, tx: &mut InventoryTx<'_>, selector: &ItemSelector)
        -> InventoryResult<Option<SyncItemId>>;
    fn list_ids(&self, tx: &mut InventoryTx<'_>, filter: &ItemFilter,
                pagination: &Pagination) -> InventoryResult<Page<SyncItemId>>;
    fn create(&self, tx: &mut InventoryTx<'_>, kind: ItemKind)
        -> InventoryResult<SyncItemId>;
    fn apply_links(&self, tx: &mut InventoryTx<'_>, id: &SyncItemId,
                   expected_revision: u64, changes: &ItemLinksChange)
        -> InventoryResult<()>;
    fn set_presence(&self, tx: &mut InventoryTx<'_>, id: &SyncItemId,
                    expected_revision: u64, side: ObservationSide,
                    presence: PresenceState, proof: &ObservationProof)
        -> InventoryResult<()>;
    fn set_status(&self, tx: &mut InventoryTx<'_>, id: &SyncItemId,
                  expected_revision: u64, change: &ItemStatusChange)
        -> InventoryResult<()>;
    fn load_header(&self, tx: &mut InventoryTx<'_>, id: &SyncItemId)
        -> InventoryResult<Option<ItemHeader>>;
}
```

`ObservationSide` (Local/Remote) e `ItemHeader` são **detalhes internos propostos**, não substitutos de `FileData` ou de `SyncItem`. A forma exata de `InventoryTx` e de leitura paginada é definida em [storage/INTERFACES.md](../storage/INTERFACES.md). Como Rust não define interfaces de traits a partir deste Markdown, os lifetimes e ownership são ilustrativos e precisam de revisão na implementação.

## Regras invariantes

1. `SyncItemId` permanece estável após renomeação, movimentação e retries, desde que a correspondência seja comprovada; `File.id` identifica **uma ocorrência**, não o item.
2. `RemoteFileRef(account_id,file_id)` tem unicidade por conta. Caminho, nome e inode não são identidade global; `LocalFile.local_id` pode ser `None`.
3. `FieldUpdate::Keep/Set/Clear` deve preservar a diferença entre não alterar e remover vínculo; nunca reatribuir outro item silenciosamente.
4. Revisão otimista incorreta retorna `VersionMismatch` sem gravação parcial. Qualquer escrita que mude estado incrementa a revisão uma vez por transação lógica, conforme política a fixar.
5. `AbsentConfirmed` exige `ObservationProof` suficiente; `File.data=None` ou `Deleted` isolado não autoriza ausência.
6. Preservar tombstone enquanto houver baseline, operações ou atividades referenciando o item. Nenhum `ON DELETE CASCADE` destrutivo para histórico.
7. `get_item` e `list_items` reconstruirão `SyncItem` usando metadados e baseline associados: o repositório de itens **não** os interpreta nem duplica.

## Métodos públicos atendidos indiretamente

`get_item`, `list_items`, `record_local`, `record_remote`, `link_item`, `update_item_status`, `confirm_sync`. A fachada `InventoryManager` é a única API externa.

## Testes contratuais

- Mesmo ID remoto em contas distintas; duplicidade no mesmo escopo; conflito de revisão; `FieldUpdate` nos três casos.
- Busca por cada variante de `ItemSelector`, paginação, tombstones e relações de pai válidas.
- Presença desconhecida e ausência não comprovada **não** causam exclusão; falha de escrita faz rollback.
- Testar o contrato com dados isolados e SQLite temporário.

**Pendente:** política exata para deduplicação por caminho quando `local_id=None`, critérios de prova de ausência, regras para atualização de `kind`, e plano de paginação consistente.
