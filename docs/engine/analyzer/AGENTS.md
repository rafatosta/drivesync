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
- Nome/caminho semelhante e conteúdo/hash igual nunca estabelecem identidade entre
  itens independentes. Não criar pareamento automático, mesmo por nome+hash.
  Associação de recursos preexistentes exige definição explícita do usuário.
  Vínculos de futuras réplicas vêm da identidade criada/verificada pelo Operator;
  nenhum vínculo confirma baseline ou autoriza sobrescrita automaticamente.
- Consultar [ACTIONS.md](ACTIONS.md) antes de ampliar decisões ou implementar o
  Operator; distinguir ações implementadas, requisitos e políticas ainda propostas.
- Não apagar baseline existente para tratar estado incerto como primeira criação.
- Persistir decisões, conflitos e requisitos sem afirmar execução. Operações
  vinculadas não podem ser liberadas por claim/requeue genéricos. Criações usam
  a reserva específica e a identidade durável descritas em EXECUTION.md.
- Exclusão permanente é proibida. Toda exclusão deve ir para a lixeira;
  ausência de suporte exige bloqueio, nunca fallback para delete definitivo.
- Não confirmar baseline nem fabricar `Synced` a partir do retorno da análise.
- Status lê apenas a atividade local em memória, sem SQL. A reserva pertence ao
  worker mesmo se o future aguardado for cancelado; o worker ainda pode gravar.
- Preservar retry idempotente e tornar visível a obsolescência do registro.
- O usuário autorizou execução completa de novas réplicas. Analyzer recebe Operator
  pela composição e chama suas interfaces; não chama Providers ou filesystem.
  Comandos incompletos exigem bloqueios/dependências explícitos.
- Consulte PLANNING.md para lixeira condicionada e restauração explicitamente solicitada.

## Validação

Usar a fachada real e Inventory SQLite temporário para testar persistência,
revisões, retry, reinício, concorrência, rollback e limpeza de status. A suíte
pura de cada algoritmo é complementar. Executar Rust exclusivamente no Flatpak
definido pelo projeto e registrar comandos/resultados reais.

**Estado:** análise/persistência implementadas e chamadas pelo Orchestrator após
Scanner completo e durante o consumo contínuo de observações. Novas réplicas, atualização de conteúdo, renomeação e movimentação são executadas
e verificadas. Planos com várias dimensões avançam um passo por observação, sem
baseline prematuro. Resolução de conflitos, autorização de lixeira e pedido explícito
de restauração continuam distintos da execução automática de alterações comuns.
