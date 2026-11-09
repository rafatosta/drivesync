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

## Backup e reconstrução

O Storage é proprietário da política padrão e da execução física de backup/restauração, sem dependência obrigatória de [Configuration](../../configuration/README.md), que pode fornecer apenas preferências opcionais por demanda. O Storage preserva a base danificada. Corrupção comprovada: **avisar usuário → solicitar restauração automática → caso falhe, avisar novamente e reconstruir banco vazio**. Após recuperação, a inicialização retoma observação local/remota e análise normal, sem inventar baseline ou executar exclusões por falta de histórico. Falhas de acesso temporárias não iniciam reconstrução.

## Disponibilidade operacional

O Storage mantém internamente condição de disponibilidade e resultado de recuperação para expor ao `InventoryManager.status()`. Não disponibiliza SQL, conexões ou estados de transação à API pública. Cada operação continua delimitada e atômica; a consulta de status utiliza snapshot interno, não `PRAGMA` nem leitura concorrente do banco. A abertura bem-sucedida indica `Normal`, `Restored` ou `Rebuilt`; falha anterior à criação da instância é erro tipado.
