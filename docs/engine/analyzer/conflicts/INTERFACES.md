# Conflitos — INTERFACES

## 1. Objetivo do contrato

A API **interna ao Analisador** avalia uma situação entre duas representações do mesmo `SyncItem` e devolve uma conclusão autocontida: há conflito ou não, qual a razão, que evidência suporta a conclusão e o que fazer em seguida.

É uma interface de **entrada única e saída única**. Não implementa o fluxo do Analisador nem executa ações. As assinaturas Rust são **propostas para TDD**, a serem refinadas após documentação do Reconciliador.

Reutilizar os tipos proprietários:
- [Inventário](../../inventory/INTERFACES.md): `SyncItemId`, `SyncBaseline`, `PresenceState`.
- [Models](../../models/INTERFACES.md): `LocalFile`, `RemoteFile` e identidades associadas.
- [Analisador](../INTERFACES.md): `OperationCommand`, `ResolutionMeaning` e `StateFingerprint` como contratos de integração; evitar duplicá-los em Rust.

## 2. Interface de acesso

```rust
pub(crate) type ConflictResult<T> = Result<T, ConflictError>;

pub(crate) fn evaluate(
    request: ConflictRequest,
) -> ConflictResult<ConflictAssessment>;
```

A função é conceitualmente síncrona, determinística e livre de I/O. Recebe todos os dados necessários, sem acessar Inventário ou API remota. Uma mesma entrada deve produzir a mesma avaliação, independentemente da ordem de outras análises.

## 3. Objeto de entrada

```rust
pub(crate) struct ConflictRequest {
    pub item_id: SyncItemId,
    pub local: ConflictSide<LocalFile>,
    pub remote: ConflictSide<RemoteFile>,
    pub baseline: Option<SyncBaseline>,
    pub evidence: ConflictEvidence,
}

pub(crate) struct ConflictSide<T> {
    pub presence: PresenceState,
    pub snapshot: Option<T>,
}

pub(crate) struct ConflictEvidence {
    pub baseline_confirmed: bool,
    pub local_scope_complete: bool,
    pub remote_scope_complete: bool,
    pub local_content_identity: EvidenceValue<String>,
    pub remote_content_identity: EvidenceValue<String>,
}

pub(crate) enum EvidenceValue<T> {
    Verified(T),
    NotCollected,
    Unsupported,
    Unavailable,
}
```

### Atributos e semântica

| Campo | Obrigatório | Significado |
| --- | --- | --- |
| `item_id` | Sim | Identidade lógica única a que os dois lados se referem |
| `local.presence` | Sim | Presença conhecida, ausência confirmada ou desconhecimento local |
| `local.snapshot` | Não | Metadados locais efetivamente observados; `None` não indica exclusão |
| `remote.presence` | Sim | Presença conhecida, ausência confirmada ou desconhecimento remoto |
| `remote.snapshot` | Não | Metadados remotos efetivamente observados; `None` não indica exclusão |
| `baseline` | Não | Último estado sincronizado confirmado; `None` indica ausência de referência histórica |
| `evidence.baseline_confirmed` | Sim | Indica se o baseline fornecido possui confirmação confiável |
| `evidence.local_scope_complete` | Sim | Evidência suficiente da abrangência da observação local para avaliar ausência |
| `evidence.remote_scope_complete` | Sim | Evidência suficiente da abrangência da observação remota para avaliar ausência |
| `evidence.*_content_identity` | Sim | Evidência de identidade do conteúdo (por exemplo, hash verificado), separando desconhecimento de igualdade comprovada |

**Validação:** `Present` exige snapshot do lado correspondente. `AbsentConfirmed` exige evidência suficiente do escopo e não deve conter snapshot atual marcado como presente; snapshots históricos pertencem ao baseline. `Unknown` nunca deve virar ausência por inferência. `baseline_confirmed=true` requer `baseline=Some`; caso contrário a entrada é inválida. O campo `baseline_confirmed` não comprova por si só autenticidade: a origem dos dados deve ser confiável no contrato com o Analisador.

**Identidade do conteúdo:** `Verified` só pode ser usado para evidência verificada e comparável entre os lados (mesmo algoritmo e domínio, quando hash). Tokens não comparáveis não autorizam conclusão de igualdade. O tipo `String` é ilustrativo: pode ser substituído por um tipo com algoritmo, escopo e versão explícitos antes da implementação.

## 4. Objeto de retorno

```rust
pub(crate) struct ConflictAssessment {
    pub item_id: SyncItemId,
    pub presence: ConflictPresence,
    pub decision: ConflictDecision,
    pub findings: Vec<ConflictFinding>,
}

pub(crate) enum ConflictPresence {
    NoConflict,
    Conflict,
    Undetermined,
}

pub(crate) enum ConflictDecision {
    NoAction,
    Automatic { operations: Vec<OperationCommand> },
    RequiresDecision { options: Vec<ConflictOption> },
    Reobserve { requirements: Vec<EvidenceRequirement> },
}

pub(crate) struct ConflictOption {
    pub meaning: ResolutionMeaning,
    pub explanation: String,
}

pub(crate) enum ConflictDimension {
    Presence,
    Content,
    Name,
    Location,
    Metadata,
}

pub(crate) struct ConflictFinding {
    pub dimension: ConflictDimension,
    pub status: FindingStatus,
    pub explanation: String,
}

pub(crate) enum FindingStatus {
    Compatible,
    Divergent,
    Unknown,
}

pub(crate) enum EvidenceRequirement {
    ObserveLocal,
    ObserveRemote,
    VerifyContent,
    VerifyIdentity,
    VerifyBaseline,
}
```

### Semântica da saída

- **`presence`:** é a classificação explícita de conflito: `NoConflict` (não foi detectada incompatibilidade com evidência suficiente), `Conflict` (mudança incompatível comprovada) ou `Undetermined` (impossível concluir com a evidência disponível).
- **`decision`:** indica o encaminhamento sugerido. `NoAction` significa que o avaliador de conflitos não exige providência; **não** afirma que o item esteja sincronizado. `Automatic` contém operações concretas propostas, nunca executadas. `RequiresDecision` apresenta alternativas que o Analisador poderá oferecer externamente. `Reobserve` explica que evidência precisa ser obtida.
- **`findings`:** registra quais dimensões foram comparadas, por que são compatíveis/divergentes/desconhecidas e o suporte utilizado; não pode ser lista vazia quando houver conflito ou evidência insuficiente.
- **`options`:** opções semanticamente compatíveis com `ResolutionMeaning` definido no Analisador, tipicamente `KeepLocal`, `KeepRemote`, `KeepBoth` ou `Defer`. Não oferecer `KeepBoth` sem uma estratégia capaz de preservar efetivamente ambas as versões; não inventar operações de preservação.
- **`operations`:** reutiliza o contrato `OperationCommand` do Analisador/Operador. Cada comando proposto deve conter as precondições pertinentes e respeitar a ordem/dependências; o Analisador revalida as condições antes de executá-lo.

### Coerência entre presença e decisão

| Presença | Decisões permitidas | Observação |
| --- | --- | --- |
| `NoConflict` | `NoAction`, `Automatic` | `Automatic` somente se houver transformação segura derivável |
| `Conflict` | `Automatic`, `RequiresDecision` | A classificação de conflito pode ser resolvida automaticamente por regra comprovada |
| `Undetermined` | `Reobserve` | Não inventar solução ou alternativas destrutivas sem prova |

`RequiresDecision.options` e `Reobserve.requirements` não podem ser vazios. `Automatic.operations` não pode ser vazio; quando não há operações, usar `NoAction`.

## 5. Erros da interface

```rust
pub(crate) struct ConflictError {
    pub kind: ConflictErrorKind,
    pub message: String,
}

pub(crate) enum ConflictErrorKind {
    InvalidRequest,
    InconsistentInput,
    WrongItemIdentity,
    InvalidEvidence,
    Internal,
}
```

**`InvalidRequest`:** campos obrigatórios ausentes ou combinações inválidas. **`InconsistentInput`:** presença, snapshot, baseline ou escopo apresentam contradição. **`WrongItemIdentity`:** as referências informadas não pertencem ao mesmo item lógico ou não podem ser vinculadas com segurança. **`InvalidEvidence`:** evidência declarada como verificada sem requisitos mínimos de confiabilidade/comparabilidade. **`Internal`:** falha interna do algoritmo que impede classificar o caso.

**Não tratar como erro técnico:** conflito real, decisão externa necessária ou evidência insuficiente; essas situações pertencem a `ConflictAssessment`.

Mensagens técnicas devem ser sanitizadas e não carregar tokens, credenciais ou conteúdo dos arquivos.

## 6. Contrato funcional de `evaluate`

**Assinatura:** `pub(crate) fn evaluate(request: ConflictRequest) -> ConflictResult<ConflictAssessment>`.

**Finalidade:** comparar uma divergência de um item entre as representações local e remota, considerando o baseline quando disponível, e devolver a conclusão e o encaminhamento seguro.

**Parâmetros:** `request.item_id` identifica o item; `request.local` e `request.remote` fornecem presença e snapshot por lado; `request.baseline` oferece o estado anterior confirmado; `request.evidence` informa qualidade e completude das observações, incluindo identidade de conteúdo.

**Retorno:** objeto `ConflictAssessment` contendo `item_id`, `presence`, `decision` e `findings`. Em casos sem conflito, `NoAction` significa que esta avaliação não exige ação; a reconciliação pode ainda propor operações legítimas.

**Limites e erros:** entradas inválidas retornam `ConflictError`; dados apenas incompletos resultam em `Undetermined/Reobserve`. O algoritmo nunca realiza I/O, persiste dados, pergunta ao usuário ou executa as operações propostas.

## 7. Regras mínimas de avaliação

| Estados observados | Conclusão mínima |
| --- | --- |
| Local e remoto equivalentes por evidência suficiente | `NoConflict`, inclusive se ambos mudaram de forma equivalente |
| Um lado mudou e outro permanece igual ao baseline confirmado | `NoConflict`; possível transformação pertence ao plano de reconciliação |
| Ambos alteraram conteúdo de maneiras comprovadamente diferentes | `Conflict`; avaliar se existe solução comprovada ou decisão necessária |
| Mudanças simultâneas em dimensões independentes e compatíveis | `NoConflict`, sem descartar nenhuma das mudanças |
| Exclusão confirmada de um lado e alteração do outro | `Conflict`, não propagar exclusão automaticamente |
| Exclusão suspeita ou escopo incompleto | `Undetermined/Reobserve`, sem inferir `AbsentConfirmed` |
| Ambos com ausência confirmada, mesmo item identificado | `NoConflict`, desde que evidências sejam suficientes |
| Baseline ausente e dois conteúdos distintos sem vínculo comprovado | `Undetermined/Reobserve` ou `Conflict` somente com prova suficiente da incompatibilidade e identidade |
| Hash desconhecido, timestamps/tamanho iguais | Não concluir igualdade de conteúdo; `Undetermined` quando essa igualdade for determinante |

**Segurança:** nomes e caminhos não são identidades globais. Comparar recursos remotos com IDs qualificados por conta e identidades locais conforme Models/Inventário. Conclusões condicionadas por capacidades do serviço nunca devem presumir que uma operação está disponível.

## 8. Integração com o Analisador

O Analisador é responsável por adaptar o objeto `SyncItem` para `ConflictRequest`, coordenar a chamada, traduzir `RequiresDecision` em `PendingAction` e converter `Reobserve` em pendência de evidência. Os objetos públicos de pendência, revisão, opções selecionadas e estado de operação permanecem de propriedade do Analisador e do Inventário.

O módulo Conflitos devolve apenas avaliação e recomendações tipadas. Sua assinatura não depende de `Operator`, `InventoryManager` ou `Orchestrator` concretos; não possui estado persistente.

**Ponto de revisão:** o contrato do Analisador ainda está conceitual e não define uma representação Rust definitiva para `OperationCommand` nem persistência durável de `PendingAction`. Essas decisões devem ser reconciliadas na integração, sem criar tipos duplicados.
