# Reconciliador — interfaces implementadas

## Fronteira e entrada

O núcleo está em `engine/src/analyzer/reconciliation/`, privado ao Analyzer.
`reconcile(ReconciliationRequest) -> Result<ReconciliationAssessment, ReconciliationError>`
é síncrono, determinístico e sem I/O. Não é API pública do engine. Recebe exatamente
um `inventory::SyncItem`; reutiliza os modelos reais sem duplicá-los.

`ReconciliationRequest` contém `item` e `evidence: ReconciliationEvidence`:

| Campo de evidência | Tipo | Semântica |
| --- | --- | --- |
| `local_observation`, `remote_observation` | `ObservationConfidence` | `Complete`, `Partial`, `Unknown`; cobertura para as conclusões pretendidas |
| `local_content`, `remote_content` | `Change` | `Unchanged`, `Changed`, `Unknown`, em relação ao respectivo lado do baseline confirmado |
| `content_comparison` | `Comparison` | `Equal`, `Different`, `Unknown`, entre os conteúdos atuais |
| `local_parent` | `Option<SyncItemId>` | Mapeamento confirmado do pai local atual para identidade lógica do Inventário |
| `parent_comparison` | `Comparison` | Equivalência lógica entre os pais atuais local/remoto |

O coordenador deve fornecer comparações vinculadas à revisão analisada e às
observações atuais. São evidências declaradas pelo chamador interno; o algoritmo
não faz medições nem verifica a autenticidade dessas declarações. O modelo local
não possui hash de conteúdo e caminho não identifica o item lógico do pai.
Tamanho, timestamp, inode e versão remota não são usados como prova de conteúdo.
Não interpretar `version` remoto, que pode mudar por metadados, como hash.

Na ausência de baseline, as duas comparações de conteúdo contra ele devem ser
`Unknown`. Comparação cruzada de conteúdo só se aplica a arquivos presentes nos
dois lados. `Different` com ambos declarados inalterados é evidência contraditória.

## Resultado e propriedade

`ReconciliationAssessment` preserva `item_id` e `item_revision` e contém vetores
ordenados de `decisions`, `divergences` e `requirements`. Os tipos de avaliação
pertencem exclusivamente ao Reconciler. Nenhum inclui handle de Inventário,
Operador, Provider, banco ou filesystem.

As decisões semânticas implementadas são:

- `Create { source: Side, kind: ItemKind }`: criar o lado ausente a partir do presente;
  exige `ResolveDestination` e não representa comando executável.
- `Propagate { dimension: Dimension, source: Side }`: propagar mudança unilateral
  de conteúdo, nome ou localização, preservando a identidade do outro recurso.

`Side` distingue `Local` e `Remote`. `Dimension` contém `Presence`, `Content`,
`Name` e `Location`; presença é usada nas divergências. A ordem de comparação é
presença, conteúdo, nome e localização. Decisões independentes podem coexistir com
divergências, sem autorizar execução enquanto dependências não forem verificadas.

`Divergence { dimension, reason }` distingue `MissingBaseline`,
`IncompatibleStates`, `ConcurrentChanges`, `DeleteVsChange` e
`InconsistentEvidence`. Nenhum desses resultados escolhe uma versão pelo usuário.

`Requirement` contém `Observe(Side)`, `Compare(Dimension)`, `VerifyIdentity`,
`VerifyBaseline`, `VerifyContent`, `VerifySurvivingState`, `AuthorizeDeletion`,
`ResolveDestination` e `SupportedKind`. A lista não contém duplicatas e mantém
ordem estável. Lista de decisões vazia com requisitos/divergências não é sucesso.

## Regras de comparação

1. Validar ID lógico, revisão positiva, modelos, vínculos de identidade, natureza
   dos recursos e contas dos pais remotos. IDs remotos iguais em contas diferentes
   não são equivalentes. `Present` sem snapshot é contraditório.
2. Preferir `local_latest`/`remote_latest` quando disponíveis. Não preencher campos
   desconhecidos dessas observações usando valores retidos de `local`/`remote`.
3. Presença desconhecida ou cobertura parcial/indeterminada exige reobservação,
   sem decisões executáveis. Uma primeira observação completa, sem baseline nem
   identidade/snapshot associado ao lado oposto, pode identificar intenção de criação
   local→remoto (`Upload`) ou remoto→local (`Download`), preservando `Unknown` e
   os requisitos `Observe`, `VerifyIdentity` e `ResolveDestination`. Tipo observado desconhecido ou estado de lixeira
   remoto desconhecido/ativo também exige reobservação. Links e tipos especiais não
   são autorizados pela simples existência de `ItemKind`.
4. Ambos ausentes comprovadamente não exigem efeito físico. Só um presente sem
   baseline, ou com baseline de ambos ausentes, produz intenção de criação.
5. Um lado excluído em relação a baseline de ambos presentes e outro alterado gera
   `DeleteVsChange`. Outro lado desconhecido exige evidências; inalterado exige
   `AuthorizeDeletion`. Não há política implícita nem decisão de exclusão.
6. Dois lados presentes sem baseline geram `MissingBaseline`/`VerifyIdentity`,
   sem sobrescrita; conteúdo não comprovadamente igual exige `VerifyContent`.
7. Comparar cada nome com seu próprio baseline: bytes locais exatos e nome remoto
   conhecido. Nomes atuais iguais convergem; nomes alterados de maneira distinta
   geram divergência. Bytes não UTF-8 são preservados, sem conversão destrutiva.
8. Comparar pai local lógico e pai remoto qualificado com seus respectivos
   baselines. Zero/múltiplos pais remotos ou mapeamento local ausente ficam
   desconhecidos. Equivalência entre lados depende de evidência explícita.
9. Conteúdo usa somente comparações fornecidas. Igualdade atual comprovada converge;
   mudança unilateral comprovada propaga; mudanças concorrentes diferentes geram
   divergência; falta de prova gera requisito. Diretórios não comparam conteúdo.

## Erros

`ReconciliationError` é um enum sanitizado: `InvalidRequest`, `InconsistentInput`,
`InvalidItemIdentity`, `InvalidEvidence`. Não contém caminhos, payloads ou segredos.
Falta de evidência produz requisitos; contradição estrutural produz erro.

## Integração ainda pendente

O contrato anterior previa `operations: Vec<PlannedOperation>` com
`OperationCommand`, IDs de etapas, dependências e prontidão. Esses comandos **não
foram implementados**. O módulo `operations` já declara as requisições e o
contrato do Operator, mas não contém executor nem comandos materializados pelo Analyzer.
O núcleo implementado devolve decisões, sem inventar cópias dos tipos de execução.
A construção dos comandos, resolução de destinos/pais, capacidades, precondições
atômicas e dependências entre etapas permanece uma fase posterior no Analyzer.
Nenhuma decisão devolvida equivale a `Ready` ou autoriza execução direta.

Metadados como permissões, propriedade e atributos específicos do provedor não
possuem baseline/política portátil suficiente e não são propagados. Recuperação
persistente e autorização de exclusão também não são implementadas. Somente o
Analyzer poderá coordenar Conflitos, chamar o Operador e confirmar efeitos no
Inventário; este núcleo não depende dessas implementações.


A fachada Analyzer já mapeia estas decisões para fatos do Inventory e operações
persistentes `Blocked`; a avaliação continua sem dependência do Inventory concreto.
Essa integração não materializa comandos executáveis. Ver [Analyzer](../INTERFACES.md).
