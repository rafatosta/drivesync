# Analyzer — interfaces implementadas

## Escopo atual

A fachada pública `drivesync_engine::analyzer::Analyzer` coordena a análise de **um
`SyncItem` por chamada** e registra avaliações, decisões e operações pendentes no
Inventory. A primeira integração é de **análise e persistência**. Não executa
comandos, resolve conflitos, espera escolhas do usuário ou confirma baseline.

Os únicos métodos públicos são `new`, `analyze` e `status`. Reconciliação, Conflitos
e o mapeamento para os modelos duráveis são privados. Nenhum desses algoritmos
acessa SQL, Inventory concreto, filesystem, Provider, Operador ou GUI.

## Construção

```rust
pub fn Analyzer::new(config: AnalyzerConfig) -> AnalyzerResult<Analyzer>;

pub struct AnalyzerConfig {
    pub identity: SyncIdentity,
    pub dependencies: AnalyzerDependencies,
}
pub struct AnalyzerDependencies {
    pub inventory: Arc<InventoryManager>,
}
```

`SyncIdentity`/`InventoryManager` pertencem ao Inventory. `new` verifica
sincronização, conta e raiz remota pela API pública do Inventory já aberto. Não
abre/cria banco nem executa operações físicas. É bloqueante e deve ser chamado
fora do loop GTK. Não exige dependência de execução do `Operator`: seu contrato já está compilado,
mas ainda não há executor nem integração de execução nesta fachada.

## Entrada e processamento individual

```rust
pub async fn Analyzer::analyze(
    &self, request: AnalyzeRequest,
) -> AnalyzerResult<AnalysisOutcome>;

pub struct AnalyzeRequest {
    pub directive: Option<operations::RestoreSelection>,
    pub sync_id: SyncId,
    pub item: SyncItem,
    pub evidence: AnalysisEvidence,
    pub recorded_at: Timestamp,
}
pub struct AnalysisOutcome {
    pub analysis: RecordedAnalysis,
}
```

`SyncId` e `Timestamp` pertencem a Models. `SyncItem`, `AnalysisEvidence` e
`RecordedAnalysis` pertencem ao Inventory; seus tipos não são duplicados ou
reexportados genericamente pela fachada. Os modelos concretos estão em
`engine/src/analyzer/models.rs` e `engine/src/inventory/analysis.rs`.

A requisição usa o snapshot integral obtido do Inventory e evidências vinculadas
à sua revisão. `recorded_at` vem do chamador e não influencia o resultado dos
algoritmos. A API exige runtime Tokio; todo acesso SQLite ocorre em
`spawn_blocking`, preservando o executor assíncrono e o loop de interface.

Fluxo:

1. Validar sincronização/identidade/revisão e reservar atividade transitória local.
2. Carregar o item pela API pública e comparar o snapshot integral; mesmo ID e
   revisão com conteúdo adulterado não são suficientes.
3. Chamar Reconciler com comparações explícitas. Baseline existente não confirmado
   impede geração de decisões históricas e registra `VerifyBaseline`; não é apagado
   para reinterpretar uma exclusão como criação inicial.
4. Chamar Conflicts de forma independente. O módulo apenas identifica conflitos e
   evidências insuficientes, sem opções ou ações de resolução.
5. Mapear avaliações para findings e requisitos duráveis, preservando a origem e
   a ordem. Converter intenções de criação/conteúdo/nome/localização em tipos do
   Inventory, sem inventar comandos do Operador.
6. Gravar pelo `InventoryManager::record_analysis`, que revalida escopo, snapshot
   e revisão na transação que registra a avaliação e sua fila vinculada.

## Evidências e retorno durável

`AnalysisEvidence` conserva confirmação do baseline, cobertura dos dois lados,
comparações de conteúdo contra baseline e entre lados, mapeamento do pai local e
comparação dos pais atuais. `EvidenceRelation::{Equal,Different,Unknown}` descreve
comparações confiáveis fornecidas pelo coordenador. A fachada não coleta hashes
nem autentica essas evidências. Tamanho e timestamps não comprovam conteúdo.

`AnalysisOutcome.analysis` contém um recibo durável com ID, revisão analisada,
revisão resultante, fingerprint de entrada, data, evidências, findings, requisitos,
decisões vinculadas a `OperationId` e obsolescência calculada. As disposições são:

| Disposição | Significado |
| --- | --- |
| `NoChanges` | Nenhuma transformação/requisito identificado nesta análise; não confirma baseline |
| `PendingOperations` | Intenções persistidas aguardam comandos executáveis |
| `PendingEvidence` | Há evidência, autorização ou destino faltante |
| `PendingUserDecision` | Conflicts comprovou incompatibilidade que exige intervenção futura |

Conflito comprovado prevalece sobre requisitos pendentes, sem ocultá-los.
Cada intenção gera uma operação **Blocked**, vinculada à decisão no mesmo commit
SQLite. Isso não é plano executável nem autorização de transferência. As APIs
públicas de claim/requeue não liberam essas intenções incompletas.

O Inventory preserva baseline e atividades de execução. Gravar uma avaliação nova
avança uma vez a revisão do item e atualiza seu diagnóstico; nunca o marca `Synced`.
Leia [Persistência de análises](../inventory/ANALYSIS.md) para atomicidade,
paginação, consulta após reinício e esquema 4 sem migração.

## Retry, concorrência e cancelamento

- Chamadas simultâneas do mesmo item nesta instância retornam `AlreadyRunning`.
  Itens distintos podem progredir independentemente; não existe fila privada.
- Entre instâncias, a transação do Inventory usa snapshot/revisão e recibo único
  por item/revisão; não há efeitos físicos a proteger nesta integração.
- Retry idêntico reutiliza o recibo sem duplicar decisões/fila. Se o snapshot
  solicitado já não é atual, a fachada só recupera a última análise quando revisão,
  fingerprint integral e evidências correspondem ao pedido original; caso contrário
  retorna `StaleItem`. O recibo pode estar `stale=true` após observação posterior.
- Reanalisar o item na revisão resultante com a mesma evidência/avaliação também
  reutiliza o recibo. Novas observações/evidências podem exigir outro registro.
- Cancelar o future depois do despacho não cancela `spawn_blocking`: a gravação
  pode terminar. A reserva permanece com o worker até seu término; retry/consulta
  recupera o registro. Não prometer interrupção instantânea ou rollback por cancelamento.
- Falha técnica sem gravação retorna erro; a transação impede avaliação ou fila
  parcialmente registradas. Incerteza sobre recebimento da resposta exige consulta,
  sem repetição de efeitos físicos, que não são executados nesta fase.

## Status transitório

```rust
pub fn Analyzer::status(&self, request: AnalysisStatusRequest)
    -> AnalyzerResult<AnalysisStatusResult>;
```

`AnalysisStatusRequest` contém `item_id`; o retorno contém `active: Option<ActiveAnalysis>`.
`ActiveAnalysis` contém item, revisão e fase real: `Validating`, `Reconciling`,
`EvaluatingConflict` ou `Recording`. Não há fase de execução fictícia, bytes ou
porcentagem inventados. `status` consulta somente memória local, rapidamente e sem
I/O. ID sintaticamente válido inativo retorna `None`, sem provar escopo ou estado
sincronizado. A atividade termina com o worker, inclusive em erro ou cancelamento
do chamador. Não criar `pause`, `resume`, `stop` ou histórico privado.

## Erros sanitizados

`AnalyzerError { kind, inventory_error: Option<InventoryError> }` preserva somente
categorias tipadas, sem SQL, caminhos, credenciais ou payloads.

`AnalyzerErrorKind`: `InvalidConfiguration`, `InvalidRequest`, `WrongSyncScope`,
`AlreadyRunning`, `StaleItem`, `InvalidEvidence`, `InventoryFailure`,
`RuntimeUnavailable`, `Internal`. Revisão divergente no Inventory vira `StaleItem`;
escopo divergente vira `WrongSyncScope`. Outros erros de persistência conservam a
categoria do Inventory no campo opcional.

## Funcionalidades adiadas

Execução pelo Operador, renovação de comandos após dependências satisfeitas,
`Completed/Pending/Failed` relativos a efeitos físicos, `PendingAction` com opções,
validação de escolhas, resolução e recuperação de efeitos indeterminados não estão
implementados. A API atual devolve **recibo de análise**, não relatório de execução.
Os contratos dessas funcionalidades deverão ser definidos quando implementados,
sem expor os algoritmos privados nem permitir SQL fora do Inventory.

O recibo agora contém plano opcional: [PLANNING.md](PLANNING.md). O fluxo do Analyzer sempre o produz; persistência direta de avaliações isoladas pode usar `None`.
