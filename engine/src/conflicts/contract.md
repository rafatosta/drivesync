# Contrato — conflicts

## Classificação SYNC-006

A decisão pura recebe diferenças de L e R em relação a B, ausência explícita de
B ou B inválida, e a comparação de conteúdo L/R. O adaptador calcula diferenças
somente com evidências verificadas no mesmo escopo de conta, cliente e raízes.
Nome, pai e conteúdo são dimensões independentes. Identidade substituída tem
precedência sobre equivalência; dispositivo/inode não são identidade permanente.

Estados distintos: presente, ausência observada, exclusão confirmada,
indisponibilidade com causa, desconhecido e incompatibilidade. Ausência em um
inventário não é exclusão confirmada. Revisões regressivas, fingerprints inválidos
e provas incompletas não autorizam ação. Timestamps não decidem precedência.

Saída: classificação, encaminhamento e motivo tipados. Sincronizar/conciliar são
candidaturas, nunca autorização de execução. Preservar significa manter intactas
as versões disponíveis; não significa que cópias adicionais foram feitas.

## Ordem determinística

1. Rejeitar B inválida, desconhecido, indisponibilidade e diferenças incompletas.
2. Bloquear incompatibilidade e substituição de identidade, mesmo com bytes iguais.
3. Sem B: permitir candidato novo somente no lado oposto comprovadamente ausente;
   bloquear adoção de preexistentes e inferência de exclusão.
4. Com B: distinguir exclusão confirmada, ausência observada e modificação.
5. Comparar conteúdo e cada atributo de localização separadamente: destinos
   iguais convergem; destinos diferentes conflitam; nome/pai versus conteúdo são
   conciliáveis com identidade contínua. Destino seguro precisa de revalidação.
6. Conteúdo concorrente igual converge; diferente conflita; não comparado permanece
   preservado e pendente de prova. Não inventar conflito comprovado.
7. Sem mudança: nenhuma operação; mudança unilateral: candidato sujeito à política.

## Integração e segurança

O planner traduz a classificação para ações suportadas pelo executor de downloads.
Upload de atualização, propagação de exclusão, aplicação de rename/move e resolução
assistida não são habilitados por este classificador. Ausência local sem prova de
exclusão mantém a reposição conservadora existente, exceto quando R também mudou:
esse caso requer preservação/revisão, sem recriar silenciosamente.

Coleta e prova remota ficam em changes; a classificação da prova fica em conflicts.
Revisão remota nova é evidência de mudança potencial: a prova compara bytes com B
além de comparar L/R. Se os bytes de R continuam iguais a B, uma edição de L é
mudança apenas local, mesmo com revisão remota maior.
O journal fornece também a localização instalada de B. A coleta pode reconhecer
um caminho local novo somente se o antigo estiver ausente e existir um único
candidato com hash/dispositivo/inode de B. Não é identidade permanente contra
reutilização de inode com bytes iguais. Relocação com conteúdo local também
alterado e pais locais sem identidade remota verificável permanecem ambíguos.
O plano `Classified` guarda decisão e caminho local associado e não é executável.
A reconciliação evita apresentar esse caminho também como arquivo novo. O upload
create-only não cria outro item para uma identidade gerenciada observada em
caminho novo. Não é implementação da aplicação bidirecional de rename/move.
A apresentação deriva do resultado central ou dos bloqueios estruturais do plano.
Classificação não escreve arquivos, não acessa rede/banco e nunca avança B.
Execução revalida escopo, identidade, bytes e revisão. Observações/pendências usam
a reconciliação SQLite existente, separada do journal de instalações confirmadas.
Repetir entrada produz resultado idêntico; falhas/reinício exigem nova observação.

## Fundamentação e limites

Técnica: comparação de três vias sobre estado confirmado. Política DriveSync:
preservar versões e bloquear exclusão/atualização não autorizada, sem escolher
por timestamp. Referências primárias consultadas em 2026-10-30:
[Files](https://developers.google.com/workspace/drive/api/reference/rest/v3/files)
e [Changes](https://developers.google.com/workspace/drive/api/reference/rest/v3/changes).
Revisão remota inclui alterações de metadados; `removed` pode representar perda
de acesso. Não se presume compare-and-swap genérico ou histórico completo de eventos.

C01/C03/C04 exigem preservação; C09/C12 são conciliáveis apenas com evidência de
identidade e atributos; C21 exige equivalência comprovada; P01 permite apenas
operação suportada. Os testes e evidências de execução pertencem ao commit/PR;
o catálogo não indica homologação com Google Drive real.

## SYNC-007 — detecção de edições divergentes

A coleta refina o candidato concorrente somente após comparar R/B e L/R. Antes de
publicar equivalência, divergência ou uma decisão de localização, confere novamente
assinatura local (incluindo ctime), hash e identidade por descritores sem seguir
symlinks, na raiz já bloqueada. Prova local obsoleta/inacessível vira desconhecida;
cancelamento e suspensão da raiz abortam a preparação. Não há atomicidade entre
observações dos dois sistemas; o resultado não autoriza resolução posterior.

Pseudocódigo: classificar B/L/R → verificar metadados/bytes remotos → comparar R/B
e L/R → validar raiz e assinatura/hash/identidade de L → publicar decisão ou adiar
→ persistir observação separada de B → reobservar antes de qualquer efeito.

O conflito sinaliza as origens local e Drive, sem anunciar cópias não realizadas.
Preservação nesta entrega é bloqueio de substituição com originais intactos;
backup de conflito e resolução assistida permanecem fora deste contrato.

Oráculos determinísticos (resultados de execução pertencem ao PR):

| Cenário | Teste e efeito esperado |
| --- | --- |
| C01/C06 igual, divergente, offline e retomada | `c01_c06_divergence_offline_and_recovery_persist_without_selecting_a_version` em `download/sync006_acceptance.rs`: reabre SQLite a cada ciclo, verifica mensagem com ambas as origens, bytes intactos, nenhuma cópia/avanço de B. |
| C01 concorrência durante leitura remota | `c01_local_changes_during_remote_proof_suspend_stale_diagnosis` em `download/reconciliation_tests.rs`: edição e substituição com bytes iguais invalidam tanto equivalência quanto divergência; nenhuma instalação. |
| C06 mudanças unilaterais e divergência | `c06_reopened_offline_local_remote_and_divergent_changes_preserve_bases`: só mudança remota unilateral pode instalar; conflito mantém L/R/B. |
| Prova incompleta/cancelamento | `incomplete_remote_equivalence_proof_is_unknown_without_media_installation`, `cancellation_during_equivalence_proof_never_publishes_a_plan`: não publicar sucesso sem prova. |
| Falha de persistência | `failed_observation_commit_and_cancellation_keep_the_previous_classification`: transação abortada conserva geração anterior; não avança B. |

Todos usam provedor remoto simulado; efeitos de arquivos e persistência usam
filesystem/SQLite reais temporários. Reabertura não simula perda de energia.
Referência primária: [Files](https://developers.google.com/workspace/drive/api/reference/rest/v3/files),
consultada em 2026-10-30: revisão não é prova de mudança de conteúdo; checksums e
metadados têm limites de disponibilidade. Nenhuma nova capacidade remota é presumida.

## SYNC-008 — edição e indisponibilidade

A reconciliação compara observações históricas com a localização e fingerprint
de B confirmada, inclusive quando a identidade remota saiu do inventário.
Conteúdo alterado na mesma identidade local gera
`LocalChangedRemoteUnavailable`; identidade substituída continua incompatível.
Uma relocação local só é associada quando o caminho de B está ausente e existe
um único fingerprint completo igual (hash/dispositivo/inode). Essa associação
não transforma o item em upload novo. Relocação com edição simultânea permanece
ambígua, sem inferência de exclusão.

Pendências de descendentes elevam a atenção dos diretórios históricos
indisponíveis, por componentes de caminho. Nenhuma análise permite excluir,
substituir, recriar o remoto ou avançar B. A observação e suas evidências persistem
no SQLite de reconciliação; reabertura não concede autorização para efeitos.

### Evidência de lixeira e exclusão versus localização

O adaptador conserva `confirmed_trashed` somente quando metadados explícitos
`trashed=true` pertencem ao ID do evento. `removed=true`, ausência em scan,
HTTP 404, falha de acesso e timeout não estabelecem essa prova. O checkpoint
registra a prova no aviso daquele ID/caminho; avisos de journal não a inventam.
A evidência pertence à observação, não afirma que o item ainda esteja na lixeira
no momento de uma futura decisão. Um novo evento ambíguo remove a certeza anterior.

Pseudocódigo: coletar evento → validar ID → distinguir lixeira/indisponibilidade
→ podar subárvore remota sem efeito local → registrar avisos dos descendentes
→ comparar cada localização de B com L → associar relocação única ou manter
incerteza → elevar atenção dos ancestrais → persistir observações → reobservar
antes de qualquer efeito. Descendentes de pasta na lixeira permanecem
indisponíveis: não herdam uma prova individual de exclusão.

`RemoteDeleted` indica observação de lixeira sem alteração local comprovada;
`EditDeleteConflict` indica edição/relocação ou descendente alterado contra essa
observação. Prova incompleta permanece `Unknown`; falta de acesso nunca é
apresentada como exclusão confirmada. Diretórios com novo descendente também
exigem revisão. Não se cria cópia de conflito nem se promete recuperar uma
versão remota inacessível.

Ausência do caminho local de B, sem relocação verificável, somada a rename/move
remoto bloqueia a recriação em destino novo (`LocalMissingRemoteChanged`).
Continua sendo ausência observada, não intenção comprovada de exclusão local.

Os formatos internos exatos passam a checkpoint v2 e reconciliação v3.
Versões anteriores são rejeitadas sem migração, exclusão ou reset automático;
arquivos de usuário e credenciais não mudam. Estado de desenvolvimento antigo
exige recuperação explícita conforme o fluxo existente.

### Oráculos SYNC-008

| Cenários | Teste determinístico e efeito esperado |
| --- | --- |
| C03/C08/C11 | `c03_c08_c11_unavailable_remote_preserves_edits_and_relocations_after_restart`: edição, rename, move, substituição, ausência e ausência de avisos; distingue lixeira de indisponibilidade; persiste/reabre sem upload novo, instalação ou avanço de B. |
| C03/C16 | `c03_c16_folder_trash_or_access_loss_preserves_descendants_through_failure_and_restart`: evento só do pai, offline, falha transacional após checkpoint, retomada e repetição; filho editado intacto e pai pendente. |
| C04/C08/C11/C16 | `c04_c08_c11_c16_missing_local_tree_and_remote_changes_require_review`: ausência local com edição/rename/move remoto, inclusive subárvore; nenhuma recriação de arquivo. |
| C08/C11 | `c08_c11_confirmed_deletion_conflicts_with_location_changes_in_both_directions`: classificação pura simétrica, preservação obrigatória. |
| Evidência remota | `explicit_trash_is_distinct_from_removed_and_requires_matching_identity`: lixeira explícita, evento ambíguo e ID inválido. |
| Concorrência | `local_removal_after_remote_update_planning_invalidates_the_candidate` em `download/reconciliation_tests.rs`: remoção após plano bloqueia instalação. |

Os testes de integração usam arquivos e SQLite temporários reais e provedor
remoto simulado. Os resultados e comandos de execução pertencem ao PR.
Reabertura/falha transacional não comprova recuperação de perda real de energia.


## SYNC-010 — preservação sem seleção (P02/P08)

O [contrato de preservação](preservation.md) complementa a classificação pura:
o executor de download pode criar uma cópia local verificada da versão remota
em divergência de conteúdo binário gerenciado no mesmo caminho, mantendo o
arquivo local original e a base B. Isso substitui, para esse cenário, o limite
anterior de apenas preservar as origens sem cópia adicional. Classificação
isolada continua sem efeitos e nunca comprova que uma cópia foi criada.
`DownloadReport.conflict_copies` informa caminho original, cópia, ID e revisão
somente após verificar identidade/tamanho/hash; `attention()` mantém pendência
e apresenta o local recuperável. Falhas específicas não anunciam cópia íntegra.
`conflict_copies` no SQLite principal v3 é separado de `downloads`: não fornece
base nem histórico de exclusão. Formato v2 é rejeitado sem reset/migração.

## SYNC-011 — pendências e auditoria duráveis

`ConflictHistory` persiste um registro pendente por chave de item/caminho e
eventos append-only para mudanças verificadas de diagnóstico, cópias de conflito
publicadas e falhas classificadas. Uma observação posterior `Unchanged` ou
`Equivalent` retira o registro da lista atual de pendências sem apagar eventos;
uma ausência de evidência não fecha a pendência. Cada entrada mantém escopo da conta/raízes,
origem observada (ID/pasta/nome remoto e caminho local), versão remota e
fingerprint local quando disponíveis. O
journal não armazena bytes, tokens ou mensagens cruas do provedor. Repetir uma
observação ou resultado idêntico não duplica o evento; uma alteração observada
gera novo evento e mantém as versões anteriores no histórico.

A reabertura fornece registros e eventos sem rede, mas não permite reaplicar uma
decisão. Só efeitos comprovados são descritos como preservados; falha/ausência
permanece pendente. Resolução assistida e fechamento de conflitos não estão
implementados. Os testes de persistência usam SQLite real temporário e cobrem
reabertura, idempotência, isolamento de escopo e rollback; não demonstram queda
real de energia ou integração com Google Drive.

Oráculos P02/P08: `download/preservation_tests.rs` cobre reabertura, repetição,
revisões seguintes, offline de mídia, falha de integridade/revisão, edição local,
colisão de arquivo/symlink/diretório e criação concorrente, replay antes/depois
do rename, rollback SQLite antes da intenção/Ready/Installed, cancelamento,
bloqueio da raiz, staging/cópia corrompidos, isolamento de escopos e continuidade
de itens independentes. Os resultados de execução pertencem à PR.

## SYNC-012 — revalidação antes de aplicar resolução (N06/E06)

`ResolutionProposal` deriva apenas de um `ConflictRecord` pendente com ID, pasta,
nome, revisão remota, caminho e fingerprint local completos. Imediatamente antes
de agir, o chamador coleta uma `ResolutionObservation` nova e
`DriveSyncEngine::revalidate_resolution` compara identidade remota (ID/pasta/
nome/caminho), revisão remota exata (inclusive regressão) e fingerprint local.
Qualquer diferença, ausência ou evidência desconhecida aborta a proposta com uma
`StaleResolutionReason`, registra `ConflictOutcome::ResolutionAborted` (evento
`ResolutionAborted`, idempotente) e mantém a pendência: é exigida nova comparação.
`Ok` não autoriza escrita; executores mantêm suas próprias pré-condições.
Registro inexistente, de outro escopo ou já encerrado resulta em `NotPending`
sem eventos. A aplicação de resolução assistida continua fora de escopo.

## SYNC-015 — projeção de pendências na interface

`ConflictRecord` inclui tamanho local, mtime local em segundos Unix e tamanho
remoto opcionais, colhidos das observações estáveis/inventário; ausência é
explicitamente desconhecida. O journal de conflitos usa formato exato v3,
sem migração ou reset automático dos formatos anteriores. Não há inferência de versão vencedora
por data nem consulta de caminhos históricos na apresentação. A central usa
`saved_conflicts` sem rede e não altera status ou histórico ao adiar/fechar.
Os registros continuam sendo observações, não autorizações de resolução.


## SYNC-016 — proteção de renomeações

O [contrato do executor compartilhado](resolution-execution.md) define a condição
de habilitação, as transições recuperáveis e o experimento de concorrência
necessários para concluir a aplicação de SYNC-015/SYNC-016.

### Entrada, política e efeitos permitidos

Usar a base instalada por ID remoto e o inventário completo da mesma conta/raiz.
Localizar L somente com caminho antigo ausente e candidato único com
hash/dispositivo/inode de B; a prova já existente de inode e bytes no caminho
original continua necessária para edições sem relocação. Comparar separadamente
nome, pai e conteúdo de L/R. Consultar R antes/depois da leitura e revalidar L
após a rede. Nenhuma decisão de localização autoriza I/O de mutação ou avanço de B.

Pseudocódigo determinístico:

1. Rejeitar evidência ausente, substituída, ambígua ou versão regressiva.
2. Se somente o caminho projetado mudou por um ancestral, suspender: a identidade
   do pai local não é demonstrada apenas por aquele caminho.
3. Para alteração verificável de localização, provar R/B e R/L, incluindo quando
   somente R foi renomeado. Falha de prova produz desconhecido, nunca download.
4. Nomes finais iguais convergem; nomes finais distintos exigem decisão (C07).
   Nome/pai versus conteúdo independentes produzem candidato conciliável (C09).
5. Um caminho local não representado cujo dispositivo/inode coincide com B fica
   desconhecido, inclusive offline, com aliases ou conteúdo alterado. A coincidência
   somente impede classificá-lo como novo; não autoriza adoção nem vínculo por ID.
6. Persistir observações com o CAS SQLite existente, separado de instalações.
   Falha/rollback/cancelamento mantém a observação anterior. Reabrir não autoriza
   aplicar a decisão; repetir a coleta e a revalidação.

### Oráculos e limite de implementação

[sync016_acceptance.rs](../download/sync016_acceptance.rs) cobre C07 convergente
ou divergente, C09 nos dois sentidos, offline/retomada, reabertura/repetição,
corridas de conteúdo/identidade/revisão durante a prova, aliases, ancestral
renomeado, rollback SQLite e cancelamento de persistência. São arquivos e SQLite
reais temporários com remoto simulado. Os testes exigem nenhum download/cópia
falsa, preservação de L/R e B inalterada; não comprovam aplicação de C09.

A aplicação de mesmo pai usa o [executor compartilhado](resolution-execution.md):
intenção durável por identidade, precondição forte v2, preservação, instalação
local sem substituição e confirmação transacional de B após conferir efeitos.
O coordenador só inicia combinações automáticas com upload habilitado. O fluxo
create-only não substitui esses efeitos. Mudança de pai/árvore permanece em
SYNC-017; não contornar a restrição criando outro ID remoto ou cópia ativa falsa.
Os testes de `resolution` cobrem efeitos com filesystem/SQLite reais e remoto
simulado; ensaios opt-in dos adaptadores e coordenador Google usam fixtures novas.
Nenhuma dessas evidências equivale a queda de energia, leitor de tela ou
funcionamento do aplicativo empacotado instalado.

Fundamentação consultada em 2026-10-30: comparação de réplicas com estado anterior
([Unison](https://github.com/bcpierce00/unison)); atualização do mesmo ID via
[files.update](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/update).
A referência da API não constitui evidência de execução, CAS distribuído ou
recuperação implementada. O limite conservador de execução é política DriveSync.
