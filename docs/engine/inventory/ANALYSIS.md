# Persistência de análises

O Inventory registra fatos de avaliação pela API pública, sem conhecer ou chamar
os algoritmos do Analyzer. O módulo `inventory::analysis` define os modelos de
persistência, reexportados pela fachada; o SQL fica em `storage/analysis.rs`.

## Métodos adicionados

- `record_analysis(RecordAnalysisRequest) -> InventoryResult<RecordAnalysisResult>`:
  grava uma avaliação, status diagnóstico do item e intenções de operação vinculadas,
  em uma única transação IMMEDIATE.
- `get_analysis(GetAnalysisRequest) -> InventoryResult<GetAnalysisResult>`:
  consulta por `AnalysisId` ou última análise de um `SyncItemId`.
- `list_analyses(ListAnalysesRequest) -> InventoryResult<ListAnalysesResult>`:
  pagina por item, disposições e filtro `current_only`, com cursor vinculado ao
  filtro/escopo/geração e limite de inserção da primeira página.

As requisições/resultados concretos estão em `engine/src/inventory/analysis.rs`.
`RecordAnalysisRequest` contém `expected_sync_id`, `expected_item` completo,
`evidence`, `assessment` e `recorded_at`. Identidade/revisão iguais com snapshots
adulterados não são aceitos. A conclusão dos algoritmos é fornecida pelo chamador;
o Inventory valida coerência estrutural, não refaz a avaliação.

## Fatos duráveis e fila

`RecordedAnalysis` conserva identidade, tipo, revisão analisada, revisão após a
gravação, fingerprint SHA-256 do item de entrada, evidências declaradas, disposição,
findings ordenados, requisitos e decisões com `OperationId` persistente.

Disposições: `NoChanges`, `PendingOperations`, `PendingEvidence` e
`PendingUserDecision`. Conflitos comprovados prevalecem sobre requisitos de evidência;
nenhuma disposição confirma efeitos ou marca o item como `Synced`.

Cada intenção de criação, conteúdo, nome ou localização gera uma linha `Blocked`
na fila, na mesma ordem das decisões. Direção indica propagação local→remoto
(`Upload`) ou remoto→local (`Download`). `Create` também representa criação de
diretório, distinguida pelo tipo do item registrado na análise. O erro de bloqueio
identifica conflito, evidência pendente ou comando indisponível.

As operações vinculadas não contêm comando executável: `claim_operation` e
`update_operation` rejeitam sua liberação, inclusive `Requeue`. A implementação
futura de comandos terá de definir sua materialização e autorização; nunca usar a
fila sem parâmetros completos para iniciar transferências. Operações antigas em
`Running` impedem uma nova gravação de análise daquele item.

## Revisões, retries e obsolescência

Uma gravação nova avança a revisão do item uma vez, atualiza seu diagnóstico e usa
a revisão resultante como revisão-base das operações. O baseline e o histórico de
atividades de execução permanecem intactos. Falha em qualquer inserção reverte
recibo, status, revisão, findings, requisitos e fila.

A unicidade `(item, revisão analisada)` permite retry exato: o digest compara
item/evidência/avaliação e ignora a nova data de gravação. Retorna o recibo original,
sem duplicação. Conteúdo diferente para a mesma revisão falha com `VersionMismatch`.
Reavaliar o item na revisão resultante, com a mesma evidência/avaliação, também
reutiliza o recibo atual. Novas evidências ou observações podem gerar outro registro.

`stale` é calculado comparando a revisão atual com a revisão resultante do registro.
Registros antigos são preservados para consulta; `current_only` os exclui.
Repetir um recibo antigo devolve `stale=true`, sem torná-lo atual ou reiniciar trabalho.
Não existe exclusividade transacional entre SQLite e efeitos físicos/remotos.

## Esquema e limites

Esquema SQLite **5**, com tabelas relacionais `analyses`, `analysis_findings`,
`analysis_requirements` e `analysis_decisions`, ligadas a `items`/`operations` por
chaves estrangeiras, incluindo associação item/operação. Não há blob genérico de
avaliação ou cópia serializada integral de `SyncItem`; o snapshot integral só é
usado para validar o estado e calcular o fingerprint. Evidências e códigos são
armazenados em colunas próprias. As tabelas constam da inspeção somente leitura.

Sem retrocompatibilidade nesta fase: bancos com outro esquema são preservados e
retornam `UnsupportedSchema`, sem migração, exclusão ou recriação automática.
Nenhum banco real do usuário é aberto pela suíte de validação.

Opções de resolução, ações do usuário, comandos completos e recuperação de efeitos
incertos continuam fora deste contrato. Um registro persistente de conflito não é
um `PendingAction` público com opções de resolução implementadas.

Vínculos explícitos e planos: [PLANNING.md](PLANNING.md).
