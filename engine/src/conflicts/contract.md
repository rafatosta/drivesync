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
