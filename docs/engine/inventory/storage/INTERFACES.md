# Storage — contrato interno

**Estado:** infraestrutura e portas propostas para `inventory/storage/`. É o **único submódulo do Inventário que administra conexões, transações, esquema e erros SQLite**. Disponibiliza operações de armazenamento aos repositórios internos do inventário.

## Propriedade e fronteira

- Um banco `<raiz-local>/.drivesync/inventory.sqlite3` **por sincronização**, validado por `SyncIdentity` (conta, `SyncId`, raiz remota). Não criar banco silenciosamente em pasta ocupada ou substituir banco incompatível.
- Esquema e relações conceituais de `sync_info`, `sync_items`, `local_files`, `remote_files`, `remote_file_parents`, `sync_baselines`, `sync_operations`, `activity`, conforme [README](../README.md). **Sem** `remote_cursor` ou `remote_events`.
- Centralizar abertura, `PRAGMA` apropriados após decisão técnica, integridade, transação e rollback. Traduzir erros para `InventoryFailure` sem expor SQL, caminhos pessoais ou conteúdo.
- `items/`, `metadata/`, `baseline/`, `operations/`, `activity/` acessam apenas o **contexto de transação interno**, sem abrir conexões independentes ou efetuar commits autônomos.

## Portas internas propostas

```rust
// Esboço contratual. InventoryTx e seus lifetimes ainda não são tipos Rust implementados.
pub(crate) trait InventoryStorage {
    fn open(&self, request: &OpenInventoryRequest)
        -> InventoryResult<StorageOpenResult>;
    fn read<T>(&self, work: impl FnOnce(&mut InventoryTx<'_>) -> InventoryResult<T>)
        -> InventoryResult<T>;
    fn transaction<T>(&self,
                      work: impl FnOnce(&mut InventoryTx<'_>) -> InventoryResult<T>)
        -> InventoryResult<T>;
}

pub(crate) struct StorageOpenResult {
    // Contexto privado de armazenamento + SyncInfo validado.
    // Inclui InventoryRecoveryOutcome::{Normal, Restored, Rebuilt}.
}

pub(crate) struct InventoryTx<'a> {
    // Contexto de leitura/escrita privado da transação, não uma Connection pública.
}
```

A forma acima é **ilustrativa**: o `impl Trait` em método de trait, retorno com `InventoryTx<'_>` e lifetimes precisam de revisão de ergonomia Rust. Nenhum desses tipos ou métodos deve escapar pela API pública. A abertura de `InventoryManager` continua sendo `open(OpenInventoryRequest)`.

## Fronteira de transação

```mermaid
sequenceDiagram
    participant M as InventoryManager
    participant S as storage
    participant I as items
    participant B as baseline
    participant O as operations
    participant A as activity
    M->>S: transaction(work)
    S-->>M: InventoryTx
    M->>I: validar item e revision
    M->>B: replace_confirmed
    M->>O: complete_verified
    M->>I: atualizar status
    M->>A: append
    alt todas as operações válidas
        M->>S: commit
    else alguma falha
        M->>S: rollback integral
    end
```

O diagrama é **lógico**; a propriedade de commit/rollback permanece com `storage/`, mesmo que o contexto seja passado aos repositórios como empréstimo. `confirm_sync` é uma única transação: se falhar histórico, operação, baseline ou status, nada fica parcialmente concluído.

## Diagnóstico de disponibilidade sem SQL público

O Storage é o proprietário físico da disponibilidade do SQLite. Internamente mantém snapshot de `Ready`, `Recovering` ou `Unavailable` e o último resultado da abertura/recuperação (`Normal`, `Restored`, `Rebuilt`). O InventoryManager converte esse diagnóstico para sua nova API pública `status(InventoryStatusRequest)`, sem expor storage, conexões ou dados internos.

O diagnóstico deve ser **consultável sem iniciar SQL nem aguardar a conclusão de transações**. É observação da última condição conhecida, não garantia de que o próximo `read` ou `transaction` será bem-sucedido. A recuperação pode ocorrer durante a abertura, antes de existir InventoryManager: nesse caso, o resultado segue por `StorageOpenResult` / `OpenInventoryResult`; uma falha de abertura permanece erro tipado.

O Storage não é serviço de sincronização: não cria `pause`, `start`, `stop`, progresso de consulta SQLite, fila de jobs ou callbacks de GUI. I/O bloqueante do SQLite deve ocorrer fora da thread principal GTK, por integração apropriada do consumidor.

## Invariantes

1. Banco incompatível, corrompido, inacessível ou de outra sincronização: **erro**, jamais recriação destrutiva.
2. FK habilitadas, unicidade por conta para `RemoteFileRef`, revisão otimista e restrições de tombstone/histórico verificadas em SQLite real.
3. Não usar JSON/BLOB opaco para serializar `SyncItem` inteiro ou esconder tabelas relacionais; BLOB permitido para bytes locais específicos. `ProviderFileData` só após aprovação de codec e retenção.
4. Operação transacional é atômica e nenhum repositório faz `commit` unilateral.
5. Manter conexões e transações restritas ao banco de cada sincronização, expondo somente resultados tipados.
6. Sem migrações de formatos legados nesta proposta. Esquema e provisionamento **futuros** precisam de validação antes do primeiro banco real.

## Testes

Criação/abertura e identidade; FK/UNIQUE/CHECK; duas sincronizações isoladas; concorrência de escrita, rollback, reabertura após commit, disco cheio/bloqueado/corrompido; ausência de exposição de SQL, credenciais e caminhos privados.

**Pendentes:** DDL exato, índice/consulta, política WAL, busy timeout, provisionamento quando raiz indisponível, codec dos metadados escolhidos e opção para testes de crash em subprocesso.

## Recuperação por mecanismo central SQLite

O Inventory Storage registra sua identidade e localização física autorizada no mecanismo comum de [Configuration](../../configuration/INTERFACES.md), responsável pelos **snapshots consistentes SQLite e restauração física**. As conexões e transações ordinárias, DDL, identidade e validações de domínio continuam privados do Inventory Storage. Os repositórios de Items, Metadata, Baseline, Operations e Activity não precisam conhecer backup ou restauração.

Diante de corrupção **comprovada**, o Storage deve informar erro e sua indisponibilidade, assegurar exclusividade de acesso ao banco e solicitar `restore_database` pelo identificador registrado. A troca física só acontece depois que conexões/transactions relevantes foram encerradas ou isoladas, com preservação do banco anterior e arquivos WAL/journal. Após o resultado físico, o Storage reabre e valida identidade, esquema e integridade do domínio. Apenas então o Inventário sinaliza `Restored`.

A falha de restauração **não** deve criar automaticamente banco vazio. Se a coordenação autorizar explicitamente reconstrução posterior, o Storage utiliza sua rotina de inicialização do schema e sinaliza `Rebuilt`; não fabrica baseline, vínculos, atividades nem operações confirmadas. Scanner, Detector e Analyzer mantêm suas responsabilidades; a coordenação decide como recuperar cobertura e sincronização.

Incompatibilidade de esquema, conta/sync_id divergente, `SQLITE_BUSY`, falha transitória, disco cheio e permissão negada não são corrupção confirmada e não podem iniciar substituição. A recuperação de backup pode representar estado antigo em relação ao Drive/filesystem; nenhuma ação destrutiva é autorizada por simples retorno de `Restored`.

**Pendências:** estabelecer capacidade restrita de fechamento/coordenação de conexões e testes reais SQLite, WAL e falha entre preservação/substituição/reabertura. Não há Rust implementado.
