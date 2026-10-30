# Algoritmos — planner

## Escopo e estado

PLAN-001 implementa a classificação parcial B/L/R do fluxo de download de
binários da [SYNC-001](https://github.com/rafatosta/drivesync/issues/1).
Os requisitos e critérios vigentes estão na
[especificação, seção 4.7](../../../docs/SOFTWARE_SPECIFICATION.md#47-detecção-de-mudanças-e-sincronização).
A [matriz de rastreabilidade](../state/preparation.md#rastreabilidade-das-regras-do-eixo-1)
liga regras, implementação e testes; não representa aceite com Google real.

## PLAN-001 — entradas e identidade

[mod.rs](mod.rs) recebe inventário com conta, raiz remota, IDs/pais, nomes,
tipos e revisões; raiz local absoluta; bases opcionais por ID remoto; e um
observador local injetado. O chamador deve carregar B no escopo correto de
cliente OAuth, conta e raízes, conforme [STATE-001/002](../state/algorithms.md).

`ConfirmedBase` contém revisão remota positiva e fingerprint local
`SHA-256:device:inode`. A fingerprint não é somente igualdade de bytes.
`LocalObservation` distingue ausência, diretório, arquivo não gerenciado,
symlink/especial, desconhecido e arquivo verificado. A [fachada](../facade.rs) usa o [inventário local completo](../local/reconciliation.md),
com fingerprint de arquivos estáveis e evidência desconhecida para arquivos em
alteração. Erros de leitura não são convertidos em ausência.

O algoritmo puro `decide` não acessa disco/rede nem persiste estado.
`plan_with_bases` chama o observador fornecido; sua implementação de produção
faz leituras locais. Nenhuma dessas funções instala arquivos ou confirma B.

## Decisões para binários

Aplicar primeiro: revisão remota zero, regressão em relação a B ou fingerprint
inválida/incompleta em B resultam em `DeferredUnknown`. Depois:

| Evidência local L | B | Revisão R em relação a B | Decisão |
| --- | --- | --- | --- |
| Desconhecida | Qualquer | Não regressiva | `DeferredUnknown` |
| Ausente | Com ou sem B | Válida e não regressiva | `Download` |
| Verificada | Ausente | Válida | `Blocked(ExistingLocalData)` |
| Igual à fingerprint de B | Confirmada | Igual | `NoOp` |
| Igual à fingerprint de B | Confirmada | Maior | `Download` |
| Mesmo device/inode, hash diferente | Confirmada | Igual | `Blocked(LocalChanged)` |
| Mesmo device/inode, hash diferente | Confirmada | Maior | `Blocked(ConcurrentChanges)` |
| Device/inode diferente, qualquer hash | Confirmada | Igual | `Blocked(LocalIdentityChanged)` |
| Device/inode diferente, qualquer hash | Confirmada | Maior | `Blocked(ConcurrentIdentityChanges)` |
| Fingerprint verificada incompleta | Confirmada | Não regressiva | `DeferredUnknown` |
| Arquivo não gerenciado | Qualquer | Válida | `Blocked(ExistingLocalData)` |
| Diretório ou tipo especial | Qualquer | Válida | `Blocked(LocalTypeMismatch)` |

Ausência local permite propor reposição, não propagar exclusão. Não existe
entrada de ausência remota em `decide`; a matriz geral de exclusões não está
implementada aqui. Igualdade local/remota sem B não autoriza adoção. Timestamps
não participam da decisão. Revisão maior sozinha não prova mudança de bytes:
a continuação por metadados é decisão revalidada do
[executor](../download/updates.md), não equivalência inferida pelo planner.

## Construção do plano e erros

Resolver caminhos pelo grafo de IDs/pais, bloquear nomes inseguros/reservados,
colisões entre irmãos e seus descendentes. Itens sem caminho seguro ficam
bloqueados; binários sem revisão ficam desconhecidos. Diretórios e links têm
regras próprias, fora da comparação B/L/R de binários.

`InvalidPlan` sinaliza estrutura inválida, raiz não absoluta ou limites de
entrada excedidos; erros do observador são propagados. O retorno é um
`DownloadPlan` com escopo, ID, caminho, revisão esperada, observação e ação.
O plano não é autorização de escrita: o executor revalida journal, identidade,
conteúdo, configuração e metadados antes dos efeitos.

Testes puros: `three_way_requires_confirmed_local_evidence` e
`three_way_classification_requires_a_confirmed_base_and_fresh_local_fingerprint`
em [tests.rs](tests.rs). Caminhos: `ambiguous_and_unsafe_names_never_reach_observer`,
`folder_ambiguity_blocks_descendants_and_missing_versions_defer` e
`metadata_observer_preserves_files_and_rejects_symlinks`. Os testes integrados
com B durável estão na matriz de rastreabilidade citada acima.

## Limites e decisões futuras

Classificação geral de exclusões, renomeações, equivalência de revisões e
resolução bidirecional permanecem propostas dos contratos de conflitos.
`LocalChanged` e `ConcurrentChanges` são saídas distintas do planner; o relatório
de execução pode expor somente `DownloadConflict`. Não há evidência de aceite
da apresentação dessas classificações na interface.

SYNC-002 continua como contrato de revalidação para operações futuras, incluindo
atualização remota: comparar antes de escrever não elimina TOCTOU. Não autorizar
substituição remota sem demonstrar precondição atômica ou política conservadora
aprovada de preservação. Os testes de download não validam GET/PATCH concorrentes
nem comprovam capacidades de escrita condicional da API Google.

Referências de projeto preservadas: [reconciliação Unison](https://www.cis.upenn.edu/~bcpierce/unison/),
[Drive changes](https://developers.google.com/workspace/drive/api/guides/manage-changes),
[HTTP RFC 9110](https://www.rfc-editor.org/rfc/rfc9110) e
[Drive files.update](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/update).
São fundamentação, não evidência de integração real.

## SYNC-002 — diagnóstico de identidade local

Contrato RF-ID-01–07 da especificação. Para binários com B:

1. Rejeitar revisão zero/regressiva e fingerprint inválida como `DeferredUnknown`.
   Decodificar exatamente SHA-256 hexadecimal, device e inode; inode zero não
   oferece evidência utilizável. Erro de observação continua sendo erro/desconhecido.
2. Se L for regular verificado, comparar device/inode antes dos bytes. Par diferente
   resulta em `LocalIdentityChanged` ou `ConcurrentIdentityChanges`, conforme R
   permaneça igual ou avance em relação a B. Hash igual não elimina essa diferença.
3. Com mesmo par, hash diferente mantém `LocalChanged`/`ConcurrentChanges`;
   hash e par iguais mantêm `NoOp`/`Download`. Tipo incompatível é `LocalTypeMismatch`.
4. Sem B, não adotar arquivo existente. Somente a execução revalidada confirma B.

O diagnóstico é verificável no plano persistido por item/escopo. O relatório
continua podendo expor `DownloadConflict`; não se exige alteração da GUI.
A função de comparação não escreve nem consulta rede; repetir entradas produz
mesmo resultado. O executor mantém a revalidação independente antes dos efeitos.

Fundamentação consultada em 2026-10-28: [Rust MetadataExt](https://doc.rust-lang.org/std/os/unix/fs/trait.MetadataExt.html)
expõe device/inode; [Linux inode(7)](https://man7.org/linux/man-pages/man7/inode.7.html)
limita a unicidade do inode ao filesystem. A separação dos motivos é política
DriveSync. Esse par é evidência observável, não identificador eterno; não demonstra
continuidade contra reutilização acompanhada de bytes iguais ou rollback conjunto.

## SYNC-003 — equivalência e observações locais completas

`changes::prepare` constrói o plano com o inventário local completo e refina
candidatas `ConcurrentChanges`, após a classificação de identidade de `decide`.
Para o mesmo objeto com B, bytes remotos são lidos para um sink, limitados ao
tamanho esperado, com MD5 do provedor e SHA-256 calculados. Metadados exatos são
verificados antes/depois; SHA-256 igual ao local permite `Equivalent`.
Erro de leitura ou revisão alterada resulta em `DeferredUnknown`; cancelamento,
configuração alterada e falha de estado interrompem a preparação.

`Equivalent` não autoriza escrita. O executor verifica B atual, vínculo de
caminho/identidade/parent/root, versões, cadeia remota e conteúdo local/remoto
novamente. Somente conta um resultado sem efeito; não registra instalação nem
avança B. Repetição pode reler o conteúdo, mantendo a mesma decisão e os bytes.
Sem B ou com inode diferente, a equivalência não é tentada. O caso de revisão
maior com L intacto mantém a atualização revalidada do executor existente.

O relatório carrega classificações anteriores à execução; não representam
confirmação de que as operações propostas terminaram. Ausência remota e caminhos
históricos são indisponibilidade observada, sem propagação de exclusão.

## SYNC-006 — decisão central e localização

`decide` delega ao adaptador de [conflicts](../conflicts/contract.md). A prova de
conteúdo também compara R com B, para não confundir revisão de metadados com
edição remota. `LocalMissingRemoteChanged` impede recriar silenciosamente L ausente
no caminho instalado quando R avançou. Um novo caminho remoto comprovado pela
localização instalada é tratado como relocação observada, preservando a cópia antiga.
O executor verifica novamente B quando L desaparece depois do planejamento.

A coleta pode emitir `Classified`, com decisão tipada e caminho local associado,
para combinações de localização/conteúdo. O executor recusa esse candidato sem
transferência ou avanço de B; o diagnóstico diferencia conciliação pendente de
conflito real. Não se habilitam renomeações, movimentos ou atualização remota.
