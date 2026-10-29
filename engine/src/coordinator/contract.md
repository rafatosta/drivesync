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

## Disponibilidade — SYNC-004

O contrato vigente de disponibilidade está em
[Proteção de disponibilidade de raízes](../../../docs/SOFTWARE_SPECIFICATION.md#proteção-de-disponibilidade-de-raízes--sync-004).
`coordinator::open/verify` não criam marcador nem inferem exclusões. A fachada
retém um único descritor/bloqueio local da preparação ao fim de downloads e uploads.
Motivos de suspensão são persistidos fora da raiz em `state::Roots`, separados da
base confirmada. Falha de persistência bloqueia o ciclo; nenhum banco inválido é
reinicializado automaticamente.

Consulta de disponibilidade não constitui transação com a API Google. Revalidar
antes dos efeitos reduz janelas de corrida, mas não impede revogação imediatamente
após a consulta. Um efeito já concluído é reconciliado pelo journal existente;
não prometer rollback remoto nem ausência absoluta de efeitos durante revogação.
404 permanece inconclusivo entre ausência e falta de acesso, conforme
[erros do Drive](https://developers.google.com/workspace/drive/api/guides/handle-errors).
Descritores locais mantêm identidade durante renomeações, mas não são sinal
universal de montagem: reabrir o caminho e comparar sua identidade durante o ciclo,
conforme [open/openat](https://man7.org/linux/man-pages/man2/open.2.html).
