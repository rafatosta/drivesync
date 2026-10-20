# Contrato coordinator

## Papel
coordinator: responsabilidade isolada do motor; veja README do módulo.

## Consumidores e dependências
Interage exclusivamente pelos contratos em docs/contracts.md. Não conhece internos de outros módulos.

## Entradas e saídas
Entradas tipadas e validadas com IDs estáveis, versões esperadas e contexto de operação; saída tipada ou erro claro (transitório, definitivo, conflito, ambíguo).

## Limites e garantias
Falha de estado bloqueia operações destrutivas. Não assumir funções de outros módulos; sem dependência de Rust ou GTK na especificação.

## Concorrência
Operações concorrentes sobre mesma identidade são serializadas ou recusadas; idempotência e cancelamento precisam ser explícitos.

## Scheduler de múltiplas contas

Um único Coordinator/Scheduler gerencia `AccountSession` independentes. Cada operação contém `AccountId + SyncRootId`; garantir exatamente um fluxo ativo por raiz e rejeitar roots locais sobrepostas. Pausa/retry/falha de autorização são isoladas à conta, exceto falha de recurso **global** (ex.: banco SQLite íntegro não disponível), que deve resultar em degradação explícita.

Recursos globais são limitados (rede, memória, I/O), com progresso justo entre contas; não prometer prioridade rígida sem testes. Testes: A pausa enquanto B conclui, A falha OAuth, reinício com operações em ambas, fairness sob carga desigual.
