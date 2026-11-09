# Conflitos — TESTS

## Objetivo

Validar o comportamento determinístico e sem efeitos colaterais de `evaluate(ConflictRequest)`, exercitando **um item lógico e dois lados** por chamada. Fonte de verdade: [INTERFACES.md](INTERFACES.md).

Os testes são unitários, usando objetos sintéticos de Models/Inventário, sem rede, filesystem ou SQLite. Não utilizar mocks do algoritmo sob teste. Cada caso compara o retorno esperado construído independentemente.

## Matriz por método

### CF-01 — `evaluate`: ausência de conflito

**Sucesso:** baseline confirmado e estados equivalentes, incluindo duas alterações idênticas comprovadas, retornam `NoConflict` com conclusões justificadas.

**Negativos:** mesmo tamanho/mtime sem prova de conteúdo não é igualdade comprovada; não produzir `NoConflict` quando a informação ausente for decisiva.

**Retorno e invariantes:** `NoAction` não significa sincronização concluída; `findings` registra as dimensões comparadas.

### CF-02 — `evaluate`: alterações unilaterais

**Sucesso:** somente local ou somente remoto difere do baseline; devolver `NoConflict` quando não houver incompatibilidade de mudanças, permitindo ao Reconciliador determinar a transformação.

**Negativos:** baseline não confirmado e versão desconhecida não contam como estado igual.

**Verificação:** nenhuma execução ou I/O; resposta idêntica para entrada idêntica.

### CF-03 — `evaluate`: conteúdo alterado nos dois lados

**Sucesso:** baseline A, local B e remoto C, com conteúdos distintos comprovados; devolver `Conflict` e decisão externa ou solução automática verificável.

**Negativos:** não escolher lado arbitrariamente e não criar `Automatic` sem prova.

**Verificação:** `findings` identifica divergência de conteúdo e `RequiresDecision.options` contém somente alternativas aplicáveis.

### CF-04 — `evaluate`: alterações compatíveis

**Sucesso:** renomeação de um lado e edição de conteúdo do outro, ambas identificadas por referências confiáveis, resultam em `NoConflict` ou solução automática segura quando a conclusão estiver comprovada.

**Negativos:** identidade do recurso ou capacidade técnica incerta exige reobservação.

**Verificação:** nenhuma alteração é descartada indevidamente.

### CF-05 — `evaluate`: exclusão e edição

**Sucesso:** exclusão confirmada em um lado e edição no outro produzem `Conflict`, sem proposta automática de exclusão da versão editada.

**Negativos:** ausência de snapshot sem confirmação de escopo produz `Undetermined`, não `Conflict` comprovado.

**Verificação:** evidências e opções preservam a versão editada.

### CF-06 — `evaluate`: ausência e evidência incompleta

**Sucesso:** um lado com `Unknown`, observação parcial, hash indisponível decisivo ou baseline sem comprovação produz `Undetermined/Reobserve`.

**Negativos:** nunca inferir `AbsentConfirmed` de `None`, evento de remoção isolado ou falta de resposta.

**Verificação:** requisitos de evidência não vazios e direcionados ao dado ausente.

### CF-07 — `evaluate`: resolução automática

**Sucesso:** situação com algoritmo comprovado e parâmetros seguros devolve `Conflict/Automatic` com operações tipadas e precondições adequadas.

**Negativos:** conflito insolúvel automaticamente devolve `RequiresDecision`; capacidade não comprovada não vira comando.

**Verificação:** lista de operações não vazia, nenhuma operação executada.

### CF-08 — `evaluate`: entradas inválidas

**Sucesso:** requisição completa e coerente produz `ConflictAssessment`.

**Negativos:** `Present` sem snapshot, `AbsentConfirmed` sem evidência, baseline marcado confirmado porém ausente, identidade incompatível e conteúdo alegadamente verificado sem comparabilidade retornam `ConflictErrorKind` adequado.

**Verificação:** erro tipado e nenhuma modificação de estado.

### CF-09 — `evaluate`: coerência de resultados

**Sucesso:** cada combinação de `ConflictPresence` e `ConflictDecision` obedece à tabela de [INTERFACES.md](INTERFACES.md#coerência-entre-presença-e-decisão).

**Negativos:** proibir `Undetermined/Automatic`, opções vazias em `RequiresDecision` e operações vazias em `Automatic`.

**Verificação:** tipos e invariantes consistentes.

## Casos transversais

| Caso | Cenário | Evidência |
| --- | --- | --- |
| CF-S01 | Executar a mesma requisição repetidamente | Retorno determinístico e nenhum efeito externo |
| CF-S02 | Identidades remotas iguais em contas diferentes | Identidades não são confundidas |
| CF-S03 | `MetadataValue::NotCollected` e `PermissionDenied` | Ausência de metadado não vira igualdade ou exclusão |
| CF-S04 | Baseline ausente ou não confirmado | Nenhuma inferência de estado histórico |
| CF-S05 | Dois arquivos de mesmo nome, mas identidades diferentes | Não correlacionar apenas pelo nome |
| CF-S06 | Exclusão local confirmada versus edição remota | Conflito e preservação do conteúdo ainda presente |
| CF-S07 | Conteúdos comprovadamente iguais com nomes diferentes | Avaliação distingue identidade de conteúdo e nome |
| CF-S08 | Mudança simultânea de nome e de pasta | `findings` identifica cada dimensão independentemente |
| CF-S09 | Nenhuma capacidade para preservar ambas versões | Não oferecer `KeepBoth` inseguro |
| CF-S10 | Entrada com campos opcionais desconhecidos | Nenhum valor inventado; reobservação quando determinante |

## Critérios de aceite

1. A função `evaluate` cobre ausência de conflito, conflito comprovado e evidência insuficiente, incluindo entradas incoerentes.
2. Cada decisão contém classificação explícita, evidências e encaminhamento válido.
3. Testes utilizam objetos sintéticos completos e expectativas independentes do algoritmo.
4. A suíte não chama `Operator`, Inventário ou serviço remoto.
5. As condições de conflito com risco de perda de dados não retornam solução automática sem garantia comprovada.
6. Cenários dependentes de capacidades ainda não representadas são marcados `BLOCKED` até definição de contrato, não aceitos por suposição.
7. Testes Rust futuros serão executados exclusivamente no ambiente Flatpak do repositório, com resultados registrados.

**Estado:** plano de testes documental. Nenhum teste Rust foi executado.
