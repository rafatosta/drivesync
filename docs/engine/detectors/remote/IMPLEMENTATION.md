# Perfil remoto implementado nesta etapa

A solicitação de implementar o Detector Remoto inclui a seleção do modelo necessária à observação e à fila. O catálogo completo de Models não é implementado.

A revisão atual altera o Detector remoto; não implementa Analisador, Inventory ou
Operations. A validação real anterior abaixo não homologa esta revisão.

## Seleção de metadados

| Decisão | Campos Google | Motivo |
| --- | --- | --- |
| Manter | id, name, mimeType, parents, trashed, version, createdTime, modifiedTime, size, md5Checksum, sha256Checksum | Identidade e metadados observados para interpretação posterior pelo Analisador |
| Sob demanda | permissões, capabilities, atalhos, labels, descrição, links de apresentação, proprietários, demais hashes | Não necessários à captura; não solicitados pelo detector |
| Descartar | tokens, resource keys, sessões, conteúdo, pessoas e campos desconhecidos | Não pertencem ao snapshot de observação |

RemoteFile guarda somente esse perfil. ProviderFileData usa JSON UTF-8, formato `google.drive.detector-file`, versão 1, allowlist dos campos acima e limite de 64 KiB. Campos ausentes continuam NotCollected. Não transportar GoogleFile bruto. A fila admite payload remoto com validação integral e usa o codec File versão 2 no esquema SQLite versão 3.

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

O checkpoint versão 2 contém cursor, fila e conjunto de controle do percurso
inicial, época/sequência e no máximo uma página pendente de eventos completos.
O conjunto de diretórios visitados é liberado ao terminar a captura. Não contém
catálogo nem snapshots anteriores dos arquivos. Durante o percurso, fila e
conjunto de diretórios ainda são clonados/serializados como estado técnico;
esta revisão não implementa DAOs relacionais para esse trabalho pendente.
O limite de checkpoint permanece 16 MiB e o percurso admite até 100.000
diretórios marcados/trabalhos pendentes. Falhas são explícitas.

Uma página preparada é persistida antes da emissão individual; falhas entre
emissão e checkpoint reutilizam os mesmos objetos, IDs e horários. O avanço do
cursor acontece após aceite durável dos eventos e independe do ack externo.
Não se afirma uma única transação de página: o protocolo durável de preparação
e replay continua vigente e é testado após reabertura de SQLite real.
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

## Validação da revisão em 14/11/2026

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
