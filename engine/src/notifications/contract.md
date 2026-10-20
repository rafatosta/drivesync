# Contrato notifications

## Papel
notifications: responsabilidade isolada do motor; veja README do módulo.

## Consumidores e dependências
Interage exclusivamente pelos contratos em docs/contracts.md. Não conhece internos de outros módulos.

## Entradas e saídas
Entradas tipadas e validadas com IDs estáveis, versões esperadas e contexto de operação; saída tipada ou erro claro (transitório, definitivo, conflito, ambíguo).

## Limites e garantias
Nenhum token em evento ou log. Não assumir funções de outros módulos; sem dependência de Rust ou GTK na especificação.

## Concorrência
Operações concorrentes sobre mesma identidade são serializadas ou recusadas; idempotência e cancelamento precisam ser explícitos.
