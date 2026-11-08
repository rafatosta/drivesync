# Agentes — Models (contratos de dados propostos)

## Situação e fonte de verdade

**Documentação conceitual; NÃO implementar código Rust nesta etapa.**

As definições de atributos, tipos, variantes, ausência e invariantes estão em [INTERFACES.md](INTERFACES.md). O [README.md](README.md) apresenta responsabilidades e uma tabela resumida; [TESTS.md](TESTS.md) traduz essas regras em casos verificáveis. A implementação futura deverá ser **reflexo literal da especificação após aprovação do responsável**, sem acrescentar ou alterar campos por preferência do agente.

**Atributos documentados agora são PROPOSTOS, não aprovados para implementação automaticamente.**

## Proprietários

- `models/`: `LocalFile`, `RemoteFile`, `FileData`, `File` e auxiliares explicitamente definidos.
- `detectors/`: `FileBatch`, identificação de lote, lease, fila e confirmação. Models apenas explica a relação, não redefine o lote.
- `providers/google/api/`: `GoogleFile` e protocolo Google; não importar `GoogleFile` em Models.
- `inventory/`: propriedade atual de `SyncItem` até revisão específica. **Não introduzir nem migrar `SyncItem` nesta fase.**

## Restrições literais

1. `FileData` tem somente `Local(LocalFile)` e `Remote(RemoteFile)`. Não inventar `Both`, `Google`, `Unknown` ou lógica operacional.
2. `File` encapsula os **sete campos propostos**: `id`, `sync_id`, `event`, `observed_at`, `coverage`, `scope`, `data`; não misturar com `SyncItem`.
3. `File.id` identifica **a ocorrência**, não o arquivo físico; replay da mesma ocorrência preserva ID e payload.
4. `LocalFile` registra todos os grupos conhecidos de metadados reais Linux e seus estados de coleta; `RemoteFile` armazena os valores efetivamente observados. `None` não significa zero, ausência física ou comando de exclusão.
5. `FieldValue::Unknown` e `FieldValue::Known([])` são semanticamente diferentes; não simplificar ambos para lista vazia.
6. `ProviderFileData` contém somente metadados opacos, nunca tokens, credenciais, respostas completas não filtradas ou conteúdo do arquivo.
7. Não construir I/O de arquivo, rede, SQLite, fila, autenticação, operações, detector ou analisador dentro de Models.
8. Não criar `ModelManager`, `Repository` ou trait operacional para justificar abstrações desnecessárias.
9. Não duplicar tipos já existentes (`AccountId`, `SyncId`, `LocalFileId` etc.). Propor migração/reexportação sob revisão explícita.
10. Não definir limite/codificação de `ProviderFileData.payload` enquanto estiver pendente; bloquear implementação afetada, em vez de adivinhar.
11. A construção de `File` deve respeitar as relações entre `event`, `scope`, `data` e `coverage` definidas no contrato; não fabricar dados para contornar validação.
12. Não alterar `detectors/`, `detectors/storage/`, `inventory/` ou `operations/` para fazer compilar um exemplo conceitual de Models.

## Metadados completos de LocalFile

`LocalFile` não é mais o registro de seis campos. A proposta formal em [INTERFACES.md, seção 2](INTERFACES.md#2-localfile--metadados-reais-completos-do-linux) determina **todos os campos e subestruturas** de `kind`, `mode`, `owner`, `times`, `storage`, `flags`, `direct_io`, `atomic_write`, `xattrs`, `acl`, `symlink` e `extra_metadata`.

- Usar `MetadataValue<T>` para distinguir valor conhecido, não consultado, não suportado, acesso negado e indisponível.
- Preservar timestamps com nanossegundos; `ctime` não significa criação.
- Preservar nomes e valores binários de xattrs e destino de symlinks, sem conversão com perda.
- Não confundir `stx_attributes` com máscara de suporte, nem `dev` com `rdev`, nem tamanho lógico com espaço alocado.
- Não declarar coleta completa quando houver grupos `NotCollected`. Múltiplas consultas não fornecem um snapshot transacional.
- `LocalFile` não executa `statx`, xattr, ACL ou `readlink`: tais operações pertencem ao Detector Local.
- Metadados sensíveis como ACL, capabilities e `security.*` não autorizam log, transmissão, restauração automática ou upload.
- Hash de conteúdo/MIME inferido não são metadados intrínsecos: **não restaurar o antigo campo `LocalFile.hash`**.
- Não inventar formatos, valores substitutos, limites de leitura, permissões ou política de coleta ainda pendentes.
- **O contrato é conceitual e depende de aprovação antes da implementação Rust.**

## Bloqueio obrigatório antes de implementar LocalFile

**Não iniciar a implementação da estrutura completa descrita atualmente.** Primeiro executar uma **revisão de necessidade de todos os metadados** com o responsável pelo projeto.

O contrato atual é um catálogo deliberadamente abrangente: **levantar todos os dados disponíveis primeiro; identificar e descartar os desnecessários depois**. Criar uma matriz por campo/grupo contendo finalidade, consumidor, disponibilidade/fonte, custo, exposição de dados sensíveis e decisão fundamentada (`manter`, `sob demanda` ou `descartar`). Não eliminar atributos antecipadamente por conveniência do agente e tampouco implementá-los todos só porque foram enumerados.

Somente depois de a seleção ser explicitamente aprovada, consolidar os atributos implementáveis em `INTERFACES.md`, ajustar `TESTS.md` e implementar **literalmente o contrato final aprovado**. Essa etapa de revisão é um **pré-requisito**, não uma sugestão facultativa.

## Isolamento obrigatório do Detector frente ao esquema

**Aceite arquitetural:** adicionar, remover ou renomear campos de `LocalFile` não pode exigir edição da lógica do Detector Local. O Detector consome uma **interface de coleta estável**, não a representação campo a campo do modelo.

- Não construir `LocalFile` com struct literal no Detector nem usar os campos como condição para entregar uma notificação.
- O coletor/adaptador específico de metadados locais faz a construção e pode usar funções auxiliares independentes para `statx`, ACL, xattrs e outras fontes.
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
- Não implementar até a aprovação da especificação e da revisão de metadados anteriores ao código.

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
