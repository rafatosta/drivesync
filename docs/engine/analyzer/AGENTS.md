# Orientações para agentes — Analyzer

## Escopo implementado

A fachada `engine/src/analyzer/` coordena uma análise por `SyncItem` e persiste
recibos e intenções bloqueadas pela API pública do Inventory. Ler [README](README.md),
[INTERFACES](INTERFACES.md), [TESTS](TESTS.md), [persistência do Inventory](../inventory/ANALYSIS.md)
e o [AGENTS da raiz](../../../AGENTS.md) antes de alterar a integração.

- Expor somente `new`, `analyze` e `status`. Sem lote, fila privada, pause/resume/stop.
- Reconciler e Conflicts são privados, puros e independentes. Não chamar um pelo outro.
- Conflicts **apenas identifica**, conforme solicitação do usuário. Não implementar
  ações, escolhas automáticas ou opções de resolução nesta fase.
- Todo SQL permanece no Inventory. Reutilizar seus modelos públicos e os de Models;
  converter explicitamente os resultados dos algoritmos para os fatos duráveis.
- Verificar escopo e snapshot integral, não apenas ID/revisão. A transação revalida
  esses fatos antes de persistir para impedir decisões baseadas em observação antiga.
- Não apagar baseline existente para tratar estado incerto como primeira criação.
- Persistir decisões, conflitos e requisitos sem afirmar execução. Operações
  vinculadas permanecem `Blocked`, sem liberação por claim/requeue genéricos.
- Não confirmar baseline nem fabricar `Synced` a partir do retorno da análise.
- Status lê apenas a atividade local em memória, sem SQL. A reserva pertence ao
  worker mesmo se o future aguardado for cancelado; o worker ainda pode gravar.
- Preservar retry idempotente e tornar visível a obsolescência do registro.
- Não introduzir Operador/Provider/GUI ou comandos fictícios para preencher lacunas.

## Validação

Usar a fachada real e Inventory SQLite temporário para testar persistência,
revisões, retry, reinício, concorrência, rollback e limpeza de status. A suíte
pura de cada algoritmo é complementar. Executar Rust exclusivamente no Flatpak
definido pelo projeto e registrar comandos/resultados reais.

**Estado:** análise/persistência implementadas e chamadas pelo Orchestrator após
Scanner completo e durante o consumo contínuo de observações. Execução e
resolução permanecem pendentes.
