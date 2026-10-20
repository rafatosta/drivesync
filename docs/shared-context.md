# Dados compartilhados e ownership

Não existe objeto global mutável de acesso irrestrito. Os dados são acessados por interfaces tipadas e responsáveis claros.

| Categoria | Modelos | Dono da escrita |
|---|---|---|
| Identidade/configuração | AccountId, SyncRootId, SyncPolicy | accounts/configuration |
| Estado durável | BaseVersion, ChangeEvent, Cursor, SyncOperation, ConflictRecord | state (operações coordenadas) |
| Contexto transitório | OperationId, CancellationToken, CorrelationId, progresso | coordinator |
| Credenciais | access/refresh tokens | auth / armazenamento seguro |

## SyncContext conceitual
Contém apenas AccountId, SyncRootId, OperationId, CancellationToken e política validada ou referência imutável. **Não transporta refresh tokens**.

## Fluxo de dados
Coordinator obtém snapshots do StateRepository e observações de LocalInventory/RemoteProvider; passa **valores imutáveis** ao Planner; recebe SyncDecision; registra SyncOperation; executa via adaptadores; persiste confirmação transacional.

## Invariantes
- Nenhum módulo pode modificar diretamente o armazenamento privado de outro.
- Credenciais não entram em logs, eventos nem modelos de domínio compartilhados.
- UNKNOWN não significa ABSENT.
- BaseVersion somente é confirmada após efeitos validados.
- Resultados ambíguos exigem reconciliação antes de repetição.
