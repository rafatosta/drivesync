# Operador — contratos de recuperação

## Entrega da etapa 1

Há contratos compilados e persistência técnica no Inventory. Não há executor,
factory concreta, autenticação nova, transferência, worker, liberação de fila ou
confirmação de baseline. O Analyzer continua produzindo planos bloqueados.

## Identidade e origem

Todas as requisições recebem `ExecutionContext { id, source }`, separado das
opções de progresso/cancelamento. O chamador gera um `ExecutionId` único por ação
intencional e o conserva no retry. Mesmo comando com dois IDs significa duas
execuções; o journal não deduplica por nome, hash ou recurso. O ID aceita até 128
caracteres ASCII alfanuméricos, hífen e sublinhado, inclusive na desserialização.

`ExecutionIntent` vincula de forma imutável contexto, comando e `ExecutionScope`
(sync, conta, raiz remota e bytes Unix da raiz local). Um ID não pode trocar
origem, evidência, destino ou comando. O coordenador futuro deve construir a
origem a partir dos fatos protegidos pelo plano atual e validar suas guardas;
não pode substituir silenciosamente esses fatos por uma observação posterior.
O ID técnico não é `SyncItemId`, nem o Operator recebe itens do Inventory.

`SourceEvidence` transporta o modelo observado, sem protocolo Google:

- Local: `LocalFile` e SHA-256 opcional qualificado. Para preparar execução,
  exigir arquivo regular, tamanho, dispositivo/inode, único hardlink e tempos de
  modificação/alteração conhecidos, estruturalmente válidos e referência igual à
  do comando. Não inventar `LocalFileId` quando ausente.
- Remoto: `RemoteFile` e revisão opcional. Exigir arquivo, presença fora da
  lixeira, tamanho e versão ou revisão conhecida, no ID/provedor exatos.
- Conteúdo inline: os próprios bytes fazem parte do comando imutável; não requer
  evidência de um recurso externo. Comando sem origem não aceita origem avulsa.

Essas verificações preservam a expectativa, mas não medem arquivo nem fecham
corridas. O executor futuro deve revalidar origem, raiz/pais, alvo, capacidades e
precondições antes dos efeitos/publicação. SHA-256 local isolado não prova
igualdade com conteúdo remoto. Evidência insuficiente bloqueia a preparação.

## Porta de persistência

`ExecutionJournal: Send + Sync` é object-safe e **bloqueante**. O Inventory
implementa o trait e os três métodos públicos correspondentes, usando transações
SQLite reais. O Operator futuro deve chamar essa porta em worker bloqueante;
nenhum SQL pertence a Operations e nenhuma chamada roda no loop GTK.

| Método | Entrada | Retorno e regra |
| --- | --- | --- |
| `prepare_execution` | `PrepareExecutionRequest { intent }` | `ExecutionCheckpoint` na revisão 1, ou checkpoint atual no retry exato; ID com outro vínculo falha |
| `get_execution` | `GetExecutionRequest { id }` | Checkpoint persistido, sem requeue, reset ou mudança de revisão |
| `save_execution` | `SaveExecutionRequest { id, expected_revision, state }` | CAS atômico; gravação divergente com revisão antiga falha; retry exato da gravação imediatamente anterior recupera seu resultado |

Os métodos do Inventory retornam `InventoryResult`; o trait traduz falhas para
`OperatorFailure` sanitizada. `CheckpointConflict` indica conflito de revisão/ID;
`CheckpointUnavailable` abrange falha do journal. Erro na porta tem efeito
`Unknown`: falha do banco não é prova de que um efeito físico anterior não ocorreu.
Não repetir uma mutação por causa desse erro.

## Checkpoint e efeitos

O estado persiste fase, efeito, staging com metadados/tamanho/SHA-256, identidade
remota reservada, token sensível de sessão, offset confirmado e resultado final.
Preparar e atualizar esses registros não altera fila, itens, associações,
namespace de planejamento ou baseline e não autoriza execução.

| Fase | Efeito permitido | Recuperação |
| --- | --- | --- |
| `Prepared` | `NotApplied`, sem resultado ou sessão já iniciada | Revalidar antes de começar |
| `Executing` | `Unknown`, sem resultado | Reconciliar efeito antes de qualquer reenvio |
| `Verifying` | `Unknown` ou `Applied`, sem resultado | Verificar recurso por identidade |
| `Completed` | `Applied` e resultado `Verified` | Recuperar resultado histórico; não repetir comando |

Persistir `Executing/Unknown` **antes** de despachar uma mutação. A máquina aceita
Prepared→Prepared/Executing, Executing→Executing/Verifying e
Verifying→Verifying/Completed. Não regride fase, não apaga efeito aplicado nem
transforma cancelamento solicitado em ausência de efeito. Completed é imutável;
o retry de sua gravação usa a revisão anterior, não abre nova execução.

Staging, ID remoto reservado e sessão já persistidos não podem ser trocados ou
apagados; offset só avança e não pode superar o tamanho da origem. Criação remota
precisa reservar o ID antes de sair de Prepared. Origem local exige staging
verificado antes de Executing. Token só aparece com identidade remota e fora de
Prepared. O vínculo e os dados persistidos são revalidados nas leituras; corrupção
falha explicitamente, sem reset. A gravação de intenção e primeiro checkpoint é
uma transação única.

Este contrato inicial é conservador: **não há transição para descartar/reiniciar
sessão expirada nem para reexecutar efeito incerto**. Antes da etapa de upload,
acrescentar protocolo explícito de reconciliação com evidência do recurso e da
sessão anterior. Não substituir token/resetar offset com uma gravação genérica.
A etapa 1 valida recuperação do registro, não retomada física de transferência.
CAS protege gravações, não é lease de worker e não autoriza dois executores a
produzir efeitos; elegibilidade e exclusão mútua pertencem à integração futura.

## Resultado e identidade após publicação

`ExecutionResult` guarda `ItemResult` ou `TransferResult`, incluindo situação de
integridade e bytes da execução original. Recuperar esse resultado não significa
que tais bytes foram transferidos novamente na chamada atual. Progresso não
atualiza checkpoint por presunção nem serve como prova de conclusão.

`ItemResult.resource` contém a identidade realmente publicada. Atualizar conteúdo
local por staging pode mudar o inode/ID físico; conservar caminho lógico não
prova preservação dessa identidade. O journal admite essa mudança no mesmo alvo
local. Atualização remota deve conservar o ID remoto. A integração futura valida
a evidência, renova o vínculo físico e reobserva os lados antes de confirmar
baseline. O Operator nunca confirma associação ou baseline por conta própria.

## Privacidade e esquema

O esquema do Inventory é **7**. Esquemas anteriores são preservados e recusados,
sem migração, exclusão ou recriação automática. `execution_intents` conserva o
vínculo e `execution_checkpoints` conserva as colunas de recuperação. O token é
BLOB privado de até 8192 bytes, com Debug redigido. Ele não deve conter OAuth e
só o especialista deve interpretar sua representação; o Provider deve validar a
origem de qualquer URI antes de enviar credenciais.

As novas tabelas não são expostas pela allowlist do inspetor SQL da GUI, para
não apresentar tokens de retomada. A API tipada é destinada à recuperação
interna. Serialização é persistência sensível, não formato para logs.

## Pendências dos Providers e limites

O Provider atual possui upload resumable de criação/atualização, consulta de
sessão e download por range/revisão. Não há contrato de escrita condicional
`If-Match` na interface inspecionada. Leitura de versão seguida de escrita não
impede alteração concorrente. Nenhum Provider foi alterado nesta etapa.

Antes de implementar mutação remota dependente de precondição, apresentar a
limitação e verificar o suporte efetivo da API. Propor uma extensão tipada mínima
que transporte a condição e diferencie rejeição, efeito confirmado e resposta
perdida. Se a plataforma não oferecer a proteção necessária, manter bloqueado.
A validação do protocolo remoto será feita nessa etapa, não presumida aqui.

Lixeira local, publicação/recuperação física, substituição atômica e retomada de
sessão ainda não estão implementadas. `delete` permanece proibido e deve recusar
antes de I/O, journal ou autenticação; `OPR-L-11` foi corrigido para exigir isso.
