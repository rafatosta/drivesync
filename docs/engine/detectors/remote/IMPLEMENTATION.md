# Perfil remoto implementado nesta etapa

A solicitação de implementar o Detector Remoto inclui a seleção do modelo necessária à observação e à fila. O catálogo completo de Models não é implementado.

## Seleção de metadados

| Decisão | Campos Google | Motivo |
| --- | --- | --- |
| Manter | id, name, mimeType, parents, trashed, version, createdTime, modifiedTime, size, md5Checksum, sha256Checksum | Identidade qualificada, pertencimento à árvore e classificação conservadora |
| Sob demanda | permissões, capabilities, atalhos, labels, descrição, links de apresentação, proprietários, demais hashes | Não necessários à captura; não solicitados pelo detector |
| Descartar | tokens, resource keys, sessões, conteúdo, pessoas e campos desconhecidos | Não pertencem ao snapshot de observação |

RemoteFile guarda somente esse perfil. ProviderFileData usa JSON UTF-8, formato `google.drive.detector-file`, versão 1, allowlist dos campos acima e limite de 64 KiB. Campos ausentes continuam NotCollected. Não transportar GoogleFile bruto. A fila admite payload remoto com validação integral e conserva seus registros locais versão 1.

## Progresso e falhas

A instância é isolada por conta, sincronização e raiz remota. A leitura inicial captura o cursor antes de percorrer a árvore paginada. Alterações posteriores são obtidas por Changes, verificando pertencimento pela cadeia de pais. Uma página preparada é persistida antes da emissão individual; falhas entre emissão e checkpoint reusam os mesmos objetos e IDs. Avanço de checkpoint não confirma sincronização. Limites finitos de páginas e árvore produzem cobertura parcial/erro, nunca sucesso completo fictício.

Initial/Recovery observam a árvore sem inferir exclusão por ausência. Removed, perda de acesso e cursor expirado exigem reavaliação; nunca exclusão confirmada. Renomeação/movimento precisam do snapshot anterior da mesma identidade. Sem evidência suficiente, emitir Recheck. Só stop encerra polling; pause suspende entrega e mantém captura e persistência.

O scheduler pertence à engine. O diagnóstico expõe início, duração e próxima consulta, contadores e identificador sanitizado do cursor. A UI apenas apresenta esses valores. A validação completa do Orquestrador conjunto continua uma etapa posterior; Developer usa consumo terminal explicitamente diagnóstico.

## Fontes e reaproveitamento

Foram revisados `archive/sync-engine-before-rebuild:engine/src/remote/changes.rs` e o contrato de coleta antigo. Foram reaproveitadas as decisões de cursor anterior à leitura, paginação completa, validação de identidade e distinção entre removed e exclusão. O transporte antigo e sua associação com Inventory não foram copiados: todas as operações usam o Provider atual e sessões renovadas por Accounts.

Referências de protocolo: [Changes](https://developers.google.com/workspace/drive/api/guides/manage-changes), [changes.list](https://developers.google.com/workspace/drive/api/reference/rest/v3/changes/list) e [File](https://developers.google.com/workspace/drive/api/reference/rest/v3/files). A coleção entrega snapshots atuais; ações transitórias entre consultas podem ser agregadas pelo Google.

## Validação real em 13/11/2026

A conta autorizada `rtosta.coc@gmail.com` foi verificada pelo plugin. Foi criada a pasta exclusiva `DriveSync Remote Detector Validation 2026-11-13` (`19rfb-LZVjPJc99BMg2QsMvtKGASGSQ2J`). Um fixture temporário de integração usou apenas uma cópia privada dos metadados dessa conta e autenticação gerida por Accounts, dentro do SDK com rede/Secret Service. O detector observou a pasta enquanto o plugin criou `remote-child`, renomeou para `remote-child-renamed` e moveu para `destination`. A engine registrou Created, Renamed e Moved com identidade remota correlacionada; o teste passou. A primeira tentativa sem rede no SDK exibiu RemoteUnavailable; a execução com rede passou. A renovação gerida por Accounts rotacionou a referência de credenciais da cópia; ao encerrar, a referência original da conta de teste foi restaurada e verificada no Secret Service, a entrada adicional foi removida e os metadados temporários foram apagados. Nenhuma outra conta foi alterada. A pasta de teste foi mantida para inspeção.

O teste real não executa mutações pela engine, não confirma sincronização nem valida todos os casos de permissão/expiração do Google real. Cenários de falha, cancelamento, replay, ack e isolamento são exercitados separadamente com portas falsas e SQLite real. O relatório local sanitizado fica em `_build/remote-live-validation/report.json`; o fixture temporário fica no mesmo diretório e não é um teste automatizado versionado. Para repetir uma integração real, usar perfil autorizado independentemente: copiar metadados de Accounts não isola as referências do Secret Service e exige restaurá-las após eventual rotação. Não versionar perfis, credenciais ou artefatos de execução.


## Registro e separação por provedor

A seleção agora ocorre em `remote/mod.rs`, a partir de Accounts, por uma factory registrada. `interfaces.rs` fornece `RemoteObserver` e `RemoteBackend`; `runtime.rs` depende somente dessas interfaces e dos Models comuns. O especialista Google foi organizado em `google/mod.rs`, `api.rs`, `metadata.rs`, `timestamp.rs` e `tests.rs`. A projeção de eventos para diagnóstico é comum aos provedores. O backend usa a identidade do provedor sem alterar o namespace Google já persistido.

A validação da seleção usa perfis sintéticos próprios, incluindo conta em reautorização, sem copiar perfis reais ou consultar o chaveiro. Um observador falso independente do Google exercita o scheduler real, e um teste rejeita payload de outro provedor. Provedor desconhecido e conta ausente são rejeitados antes da criação de armazenamento. Esta refatoração não repetiu mutações no Drive real.
