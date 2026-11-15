# Orchestrator — orientações para agentes

## Objetivo

Implementar **apenas a coordenação de sincronização** por meio das interfaces públicas já documentadas. Não reimplementar Scanner, Detector, Inventory, Analyzer, Operator, Providers ou Configuration.

## Leitura obrigatória

1. [README.md](README.md) — fluxos e divisão de responsabilidades.
2. [INTERFACES.md](INTERFACES.md) — portas e contratos internos conceituais.
3. [TESTS.md](TESTS.md) — invariantes, lacunas e casos bloqueados.
4. [Detector](../detectors/INTERFACES.md) e [Scanner](../scanner/INTERFACES.md).
5. [Inventory](../inventory/INTERFACES.md) e [Analyzer](../analyzer/INTERFACES.md).
6. [Accounts](../accounts/INTERFACES.md) e [Configuration](../configuration/INTERFACES.md), quando usados para composição.
7. [AGENTS.md raiz](../../../AGENTS.md) para convenções gerais.

## Regras

- O caminho lógico é `Detector → Inventory → Analyzer`, com `Scanner → Inventory` durante a inicialização; nunca `Detector → Analyzer` por evento bruto.
- `Detector.start(delivery=Paused)` deve iniciar Local e Remoto **antes** do Scanner, para não perder mudanças concorrentes à varredura.
- `pause` suspende **apenas entrega**, não observação. `stop` encerra observação; o Orchestrator assume consequências de gaps, sem delegar reconstrução automática ao Detector.
- Consumir `FileBatch` e usar `Inventory.record_local/record_remote` com evidência adequada; não inventar `AbsentConfirmed` nem vínculo pela igualdade de nomes.
- **Não confirmar o lote** antes que toda sua ingestão esteja duravelmente aceita. Não descartar eventos por falha. Reconhecer que commit entre Detector/Inventory não é atômico e exigir idempotência por `File.id` antes de prometer recuperação pós-crash.
- Obter `SyncItem` atual do Inventory e chamar `Analyzer.analyze` para decisões e execução. Não chamar Operator/Providers nem subalgoritmos internos.
- `Pending` é desfecho da chamada e requer recuperação durável ainda não definida; não criar fila paralela de conflitos em memória como fonte oficial.
- Não interpretar `Completed` como sucesso sem Inventory atualizado; `Failed`/efeito incerto requer reobservação e não replay cego.
- Status agregados combinam snapshots existentes e escopos reais; não inventar métricas, percentuais, estados globais de Analyzer nem conectividade de Accounts.
- Instanciamento de módulos, compartilhamento entre contas, limites de concorrência, workers e runtime ficam deliberadamente **abertos**. Não adicionar defaults arbitrários para esses conceitos.
- `Configuration` é opcional para preferências; não deve se tornar escalonador. Recovery SQLite usa seu mecanismo comum através dos respectivos Storages.
- API externa `facade/` é outro limite: **não** alterar métodos públicos existentes no Rust ou inventar API de UI para completar a documentação interna.
- Testes com SQLite real temporário, fakes de módulo e ambientes Flatpak; identificar casos `BLOCKED`, testes não executados e decisões pendentes sem mascarar lacunas.

## Escopo compilado autorizado

O usuário solicitou integrar Scanner à abertura e expor sua visualização no
Developer Console. A etapa atual compõe Detector pausado, Inventory e Scanner,
com snapshots e cancelamento por escopo. O consumidor terminal diagnóstico foi
removido, sem alias ou caminho de retrocompatibilidade.

Até conectar Analyzer/ingestão, não reservar nem confirmar eventos. Partial
mantém a análise suspensa; ReadyForAnalyzer é só um pré-requisito local e não
autoriza retomar entrega para um consumidor inexistente. A coordenação completa
das seções conceituais permanece pendente.
