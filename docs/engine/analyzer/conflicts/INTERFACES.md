# Conflitos — interfaces implementadas

## Escopo: somente identificação

`engine/src/analyzer/conflicts/` é privado ao Analyzer e independente do
Reconciler. A função síncrona `evaluate(ConflictRequest) -> Result<ConflictAssessment,
ConflictError>` identifica conflitos de **um item por chamada**, deterministicamente,
sem I/O ou estado mutável externo.

Esta versão não oferece `Automatic`, `RequiresDecision`, opções de resolução,
`OperationCommand`, escolhas de versão ou ações. Ações e interação serão
implementadas posteriormente pelo Analyzer.

## Entrada e propriedade

`ConflictRequest` contém `item: inventory::SyncItem` e `evidence: ConflictEvidence`.
O item inclui identidade, revisão, presenças, snapshots e baseline. Não duplicar
`SyncItem`, `LocalFile`, `RemoteFile`, `SyncBaseline` ou seus identificadores.

| Evidência | Tipo | Significado |
| --- | --- | --- |
| `baseline_confirmed` | `bool` | Permite usar o baseline fornecido como histórico confirmado |
| `local_scope_complete`, `remote_scope_complete` | `bool` | Cobertura suficiente para concluir sobre a presença de cada lado |
| `content` | `ContentEvidence` | Comparações explícitas de conteúdo atual contra baseline e entre lados |
| `local_parent` | `Option<SyncItemId>` | Pai local atual mapeado para identidade lógica do Inventário |
| `current_parents` | `EvidenceComparison` | Equivalência lógica comprovada dos pais atuais |

`ContentEvidence` contém `local_to_baseline`, `remote_to_baseline` e
`current_sides`, todos `EvidenceComparison::{Equal, Different, Unknown}`.
As comparações de conteúdo pertencem ao contrato de entrada do Conflicts; o
algoritmo não chama Reconciliação nem usa sua avaliação como verdade.

O chamador interno deve fornecer comparações confiáveis e vinculadas à revisão
analisada, com o mesmo algoritmo/domínio ao comparar hashes. O avaliador não
coleta nem autentica evidências. Não usa tamanho, data, inode, nome ou versão
remota como prova de conteúdo. Comparações contra baseline ausente ou não
confirmado não são usadas para inferir histórico. Diretórios não comparam conteúdo.

Snapshots `*_latest` prevalecem quando disponíveis; campos desconhecidos neles
não são preenchidos com os valores retidos. Snapshots retidos de lados ausentes
são históricos e não são usados como prova de presença atual.

## Retorno: classificação e motivos

`ConflictAssessment` contém:

- `item_id`, `item_revision`: identidade e revisão exatas da entrada;
- `presence: ConflictPresence`: `NoConflict`, `Conflict` ou `Undetermined`;
- `findings: Vec<ConflictFinding>`: dimensões avaliadas, status e motivo;
- `requirements: Vec<EvidenceRequirement>`: evidências faltantes, sem duplicatas.

`requires_user_intervention()` retorna verdadeiro somente para `Conflict`.
`Undetermined` exige obter evidências antes de concluir; não é convertido em
uma escolha do usuário. `NoConflict` identifica ausência de incompatibilidade
nas dimensões avaliadas; não afirma que o item está sincronizado nem autoriza
execução, exclusão ou confirmação do baseline.

Cada `ConflictFinding` contém `dimension`, `status` e `reason`:

| Campo | Valores |
| --- | --- |
| `ConflictDimension` | `Presence`, `Content`, `Name`, `Location` |
| `FindingStatus` | `Compatible`, `Divergent`, `Unknown` |
| `FindingReason` | `Equivalent`, `UnilateralChange`, `ConcurrentChanges`, `DeleteVsChange`, `NewItem`, `MissingBaseline`, `InsufficientEvidence`, `UnsupportedKind` |

A ordem é presença, conteúdo (arquivos), nome e localização. Na avaliação de
exclusão versus alteração, um finding de presença agrega as comparações do lado
sobrevivente. Requisitos específicos preservam lacunas de conteúdo, nome ou pai.

`EvidenceRequirement` distingue `ObserveLocal`, `ObserveRemote`, `VerifyBaseline`,
`VerifyContent`, `VerifyParent`, `VerifyName` e `VerifyKind`. São descrições de
lacunas, sem agendar ou executar observações.

## Regras determinísticas

1. Presença desconhecida, cobertura incompleta, tipo desconhecido ou estado remoto
   em lixeira/indeterminado produzem `Undetermined` com requisito específico.
2. Ambos ausentes comprovadamente produzem `NoConflict`. Um lado presente sem
   baseline representa item novo, sem conflito comprovado. Baseline existente mas
   não confirmado não permite distinguir criação de exclusão histórica.
3. Para dois lados presentes, comparar conteúdo, nome e localização de forma
   independente. Igualdade atual comprovada é compatível mesmo quando ambos mudaram.
4. Mudança unilateral em relação a baseline confirmado é compatível. Renomeação
   local e edição remota, por exemplo, não exigem decisão do usuário por si só.
5. Mudanças concorrentes da mesma dimensão, com resultados atuais comprovadamente
   distintos, produzem `Conflict/ConcurrentChanges` e exigem intervenção do usuário.
6. Exclusão confirmada contra mudança comprovada de conteúdo, nome ou pai produz
   `Conflict/DeleteVsChange`. Contra lado inalterado produz `NoConflict`; isso não
   fornece política ou autorização de exclusão. Lado sobrevivente desconhecido
   produz `Undetermined` quando não existe mudança comprovada.
7. Nomes locais preservam bytes Unix e são comparados com seu próprio baseline;
   nomes remotos usam valores conhecidos. Nomes iguais não provam identidade.
8. Pai local usa identidade lógica; pai remoto usa referência qualificada pela
   conta. Zero/múltiplos pais remotos ficam desconhecidos. A equivalência entre
   os lados depende de mapeamento explícito, não de nome/caminho semelhante.
9. Ausência de baseline não prova concorrência: diferenças sem histórico confirmado
   produzem `Undetermined/VerifyBaseline`. Estados atuais comprovadamente
   equivalentes podem ser classificados sem inventar histórico.
10. Um finding divergente determina `Conflict` mesmo havendo dimensões desconhecidas;
    os requisitos continuam presentes. Sem divergência, qualquer finding desconhecido
    determina `Undetermined`; todos compatíveis determinam `NoConflict`.

## Validação e erros

`ConflictError` contém apenas `InvalidRequest`, `InconsistentInput`,
`WrongItemIdentity` e `InvalidEvidence`, sem caminhos, tokens ou payloads.
Valida IDs não vazios, revisão positiva, coerência dos modelos, tipos, vínculos
local/remoto, conta do pai remoto e provedor entre snapshot retido e recente.
`Present` sem snapshot é erro; `Unknown` com snapshot ausente é incerteza legítima.
`baseline_confirmed=true` exige baseline. Comparação cruzada de conteúdo exige
arquivo presente nos dois lados. Ambos inalterados contra baseline confirmado
mas conteúdos atuais distintos constituem evidência contraditória.

## Limites de integração

O módulo não conhece Inventário concreto, Operador, Providers, Orquestrador, GUI,
filas ou pendências persistidas. O Analyzer poderá consumir a classificação para
criar pendências posteriormente; a fachada atual já persiste a classificação no Inventory, sem criar opções ou ações.

Metadados específicos (permissões, proprietário, capacidades e atributos do
provedor) não têm política/baseline portátil suficiente e não são classificados
nesta versão. Presenças e comparações são evidências declaradas pelo coordenador;
a função não comprova sua autenticidade nem exclusividade entre instâncias.
