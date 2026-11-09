# Reconciliador — INTERFACES

## 1. Contrato privado

O Reconciliador recebe **uma requisição para um `SyncItem`** e retorna uma avaliação contendo a lista de operações propostas, as divergências detectadas e as evidências que faltam. O módulo não exige o resultado de Conflitos para executar sua função: ambos são algoritmos independentes coordenados pelo Analisador.

```rust
pub(crate) type ReconciliationResult<T> = Result<T, ReconciliationError>;

pub(crate) fn reconcile(
    request: ReconciliationRequest,
) -> ReconciliationResult<ReconciliationAssessment>;
```

A assinatura é **conceitual para TDD**, síncrona e sem I/O. A mesma entrada deve produzir a mesma saída. A implementação poderá organizar funções auxiliares internamente sem expô-las como APIs adicionais.

## 2. Tipos importados e propriedade

Reutilizar, sem duplicar:

- [Inventário](../../inventory/INTERFACES.md): `SyncItem`, `SyncItemId`, `SyncBaseline`, `PresenceState`, `ItemKind`, versões e revisão.
- [Models](../../models/INTERFACES.md): `LocalFile`, `RemoteFile` e suas referências e valores de metadados.
- [Analisador](../INTERFACES.md): contrato de `OperationCommand`, que reutiliza as dez requisições tipadas do [Operador](../../operations/INTERFACES.md).

Uma `SyncItemId` identifica o item lógico; `RemoteFileRef` identifica o recurso remoto qualificado pela conta; `SyncItem.revision` serve ao controle otimista de alterações no Inventário. Não confundir essas identidades.

## 3. Requisição

```rust
pub(crate) struct ReconciliationRequest {
    pub item: SyncItem,
    pub evidence: ReconciliationEvidence,
}

pub(crate) struct ReconciliationEvidence {
    pub local_observation: ObservationConfidence,
    pub remote_observation: ObservationConfidence,
    pub content_comparison: ContentComparison,
}

pub(crate) enum ObservationConfidence {
    Complete,
    Partial,
    Unknown,
}

pub(crate) enum ContentComparison {
    VerifiedEqual,
    VerifiedDifferent,
    NotCompared,
    NotComparable,
}
```

### Campos e semântica

| Campo | Obrigatório | Descrição |
| --- | --- | --- |
| `item` | Sim | Objeto `SyncItem` integral, com `id`, `kind`, `revision`, lados local/remoto, presença e baseline |
| `evidence.local_observation` | Sim | Qualidade da observação local para o escopo do item |
| `evidence.remote_observation` | Sim | Qualidade da observação remota para o escopo do item |
| `evidence.content_comparison` | Sim | Comparação de conteúdo entre lados, quando comprovada por evidência independente |

`Complete` significa observação abrangente para a conclusão que se pretende tirar, **não** disponibilidade de todos os metadados do arquivo. `Partial` e `Unknown` não autorizam inferir exclusão. `ContentComparison::VerifiedEqual` exige comparação confiável entre os conteúdos atuais; `VerifiedDifferent` exige prova de diferença. Igualdade de tamanho ou timestamp, isoladamente, não prova igualdade de bytes.

`SyncItem.baseline=None` indica ausência de referência anterior confirmada. `SyncItem.local=None` ou `remote=None` também não é prova de ausência; utilizar `local_presence` e `remote_presence` com evidências adequadas.

A confiança da observação complementa — não substitui — as provas de presença registradas pelo Inventário. Se a entrada se contradiz (por exemplo, `Present` com snapshot ausente), devolver erro tipado. Se está apenas incompleta, devolver requisito de reobservação.

## 4. Resultado

```rust
pub(crate) struct ReconciliationAssessment {
    pub item_id: SyncItemId,
    pub item_revision: u64,
    pub operations: Vec<PlannedOperation>,
    pub divergences: Vec<ReconciliationDivergence>,
    pub requirements: Vec<ReconciliationRequirement>,
}

pub(crate) struct PlannedOperation {
    pub id: PlanStepId,
    pub command: OperationCommand,
    pub depends_on: Vec<PlanStepId>,
    pub safety: ExecutionReadiness,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PlanStepId(pub String);

pub(crate) enum ExecutionReadiness {
    Ready,
    NeedsConflictResolution,
    NeedsReobservation,
    NeedsDependency,
}

pub(crate) struct ReconciliationDivergence {
    pub dimension: ReconciliationDimension,
    pub reason: DivergenceReason,
    pub explanation: String,
}

pub(crate) enum ReconciliationDimension {
    Presence,
    Content,
    Name,
    Location,
    Metadata,
}

pub(crate) enum DivergenceReason {
    ConcurrentChanges,
    DeleteVsChange,
    IdentityAmbiguous,
    MissingBaseline,
    IncompatibleStates,
}

pub(crate) struct ReconciliationRequirement {
    pub kind: RequirementKind,
    pub explanation: String,
}

pub(crate) enum RequirementKind {
    ObserveLocal,
    ObserveRemote,
    VerifyContent,
    VerifyIdentity,
    VerifyBaseline,
    VerifyParent,
}
```

### Sentido dos campos

- **`item_id` e `item_revision`:** reproduzem a identidade lógica e a revisão efetivamente analisadas, para permitir ao Analisador validar se o plano continua aplicável.
- **`operations`:** comandos concretos propostos em ordem determinística. Lista vazia significa **nenhuma operação segura identificada agora**, e não necessariamente sincronização concluída.
- **`PlannedOperation.id`:** identificador estável **dentro do plano** para expressar dependências; não é `OperationId` do Inventário nem identidade de arquivo.
- **`command`:** requisição tipada conforme a API do Operador. Só incluir quando origem, destino, identidade e precondições puderem ser preenchidos sem suposições.
- **`depends_on`:** etapas anteriores do mesmo plano cuja conclusão é exigida. Referências a itens externos são expressas como requisitos ao Analisador; não criar comandos fictícios.
- **`safety`:** informa se há prova suficiente para a execução (`Ready`) ou se o comando depende de conflito, reobservação ou outra etapa. Mesmo `Ready` exige revalidação pelo Analisador antes da execução.
- **`divergences`:** fatos que exigem avaliação posterior, normalmente por Conflitos. O Reconciliador **não resolve** essas divergências nem oferece opções ao usuário.
- **`requirements`:** dados/observações necessários para completar uma decisão sem presumir ausência ou igualdade.

A representação de `OperationCommand` já está prevista no contrato do Analisador e reutiliza as requisições do Operador; o tipo Rust definitivo deve ser compartilhado, sem circularidade entre módulos.

### Coerência do plano

1. IDs de etapas são únicos e `depends_on` não contém ciclos.
2. Dependências internas apontam para etapas existentes e anteriores na ordem retornada.
3. Comando `Ready` não pode depender implicitamente de divergência pendente.
4. `NeedsConflictResolution`, `NeedsReobservation` e `NeedsDependency` precisam de justificativa correspondente em `divergences` ou `requirements`.
5. Requisito pendente **não** se converte automaticamente em comando de exclusão, criação ou sobrescrita.
6. Se uma decisão requer novos dados para construir uma requisição completa do Operador, retornar apenas a divergência/requisito, sem inventar `OperationCommand`.

## 5. Erros internos

```rust
pub(crate) struct ReconciliationError {
    pub kind: ReconciliationErrorKind,
    pub message: String,
}

pub(crate) enum ReconciliationErrorKind {
    InvalidRequest,
    InconsistentInput,
    InvalidItemIdentity,
    InvalidEvidence,
    InvalidPlan,
    Internal,
}
```

| Erro | Significado |
| --- | --- |
| `InvalidRequest` | Requisição malformada ou campo obrigatório inválido |
| `InconsistentInput` | Contradição entre presença, snapshot e baseline declarado |
| `InvalidItemIdentity` | Identidades local/remota ou vínculo lógico incompatíveis |
| `InvalidEvidence` | Comparação marcada como verificada sem fundamento ou dados incompatíveis |
| `InvalidPlan` | A construção gerou comandos/dependências inconsistentes |
| `Internal` | Falha interna que impede devolver avaliação coerente |

Mudança concorrente comprovada, falta de baseline e metadados desconhecidos **não** são erros técnicos por si só: devem aparecer em `divergences` e `requirements`.

## 6. Contrato funcional de `reconcile`

**Assinatura:** `pub(crate) fn reconcile(request: ReconciliationRequest) -> ReconciliationResult<ReconciliationAssessment>`.

**Finalidade:** comparar três estados disponíveis de um item e propor uma lista de operações necessárias e seguras, identificando separadamente situações que requerem avaliação ou observação.

**Parâmetros:**
- `request.item`: `SyncItem` completo, um único item lógico e sua revisão; o baseline pode estar ausente e as presenças podem ser desconhecidas.
- `request.evidence`: estado de cobertura das observações e comparação confiável dos conteúdos atuais.

**Retorno:** `ReconciliationAssessment`, com identidade analisada, `operations` em ordem, `divergences` e `requirements`. Podem coexistir comandos seguros e pendências. Um plano vazio só é interpretado como `Unchanged` pelo Analisador se os estados forem comprovadamente consistentes, não apenas porque não foram identificados comandos.

**Erros:** inconsistências da entrada produzem `ReconciliationError`. Evidência incompleta produz avaliação com requisitos, sem erro e sem comando destrutivo.

**Efeitos colaterais:** nenhum. Não executa operações, não acessa Inventário/Operador/Providers, não atualiza baseline nem armazena filas.

## 7. Regras mínimas de transformação

| Situação confirmada | Proposta do Reconciliador |
| --- | --- |
| Local, remoto e baseline consistentes | `operations=[]`, sem divergências/requisitos relevantes |
| Somente conteúdo local mudou e remoto permaneceu no baseline | `UpdateContent` remoto, preservando ID |
| Somente conteúdo remoto mudou e local permaneceu no baseline | `UpdateContent` local, preservando destino |
| Novo arquivo local; ausência remota comprovada; identidade não ambígua | `Upload` para criar recurso remoto |
| Novo arquivo remoto; ausência local comprovada; identidade não ambígua | `Download` para criar recurso local |
| Novo diretório local com destino remoto disponível | `CreateDirectory` remoto |
| Novo diretório remoto com destino local disponível | `CreateDirectory` local |
| Nome local alterado, remoto no baseline | `Rename` remoto por ID |
| Nome remoto alterado, local no baseline | `Rename` local |
| Pai local alterado, remoto no baseline | `Move` remoto por ID |
| Pai remoto alterado, local no baseline | `Move` local |
| Exclusão confirmada de um lado, outro sem alterações | Comando de exclusão somente com identidade e política previamente autorizada; caso contrário requisito/pendência |
| Alterações simultâneas comprovadamente compatíveis | Lista ordenada de comandos que preserva ambas as mudanças |
| Ambos alteraram conteúdo de maneira incompatível | Registrar `ConcurrentChanges`; não escolher versão automaticamente |
| Exclusão confirmada versus edição do outro lado | Registrar `DeleteVsChange`; não propor destruição automática |
| Ausência sem prova, baseline não confiável, identidade ambígua | Registrar requisitos de reobservação e/ou divergências |
| Baseline ausente com conteúdo pré-existente em ambos os lados | Não pressupor equivalência: pedir evidências/avaliação de identidade; não sobrescrever |

Esta matriz define **casos de entrada comprovada**; o algoritmo deve verificar as precondições reais antes de emitir o comando, incluindo conta, IDs, pasta pai, nome e capacidades.

### Operações múltiplas para um item

Quando o mesmo item possui modificações independentes, como nome alterado localmente e conteúdo alterado remotamente, o Reconciliador pode devolver duas transformações: renomear remoto e atualizar conteúdo local, com dependências explícitas se houver. Se uma operação depender de outro item (por exemplo, pasta pai ainda não criada), o retorno registra `VerifyParent`/`NeedsDependency`; não cria um lote de outros `SyncItem`.

## 8. Integração

O Analisador transforma o resultado em execução, avaliação de conflitos ou pendência de reobservação. Somente o Analisador pode chamar o Operador e confirmar efeitos pelo Inventário.

O algoritmo de [Conflitos](../conflicts/INTERFACES.md) é independente: recebe uma divergência contextualizada e retorna avaliação própria. O Reconciliador não chama `evaluate` nem exige o resultado de Conflitos como parâmetro.

**Pontos para revisão posterior:** o `OperationCommand` do Analisador ainda é uma proposta conceitual; `SyncBaseline.parent_identity` é provisório; a API atual de fila do Inventário não armazena todos os argumentos necessários a cada comando. A implementação precisa resolver essas integrações sem inventar equivalências ou alterar outros módulos silenciosamente.
