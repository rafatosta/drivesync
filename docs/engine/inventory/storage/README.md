# Inventory — Storage

## Objetivo

Disponibilizar a persistência SQLite do inventário, assegurando acesso consistente, integridade e transações atômicas.

## Funcionamento

Abre e valida o banco `inventory.sqlite3`, gerencia seu esquema, índices e integridade referencial, e fornece contextos de leitura e transação. As operações transacionais são confirmadas ou revertidas atomicamente e as falhas são representadas por erros tipados.

## Interface

- `open`: abre e valida o inventário.
- `read`: executa consultas no contexto de leitura.
- `transaction`: executa operações em transação atômica.

**Entradas principais:** `OpenInventoryRequest`, `SyncIdentity` e `InventoryTx`.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)

## Backup e restauração

O Storage continua responsável por identidade, esquema, transações e validação do Inventário. Ele registra localização autorizada e identidade do banco em [Configuration](../../configuration/INTERFACES.md), que implementa **uma única rotina comum de backup consistente e restauração SQLite**. O Storage não duplica o algoritmo físico de cópia.

Corrupção comprovada exige sinalizar indisponibilidade, garantir ausência de conexões/transações ativas antes da troca e solicitar restauração ao mecanismo comum. Depois, o Storage reabre e valida identidade e esquema antes de informar `Restored`. Se o backup não puder ser restaurado, manter `Unavailable` e preservar os dados anteriores. Reconstrução vazia não faz parte de `restore_database`: requer autorização da coordenação e inicialização do esquema pelo Storage. Falhas de acesso transitórias não iniciam restauração.

## Disponibilidade operacional

O Storage mantém internamente condição de disponibilidade e resultado de recuperação para expor ao `InventoryManager.status()`. Não disponibiliza SQL, conexões ou estados de transação à API pública. Cada operação continua delimitada e atômica; a consulta de status utiliza snapshot interno, não `PRAGMA` nem leitura concorrente do banco. A abertura bem-sucedida indica `Normal`, `Restored` ou `Rebuilt`; falha anterior à criação da instância é erro tipado.
