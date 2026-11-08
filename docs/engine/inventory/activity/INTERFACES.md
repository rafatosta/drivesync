# Activity — contrato interno

**Estado:** proposta de contrato interno para histórico; não implementado. `inventory/activity/` persiste `SyncActivity` e `NewActivity` definidos em [Inventário — INTERFACES](../INTERFACES.md).

## Responsabilidade

Registra **fatos históricos relevantes por `SyncItem`**, com `item_id` obrigatório, `operation_id` opcional, tipo, origem, resultado, instante, descrição e snapshot mínimo de nome/ID remoto/caminho no momento da atividade. Deve permitir consulta paginada e filtros; não possui banco próprio nem registra toda notificação do Detector.

## Porta interna proposta

```rust
pub(crate) trait ActivityRepository {
    fn append(&self, tx: &mut InventoryTx<'_>, activity: &NewActivity)
        -> InventoryResult<SyncActivity>;
    fn list(&self, tx: &mut InventoryTx<'_>, filter: &ActivityFilter,
            pagination: &Pagination)
        -> InventoryResult<Page<SyncActivity>>;
}
```

A fachada expõe somente `record_activity` e `list_activities` e faz a coordenação de `confirm_sync`: se uma operação concluída produzir atividade obrigatória, `append` deve ocorrer **na mesma transação** de baseline, status e conclusão da operação. Histórico de conflitos por arquivo pode usar `record_activity`, **sem** que este módulo decida se houve conflito.

## Invariantes

1. `item_id` é FK **NOT NULL** para item lógico da mesma sincronização. Atividades globais sem arquivo pertencem a diagnóstico de outro componente.
2. `operation_id` pode ser `None` (por exemplo, conflito informativo), mas, quando presente, referencia operação do mesmo `SyncItem`.
3. O nome/caminho persistido na atividade é **histórico**, não leitura dinâmica do nome atual do item. Caminho local preserva bytes Unix sem conversão com perda.
4. Retenção de atividade não pode eliminar tombstone do item ainda referenciado; não aplicar `ON DELETE CASCADE` destrutivo.
5. Repetição da mesma confirmação após perda de resposta não duplica a atividade; a estratégia concreta de chave de idempotência ainda precisa ser definida.
6. `activity/` não lê `EventStore`, `FileBatch`, checkpoint nem progresso por chunk de transferências.

## Testes

Inclusão independente de conflito; histórico associado a operação; FK e isolamento de sincronização; ordenação/paginação por data; filtros; tombstone; rollback de confirmação conjunta; deduplicação de retry; strings de erro sanitizadas.

**Pendente:** política de retenção, chave estável para idempotência de histórico, filtragem de descrições/dados pessoais e ordenação determinística para itens com timestamps iguais.
