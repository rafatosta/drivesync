# Contrato queue

## Papel
queue: responsabilidade isolada do motor; veja README do módulo.

## Consumidores e dependências
Interage exclusivamente pelos contratos em docs/contracts.md. Não conhece internos de outros módulos.

## Entradas e saídas
Entradas tipadas e validadas com IDs estáveis, versões esperadas e contexto de operação; saída tipada ou erro claro (transitório, definitivo, conflito, ambíguo).

## Limites e garantias
Nunca descartar jobs não confirmados. Não assumir funções de outros módulos; sem dependência de Rust ou GTK na especificação.

## Concorrência
Operações concorrentes sobre mesma identidade são serializadas ou recusadas; idempotência e cancelamento precisam ser explícitos.

## Fila por identidade de conta

Todas as operações duráveis incluem `account_id` e `root_id`, além de ID remoto/contexto de escopo. Evitar duplicatas somente **dentro do escopo da identidade**; não coalescer eventos de contas diferentes. Pausar/repetir/retomar jobs de A não manipula a fila de B. Scheduler compartilha limite global de concorrência, com justiça e limites por conta quando relevantes. Testes de starvation e retries isolados.
