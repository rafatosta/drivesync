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
localização instalada é tratado como relocação observada pendente, sem baixar
outra cópia nem avançar B. Mudança projetada somente por ancestral permanece
desconhecida sem prova de identidade do pai.
O executor verifica novamente B quando L desaparece depois do planejamento.

A coleta pode emitir `Classified`, com decisão tipada e caminho local associado,
para combinações de localização/conteúdo. O executor recusa esse candidato sem
transferência ou avanço de B; o diagnóstico diferencia conciliação pendente de
conflito real. Não se habilitam renomeações, movimentos ou atualização remota.

## SYNC-009 — destinos representáveis e sem ambiguidades

Política DriveSync: bloquear ambiguidades, sem escolher vencedor, sanitizar nomes
ou inventar sufixos. Não há mapeamento alternativo a persistir: caminhos aceitos
continuam associados ao ID no plano/journal existente. A decisão é conservadora
inclusive em filesystems sensíveis a caixa.

1. Validar cada componente antes/depois de acrescentar `.desktop` a Workspace.
   Rejeitar vazio, pontos, separadores, controles, `< > : " | ? *`, ponto/espaço
   final, dispositivos Windows (também com extensão), nomes internos sem distinção
   de caixa e componentes acima de 255 bytes ou unidades UTF-16.
2. Produzir chave `NFD(full_case_fold(NFD(nome)))` com `caseless` e
   `unicode-normalization`. As dependências evitam implementar tabelas Unicode
   manualmente; a chave só detecta colisões, nunca substitui o nome ou o ID.
3. Contar chaves por ID do pai após conversão para nome local. Só uma ocorrência
   permite caminho; propagar bloqueio por toda subárvore ambígua. Recusar caminho
   absoluto com 4096 bytes ou mais, incluindo a raiz local.
4. Na preparação completa, indexar também os caminhos do inventário local.
   Grafia equivalente diferente, ou várias grafias equivalentes, bloqueiam o
   destino e descendentes antes das provas remotas. Arquivo/diretório incompatível
   resulta em `LocalTypeMismatch`, incluindo diretórios no destino de links.
5. Persistir os diagnósticos pelo mecanismo existente; executar somente após as
   revalidações independentes do executor. Reabrir e repetir não autoriza efeitos.

Oráculos: colisões deixam todos os IDs bloqueados sem ler conteúdo/metadados
remotos; bytes locais e bases permanecem iguais após repetição/reabertura.
Nomes iguais em pais distintos permanecem independentes. Os testes `sync009_*`
cobrem C02/C13/C14/C15/C18/C20 junto aos testes `i10_i11_i13_*`, `i12_i13_*`,
`i13_same_names_*` e `i14_*` existentes.

Limites: esta política não identifica todas as regras proprietárias de cada
filesystem/mount. Limites efetivos menores e erros de acesso continuam falhando
sem substituição silenciosa no executor. Não se faz probe destrutivo para descobrir
sensibilidade a caixa. Não há teste em volume case-insensitive real: os testes
exercitam deterministicamente a proteção conservadora em diretórios Linux reais.
Colisões remotas surgidas depois do snapshot são observadas na próxima varredura;
a execução mantém suas proteções de identidade, versão e instalação sem clobber.
Não se adiciona resolução, upload, exclusão nem decisão automática de vencedor.

Fundamentação consultada em 2026-10-30:
[Drive File.name](https://developers.google.com/workspace/drive/api/reference/rest/v3/files)
não garante unicidade no pai;
[Linux pathname(7)](https://man7.org/linux/man-pages/man7/pathname.7.html)
distingue limites por componente e caminho;
[Caseless](https://docs.rs/caseless/0.2.2/caseless/trait.Caseless.html)
fornece full case folding. A escolha de bloqueio portátil é política do produto,
não uma alegação de equivalência exata a todos os filesystems.
[Windows naming conventions](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)
fundamenta caracteres e dispositivos reservados;
[Unicode canonical caseless matching](https://www.unicode.org/versions/Unicode17.0.0/core-spec/chapter-3/#G1459)
fundamenta decomposição e folding. As tabelas efetivas são as versões das crates
fixadas no lockfile; não se promete suporte antecipado a caracteres futuros.

## SYNC-017 — movimentações concorrentes e destinos alterados

`decide_move` (módulo `planner::moves`) é puro e opera sobre IDs de item/pai, nunca
sobre caminhos ou nomes. Entrada: evidência de movimento local/remoto contra o pai
de B, mudança de conteúdo de cada lado, conflito de conteúdo e as árvores de IDs
local (mapeada) e remota.

1. Evidência `Unknown` em qualquer lado suspende a decisão (`IncompleteEvidence`).
2. Destinos distintos (C10, C17) → `Preserve(DivergentDestinations)`; não há vencedor.
3. Mesmo destino com identidade contínua → `Equivalent`.
4. Movimento de um lado só → `Apply` candidato. Edição do outro lado (C12) é
   combinada (`combined_edit`), salvo conflito de conteúdo ou evidência desconhecida.
5. O destino é validado nas duas árvores com o item reparentado: deve existir,
   estar disponível, ser pasta e ter identidade contínua (C19), e a cadeia de pais
   deve alcançar a raiz sem passar pelo próprio item (ciclo) nem por pai ausente
   (órfão). Movimentos concorrentes de ancestrais são capturados pela validação
   conjunta.

O resultado é apenas candidato: não autoriza efeito e exige revalidação do executor.
Não há execução de move local/remoto nesta etapa; integração ao coletor/executor e
validação com Google Drive real permanecem pendentes. Testes: `planner::moves::tests`.
