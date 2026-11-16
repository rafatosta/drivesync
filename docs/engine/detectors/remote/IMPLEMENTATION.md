# Perfil remoto implementado nesta etapa

A solicitação de implementar o Detector Remoto inclui a seleção do modelo necessária à observação e à fila. O catálogo completo de Models não é implementado.

A revisão atual altera o Detector remoto; não implementa Analisador, Inventory ou
Operations. A validação real anterior abaixo não homologa esta revisão.

## Perfil de entrega após extração

| Decisão | Campos Google | Motivo |
| --- | --- | --- |
| Manter | id, name, mimeType, parents, trashed, version, createdTime, modifiedTime, size, md5Checksum, sha256Checksum | Identidade e metadados observados para interpretação posterior pelo Analisador |
| Sob demanda | permissões, capabilities, atalhos, labels, descrição, links de apresentação, proprietários, demais hashes | Não necessários à captura; não solicitados pelo detector |
| Descartar | tokens, resource keys, sessões, conteúdo, pessoas e campos desconhecidos | Não pertencem ao snapshot de observação |

RemoteFile guarda somente esse perfil. Na entrega, ProviderFileData usa JSON UTF-8, formato `google.drive.detector-file`, versão 1, allowlist dos campos acima e limite de 64 KiB. Campos ausentes continuam NotCollected. A captura persiste o item JSON original; esse perfil é montado somente na extração para entrega/diagnóstico. A fila admite payload remoto com validação integral e usa o codec de registros versão 4 no esquema SQLite versão 6.

## Progresso e falhas

A instância é isolada por conta, sincronização e raiz remota. A captura inicial
obtém o cursor antes de percorrer a árvore paginada da raiz configurada. Cada
item recebido produz `Observed`, sem consultar histórico, comparar metadados ou
classificar criação, edição, renomeação, movimentação ou exclusão.

Depois da captura, Changes fornece alterações da conta, inclusive de recursos
fora da raiz inicial. O Detector não consulta ancestrais nem decide pertencimento:
repassa identidade, metadados e sinais recebidos para interpretação posterior
pelo Analisador com o Inventory. Não existe índice `known` no Detector.
`File.event.remote` preserva origem Capture/Changes, removed, time, changeType e
driveId opcionais. Removed sem File mantém a identidade em File.scope e data=None.
Ausência de metadados não é substituída por um objeto fictício. Página com ID
inválido ou identidades contraditórias falha sem avanço do cursor.

O checkpoint versão 4 contém somente cursor, época/sequência, aviso técnico e
no máximo uma página preparada. Essa página contém registros originais ou avisos técnicos, próximo
cursor e um delta de percurso (até 200 pastas descobertas, pasta processada e
token de continuação). Não existe Progress nem clonagem de fila/conjunto global.

O percurso fica nas tabelas privadas remote_scan e remote_scan_control do
Storage (SQLite 6). Cada pasta tem uma linha; o índice (done,seq) permite ler
somente a cabeça da fila. A atualização é incremental, com deduplicação de
pastas por identidade e contadores mantidos por delta. Linhas concluídas ficam
em disco para impedir ciclos; não são carregadas como coleção nem contêm
metadados de arquivos. Recovery limpa esse trabalho técnico explicitamente e
inicia nova captura; não apaga eventos nem conclui ausência/exclusão.

O checkpoint mantém o limite de 16 MiB; o percurso admite até 100.000 diretórios
por captura. A contagem/integridade é verificada na abertura; páginas normais
não contam nem percorrem toda a fila. O reset de recuperação é uma operação
sobre a captura inteira, não um custo repetido em cada página.

Uma página preparada é persistida antes da emissão individual; falhas entre
emissão e checkpoint reutilizam os mesmos objetos, IDs e horários. O avanço do
cursor acontece após aceite durável dos eventos e independe do ack externo.
A confirmação final do cursor e do delta de percurso é uma única transação
IMMEDIATE com compare-and-swap. A aceitação dos eventos precede essa transação;
a preparação durável permite reentrega idempotente se houver interrupção.
Não se promete uma única transação abrangendo todos os enqueues da página.
Recovery reinicia a captura com aviso de cobertura incompleta, sem inferir
exclusão por ausência. Falhas de coleta continuam produzindo avisos técnicos;
não são classificações de ações sobre arquivos. Só stop encerra polling;
pause suspende entrega e mantém captura e persistência.

O scheduler pertence à engine. O diagnóstico expõe início, duração e próxima consulta, contadores e identificador sanitizado do cursor. A UI apenas apresenta esses valores. A validação completa do Orquestrador conjunto continua uma etapa posterior; Developer usa consumo terminal explicitamente diagnóstico.

## Fontes e reaproveitamento

Foram revisados `archive/sync-engine-before-rebuild:engine/src/remote/changes.rs` e o contrato de coleta antigo. Foram reaproveitadas as decisões de cursor anterior à leitura, paginação completa, validação de identidade e distinção entre removed e exclusão. O transporte antigo e sua associação com Inventory não foram copiados: todas as operações usam o Provider atual e sessões renovadas por Accounts.

Referências de protocolo: [Changes](https://developers.google.com/workspace/drive/api/guides/manage-changes), [changes.list](https://developers.google.com/workspace/drive/api/reference/rest/v3/changes/list) e [File](https://developers.google.com/workspace/drive/api/reference/rest/v3/files). A coleção entrega snapshots atuais; ações transitórias entre consultas podem ser agregadas pelo Google.

## Validação histórica em 13/11/2026 — anterior à revisão de observações

A conta autorizada `rtosta.coc@gmail.com` foi verificada pelo plugin. Foi criada a pasta exclusiva `DriveSync Remote Detector Validation 2026-11-13` (`19rfb-LZVjPJc99BMg2QsMvtKGASGSQ2J`). Um fixture temporário de integração usou apenas uma cópia privada dos metadados dessa conta e autenticação gerida por Accounts, dentro do SDK com rede/Secret Service. O detector observou a pasta enquanto o plugin criou `remote-child`, renomeou para `remote-child-renamed` e moveu para `destination`. A engine registrou Created, Renamed e Moved com identidade remota correlacionada; o teste passou. A primeira tentativa sem rede no SDK exibiu RemoteUnavailable; a execução com rede passou. A renovação gerida por Accounts rotacionou a referência de credenciais da cópia; ao encerrar, a referência original da conta de teste foi restaurada e verificada no Secret Service, a entrada adicional foi removida e os metadados temporários foram apagados. Nenhuma outra conta foi alterada. A pasta de teste foi mantida para inspeção.

O teste real não executa mutações pela engine, não confirma sincronização nem valida todos os casos de permissão/expiração do Google real. Cenários de falha, cancelamento, replay, ack e isolamento são exercitados separadamente com portas falsas e SQLite real. O relatório local sanitizado fica em `_build/remote-live-validation/report.json`; o fixture temporário fica no mesmo diretório e não é um teste automatizado versionado. Para repetir uma integração real, usar perfil autorizado independentemente: copiar metadados de Accounts não isola as referências do Secret Service e exige restaurá-las após eventual rotação. Não versionar perfis, credenciais ou artefatos de execução.


## Registro e separação por provedor

A seleção agora ocorre em `remote/mod.rs`, a partir de Accounts, por uma factory registrada. `interfaces.rs` fornece `RemoteObserver` e `RemoteBackend`; `runtime.rs` depende somente dessas interfaces e dos Models comuns. O especialista Google foi organizado em `google/mod.rs`, `api.rs`, `metadata.rs` e `tests.rs`. A projeção de eventos para diagnóstico é comum aos provedores. O backend usa a identidade do provedor sem alterar o namespace Google já persistido.

A validação da seleção usa perfis sintéticos próprios, incluindo conta em reautorização, sem copiar perfis reais ou consultar o chaveiro. Um observador falso independente do Google exercita o scheduler real, e um teste rejeita payload de outro provedor. Provedor desconhecido e conta ausente são rejeitados antes da criação de armazenamento. Esta refatoração não repetiu mutações no Drive real.

## Validação da revisão de observações em 14/11/2026 — anterior à issue 94

Executados no org.gnome.Sdk//50 com a extensão rust-stable do manifesto:
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`,
`cargo test --workspace --offline`, `cargo doc --workspace --no-deps --document-private-items --offline`
e `cargo test --workspace --doc --offline`. A suíte integrada passou com 667
execuções aprovadas e 8 ignoradas (incluindo 6 doctests aprovados). Os ignorados
exigem display/perfis isolados, portais ou Secret Service.

Um provedor sintético entrega 80.000 arquivos por páginas de 200, sem reter o
conjunto no fixture. O backend de contagem verifica 80.001 observações incluindo
a raiz, checkpoint preparado menor que 1 MiB nessa carga e checkpoint final
menor que 512 bytes, sem catálogo ou controle de percurso residual. Esses
limites são asserções dessa fixture, não metas gerais nem perfil de RSS/heap.

SQLite real verifica retomada após evento aceito e falha antes do commit do
cursor, com ID/payload/horário preservados sem duplicação; sinal removed sem
metadados com identidade e horário preservados após reabertura, consumo e ack;
e rejeição do esquema anterior sem migração. As suítes existentes verificam
filas mistas, leases, pausa, cancelamento e isolamento. Não houve consulta ou
mutação no Google real, teste gráfico ou validação do aplicativo instalado.

## Validação da issue 94 em 14/11/2026

Executados no org.gnome.Sdk//50 com rust-stable do manifesto:
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --offline -- -D warnings`,
`cargo test --workspace --offline`, `cargo doc --workspace --no-deps --document-private-items --offline`
e `cargo test --workspace --doc --offline`. A suíte integrada passou com 669
execuções aprovadas (incluindo 6 doctests) e 8 testes de ambiente ignorados.

SQLite real: fila com 20.000 pastas altera somente duas linhas de percurso ao
concluir uma pasta e descobrir outra; EXPLAIN confirma o índice da cabeça. Uma
falha injetada no checkpoint faz rollback do delta e mantém o trabalho anterior.
O limite de 100.000 diretórios falha sem alterar cursor ou fila. Reabertura
verifica counters, CAS, continuação de página e isolamento entre contas/provedores.
Contadores inconsistentes falham sem reparo ou sobrescrita automática.

O produtor testa captura ampla com 198 pastas pendentes e checkpoint confirmado
menor que 512 bytes, duplicatas/ciclos sem repetição do percurso e sem supressão
das observações. Permanece o teste de 80.000 arquivos por páginas de 200, sem
catálogo no checkpoint. Falhas antes e depois do commit final são exercitadas
com SQLite real e reabertura, preservando o evento integral sem repetição de
consulta inicial. Cancelamento após aceite mantém a página preparada e produz
replay idempotente. Paginação, erros do provedor e recuperação continuam cobertos.

Não foram executados Google real, aplicativo instalado, fixture gráfica ou
perfil de heap/RSS. Os limites medidos de checkpoint pertencem às fixtures,
não representam metas gerais de memória do processo. O protocolo não mantém
transação SQLite aberta durante consultas de rede nem durante enqueues separados.

## JSON original antes da extração

O Provider oferece files_get_json/files_list_json/changes_list_json, preservando
os valores JSON recebidos antes da conversão tipada. A seleção de campos das
requisições permanece a mesma; não se pede o catálogo completo de atributos.
Cada item de files.list/get ou changes.list é guardado individualmente com ID da
ocorrência, conta, provedor, sincronização, ID remoto, origem e horário de coleta.
O original inclui campos desconhecidos, nulls explícitos e valores aninhados.
Não se promete identidade byte a byte de espaços, ordem das chaves ou escapes do
HTTP; o conteúdo JSON é preservado sem normalização de metadados.

O checkpoint preparado conserva esses mesmos registros originais. Não contém
RemoteFile montado durante captura. O Storage grava o original e seu digest;
a leitura para claim/histórico invoca o extrator Google para construir File e
RemoteFile. A adaptação do diagnóstico em memória ocorre após o commit; falha
não desfaz o registro ou fabrica metadados. O aviso de disponibilidade ainda é
emitido se a projeção de diagnóstico não puder ser construída.

Um original não adaptável permanece pendente: claim falha explicitamente sem
reservar, descartar ou marcar como consumido. Ack/release conferem identidade e
integridade do registro sem reescrever/adaptar o original. Alterações em campos
que o extrator ignora também participam do digest e da detecção de conflito.

O caminho local não muda de responsabilidade. O contrato público continua
entregando FileBatch de File comuns. Nenhum Analisador/Inventory é implementado.
Esquema SQLite 6, codec de registros 4 e checkpoint Google 4 recusam versões
anteriores sem migração, conversão ou exclusão automática.

Validação desta revisão em 14/11/2026: fmt --all --check, clippy --workspace
--all-targets --offline -- -D warnings, test --workspace --offline,
doc --workspace --no-deps --document-private-items --offline e test --workspace
--doc --offline passaram no org.gnome.Sdk//50 com rust-stable. A suíte integrada
teve 674 execuções aprovadas (incluindo 6 doctests) e 8 testes de ambiente ignorados.

Os novos testes HTTP preservam JSON desconhecido/null nas três leituras; testes
SQLite verificam original intacto após reabertura, claim/release/ack e replay,
conflito por mudança de campo ignorado pelo extrator, isolamento de conta e
recusa de tamanho sem truncamento. Metadados não adaptáveis são persistidos e
notificados depois do commit, mas a entrega falha sem reservar ou descartar.
O teste de página preparada confirma que original e horário/ID sobrevivem ao
retry. As suítes existentes verificam percurso incremental e falhas antes/depois
do commit do cursor. Não houve acesso ao Google real, teste gráfico, aplicativo
instalado ou perfil de memória.
