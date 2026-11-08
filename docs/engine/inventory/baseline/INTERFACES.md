# Baseline — contrato interno

**Estado:** proposta de porta interna, sem implementação. `inventory/baseline/` armazena `SyncBaseline` e `BaselineSide` já definidos em [Inventário](../INTERFACES.md). Seu conteúdo representa o **último estado efetivamente confirmado** nos dois lados, não o último evento detectado.

## Responsabilidades e invariantes

- Recuperar baseline opcional de cada `SyncItemId`; `None` é válido antes da primeira confirmação.
- Persistir baseline **somente dentro de `InventoryManager.confirm_sync`**, com `expected_item_revision`, evidência de operação verificada e transação compartilhada.
- A confirmação de baseline pode ocorrer junto de conclusão de `SyncOperation`, atualização de status de item e inserção de `SyncActivity`: **tudo ou nada**.
- Nunca avançar baseline por `record_local`, `record_remote`, `FileEventKind::Modified`, `acknowledge` de `FileBatch`, retorno HTTP de upload/download ainda não verificado ou mera alteração de status.
- Conservar presença por lado (`Unknown/Present/AbsentConfirmed`), nome local em bytes, identidade de pai, versões, checksums e timestamps **se comprovados**. O histórico confirmado não deve ser substituído por campos parciais atuais.
- Não decidir conflitos nem fazer comparação entre anterior/local/remoto: essa política pertence ao Analisador.

## Porta interna proposta

```rust
pub(crate) trait BaselineRepository {
    fn load(&self, tx: &mut InventoryTx<'_>, item_id: &SyncItemId)
        -> InventoryResult<Option<SyncBaseline>>;
    fn replace_confirmed(&self, tx: &mut InventoryTx<'_>,
                         item_id: &SyncItemId, baseline: &SyncBaseline)
        -> InventoryResult<()>;
}
```

`replace_confirmed` tem **visibilidade interna** e só pode ser chamado pelo fluxo controlado de `confirm_sync`, **após validação**, usando a mesma transação dos outros repositórios. Não oferecer método público independente `set_baseline`.

## Casos de teste

1. `baseline=None` no item recém-descoberto; observação local/remota não cria baseline.
2. Upload/download parcialmente bem-sucedido não altera baseline.
3. Falha ao concluir operação/inserir atividade reverte baseline.
4. Confirmação concorrente com `expected_item_revision` incorreta não modifica o estado.
5. Reinício preserva exatamente a última confirmação; timestamps não são preenchidos artificialmente.
6. Ausência de hash para diretório/documento nativo remoto é válida.

**Pendente:** tipagem final de `BaselineSide.parent_identity` (hoje `Option<String>`), critérios de validade do estado confirmado e política de baselines para itens tombstone.
