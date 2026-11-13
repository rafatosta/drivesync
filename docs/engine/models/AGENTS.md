# Agentes — Models (seleção local e catálogo remoto)

## Situação e fonte de verdade

**Documentação conceitual; NÃO implementar código Rust nesta etapa.**

As definições de atributos, tipos, variantes, ausência e invariantes estão em [INTERFACES.md](INTERFACES.md). O [README.md](README.md) apresenta responsabilidades e uma tabela resumida; [TESTS.md](TESTS.md) traduz essas regras em casos verificáveis. A implementação futura deverá ser **reflexo literal da especificação após aprovação do responsável**, sem acrescentar ou alterar campos por preferência do agente.

**Esta etapa define documentalmente o fluxo local; não implementar Rust nesta demanda.** RemoteFile e ProviderFileData permanecem propostas sem seleção. A execução futura não pode tratar a revisão local como aprovação do catálogo remoto.

## Proprietários

- `models/`: `LocalFile`, `RemoteFile`, `FileData`, `File` e auxiliares explicitamente definidos.
- `detectors/`: `FileBatch`, identificação de lote, lease, fila e confirmação. Models apenas explica a relação, não redefine o lote.
- `providers/google/api/`: `GoogleFile` e protocolo Google; não importar `GoogleFile` em Models.
- `inventory/`: propriedade atual de `SyncItem` até revisão específica. **Não introduzir nem migrar `SyncItem` nesta fase.**

## Restrições literais

1. `FileData` tem somente `Local(LocalFile)` e `Remote(RemoteFile)`. Não inventar `Both`, `Google`, `Unknown` ou lógica operacional.
2. `File` encapsula os **sete campos propostos**: `id`, `sync_id`, `event`, `observed_at`, `coverage`, `scope`, `data`; não misturar com `SyncItem`.
3. `File.id` identifica **a ocorrência**, não o arquivo físico; replay da mesma ocorrência preserva ID e payload.
4. `LocalFile` contém somente o perfil selecionado em INTERFACES § 2.3 e seus estados de coleta. Não restaurar o catálogo completo como campos ou blobs. `None` não significa zero, ausência física ou comando de exclusão.
5. `FieldValue::Unknown` e `FieldValue::Known([])` são semanticamente diferentes; não simplificar ambos para lista vazia.
6. `ProviderFileData` contém somente metadados opacos, nunca tokens, credenciais, respostas completas não filtradas ou conteúdo do arquivo.
7. Não construir I/O de arquivo, rede, SQLite, fila, autenticação, operações, detector ou analisador dentro de Models.
8. Não criar `ModelManager`, `Repository` ou trait operacional para justificar abstrações desnecessárias.
9. Não duplicar tipos já existentes (`AccountId`, `SyncId`, `LocalFileId` etc.). Propor migração/reexportação sob revisão explícita.
10. Não definir limite/codificação de `ProviderFileData.payload` enquanto estiver pendente; bloquear implementação afetada, em vez de adivinhar.
11. A construção de `File` deve respeitar as relações entre `event`, `scope`, `data` e `coverage` definidas no contrato; não fabricar dados para contornar validação.
12. Não alterar `detectors/`, `detectors/storage/`, `inventory/` ou `operations/` para fazer compilar um exemplo conceitual de Models.

## Contrato local selecionado

Seguir a matriz manter/sob demanda/descartar de INTERFACES § 2.2 e os tipos de § 2.3. A seleção desta revisão cobre os atributos locais e suas regras de ausência; não é implementação Rust nem seleção remota.

- Manter paths Unix em bytes não vazios, kind, mode, owner, mtime/ctime/btime, size/nlink/inode/device, par de flags statx/máscara e alvo literal de symlink.
- LocalFileId é opcional; o coletor inicial usa None. Não inventar ID com inode, device ou caminho.
- Não adicionar atime, hash, conteúdo, ACL, xattrs, rdev, montagem/subvolume, Direct I/O, AtomicWrite ou extra_metadata ao envelope local desta versão. Consultas sob demanda exigem contrato próprio posterior; não implementar suas estruturas antecipadamente.
- Distinguir NotApplicable de Unsupported e NotCollected. No fluxo local, NotApplicable só é permitido no alvo quando o tipo conhecido não é symlink.
- Complete refere-se ao perfil básico, com btime e flags opcionais. Models verifica coerência; a cobertura física é responsabilidade do coletor/percurso. Não declarar snapshot transacional.
- Preservar nanossegundos e segundos assinados; ctime não é criação. Timestamps não estabelecem precedência entre versões.
- A raiz é WholeSync sem dados/localizações; não aceitar LocalRelativePath vazio, `.` ou `/`. `.drivesync` é lexicalmente válido, mas filtrado pelo observador.
- Validadores puros são somente os especificados em INTERFACES §§ 2.8 e 5.6. Não usar validação para gerar identidade, completar metadados, resolver links ou abrir arquivos.
- Reutilizar AccountId, ProviderId e Timestamp existentes. Não migrar identidades ou SyncItem sem escopo explícito.

## Revisões ainda necessárias

A lista local foi selecionada e justificada documentalmente. Alterações futuras devem atualizar matriz, contrato e testes antes da implementação correspondente. Uma solicitação de implementar os modelos locais não autoriza o catálogo remoto.

O envelope FileData depende de RemoteFile. Antes de implementar o envelope completo, concluir a revisão remota ou obter decisão explícita sobre essa fronteira; não criar placeholders remotos, payload livre ou variante especial local para contornar o contrato. É possível implementar e validar isoladamente os tipos locais aprovados sem tocar nos campos remotos.

## Isolamento obrigatório do Detector frente ao esquema

**Aceite arquitetural:** adicionar, remover ou renomear campos de `LocalFile` não pode exigir edição da lógica do Detector Local. O Detector consome uma **interface de coleta estável**, não a representação campo a campo do modelo.

- Não construir `LocalFile` com struct literal no Detector nem usar os campos como condição para entregar uma notificação.
- O coletor/adaptador específico de metadados locais faz a construção e pode usar funções auxiliares independentes para `statx` e leitura literal de symlinks; novas fontes exigem contrato próprio antes de coleta.
- O coletor executa I/O; `models/` **não** executa I/O. O coletor pode mudar quando campos mudam, mas o Detector deve permanecer intacto.
- O contrato de retorno deve preservar `LocalFile` integralmente, sem reduzir o modelo a um subconjunto manual.
- Acrescentar testes de regressão que verifiquem evolução do esquema com o Detector não modificado.
- Não confundir **interface de coleta** (interna ao detector local) com interface operacional pública de `models/`; Models continua apenas dados.

## Contrato conceitual dos eventos em File

`File.event` é um objeto `FileEvent` com `kind: FileEventKind`, `previous: Option<FileLocation>` e `current: Option<FileLocation>`. São oito tipos de evento documentados em [INTERFACES.md](INTERFACES.md#5-file--observação-autocontida-e-atributos-do-evento): criação, edição, exclusão suspeita, renomeação, movimentação, mudança de metadados, reavaliação e indisponibilidade.

- `FileLocation` representa caminho local ou localização remota (conta/ID, nome e pai quando disponíveis), não dois snapshots completos de metadados.
- `Deleted` pode ocorrer sem `FileData`; nunca tratar um evento de remoção ou `data=None` como autorização de excluir arquivos.
- Renomeação/movimentação requer correlação confiável entre anterior/atual. Sem evidência, usar `RecheckRequired`.
- Orquestrador apenas transporta `File`; **Analisador** interpreta origem e evento.
- Nenhuma alteração deste contrato autoriza acoplar o Detector aos campos internos de `LocalFile`; manter a porta estável do coletor.
- O desenho completo de `FileBatch` e do protocolo de ack é propriedade do Detector. Não migrar `SyncItem`.
- Nesta demanda, apenas revisar contratos e testes. Código será uma etapa separada; a implementação do envelope completo depende também da revisão remota.

## Metadados completos e associação de RemoteFile ao provedor

**Não implementar imediatamente o catálogo completo de `RemoteFile`.** A seção 3 de [INTERFACES.md](INTERFACES.md#3-remotefile--metadados-remotos-e-associação-ao-provedor) documenta primeiro todos os campos comuns e especializados disponíveis e a associação ao objeto de origem.

- `RemoteFile.reference` qualifica o ID real com `AccountId`; `RemoteFile.provider` e `ProviderFileData.provider` precisam corresponder à conta. Nunca associar por nome/caminho ou inventar IDs ausentes.
- O **adaptador específico** conhece `GoogleFile` e transforma esse objeto em `RemoteFile`; `models/` jamais importa `GoogleFile`, HTTP ou sessão.
- Metadados comuns são normalizados **somente com semântica equivalente**; dados específicos/complexos ficam em `ProviderFileData` formatado/versionado, sem perder informações observadas que forem aprovadas para retenção.
- `MetadataValue<T>` distingue campo efetivamente observado de não solicitado, não suportado ou negado. Uma resposta parcial da API não autoriza preencher campos com zero/false/vazio.
- **Alterações nos atributos de `RemoteFile` não podem exigir alterações no Detector Remoto** (polling, cursor, scanner, sinalização e fila). O Detector recebe objeto completo do adaptador por uma porta estável.
- Rotinas auxiliares do especialista podem buscar outros campos/métodos quando necessário; `RemoteFile` continua sem I/O.
- `GoogleChange.removed` pode indicar perda de acesso. Nenhum registro remoto sem metadados confiáveis ou ID válido autoriza exclusão.
- Evitar payload bruto e material de segurança/criptografia, chaves, tokens e dados pessoais sem política autorizada de sanitização.
- **Antes de implementar**, classificar campos em `manter / sob demanda / descartar`, considerando utilidade, custo de API, escopos, privacidade, tamanho, persistência e formato do snapshot.
- Não implementar `SyncItem` nem fazer migração do Inventário nesta etapa.

## Execução futura

Após aprovação: validar tipo a tipo, criar testes unitários puros RED → GREEN → REFACTOR conforme [TESTS](TESTS.md), executar Cargo/Rust apenas no Flatpak do projeto e relatar PASS/FAIL/BLOCKED. Integração e testes de lote/ack pertencem ao Detector, não aos testes de Models.

Para regras não especificadas, registrar como pendência e **solicitar revisão do contrato antes de codificar**. Não converter comentários de exemplos anteriores em requisitos não aprovados.

[MODULE_STANDARD](../MODULE_STANDARD.md) · [AGENTS raiz](../../../AGENTS.md).
