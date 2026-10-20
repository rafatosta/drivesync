# Algoritmos queue

**Estado:** especificação proposta v0.2, independente de linguagem.

## IDs
QUEUE-001 fila persistente, QUEUE-002 coalescência.

## Pré-condições
Entradas validadas, estado conhecido, pré-condições registradas e recursos disponíveis.

## Algoritmo
1. Persistir jobs e dependências.
2. reunir eventos redundantes preservando causalidade.
3. reservar execução exclusiva por arquivo.
4. registrar transições.
5. esperar backoff.
6. recuperar leases após crash.

## Pós-condições
Resultado confirmado e durável ou erro tipado com etapas parciais recuperáveis.

## Falhas
Rede, autorização, disco, versão concorrente, estado parcial, cancelamento e interrupção entre efeito e commit.

## Verificar
Identidade, versões, integridade e invariantes antes de ações destrutivas, e efeitos confirmados depois.

## Não inferir
Timeout não prova ausência; nome não prova identidade; erro parcial não confirma sucesso; estado UNKNOWN bloqueia exclusão.

## Invariante
Nunca descartar jobs não confirmados.

## Complexidade
Metadados por item aproximadamente O(1); n operações O(n); B bytes transferidos O(B), excluídas latências externas.


## Fundamentação técnica por algoritmo

Separar **técnica consolidada**, **política própria**, **evidência de teste** e **hipótese não demonstrada**. Consulte [método](../../../docs/development-methodology.md), [fontes](../../../docs/reference-catalog.md) e [template](../../../docs/algorithm-spec-template.md).

### QUEUE-001

- **Técnica conhecida:** Journal de operações persistente com retry idempotente.
- **Referências primárias:** [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html), [Drive errors](https://developers.google.com/workspace/drive/api/guides/handle-errors).
- **Política do DriveSync:** Persistir antes de executar; estado RUNNING após crash vira NEEDS_RECONCILIATION, não COMMITTED; limitar retries.
- **Testes/oráculos a detalhar:** Interrupção antes/depois da confirmação, job duplicado, operação remota ambígua.
- **Limitações / questões abertas:** Definir chaves de idempotência viáveis para cada operação remota.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.

### QUEUE-002

- **Técnica conhecida:** Consolidação de eventos preservando causalidade.
- **Referências primárias:** [GIO FileMonitor](https://docs.gtk.org/gio/class.FileMonitor.html), [SQLite WAL](https://www.sqlite.org/wal.html).
- **Política do DriveSync:** Criar+alterar pode ser agrupado; renomear+excluir requer ordem preservada; nunca condensar com perda de significado ou registros duráveis.
- **Testes/oráculos a detalhar:** Salvar temporário/rename, dez eventos seguidos, delete/recreate mesmo nome.
- **Limitações / questões abertas:** Criar tabela de reescritas de eventos e exemplos formais; não supor comutatividade geral.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.
