# Instruções para agentes — Inventário (DriveSync)

## Escopo e referências

Implementar e testar **exclusivamente** o componente `inventory/` da engine Rust, que encapsula o inventário de **uma pasta sincronizada por instância**. Seu armazenamento privado contém observações locais/remotas, vínculos `SyncItem`, baseline confirmado, fila durável de operações e histórico por arquivo. Cursores/eventos remotos estão em revisão de propriedade frente ao Detector.

**Ler nesta ordem, antes de modificar código:**

| Documento | Responsabilidade |
| --- | --- |
| [INTERFACES.md](INTERFACES.md) | Contratos públicos, objetos de requisição/resultado, campos obrigatórios/opcionais, erros, transições e limites |
| [README.md](README.md) | Modelo relacional, identidade dos itens, persistência, isolamento e regras de segurança |
| [TESTS.md](TESTS.md) | Casos TDD por método, fixtures, integridade, concorrência, rollback, falhas e critérios de aceite |
| [AGENTS.md da raiz](../../../AGENTS.md) | Regras gerais do repositório: escopo, commits, idioma, documentação e ambiente Flatpak |
| [Google API — INTERFACES](../providers/google/api/INTERFACES.md) | Contrato externo de `RemoteFile`; não recriar uma representação incompatível |

**Ordem de precedência:** o contrato da interface define o comportamento público; o README define o modelo de dados; TESTS.md descreve como demonstrar ambos. Se encontrarem uma contradição real, **corrigir a documentação de forma explícita antes de implementar**, sem escolher silenciosamente um comportamento e sem alterar contratos por conveniência.

## Princípio: dados privados, API pública

- **O Inventário é dono de seu banco e de sua API**; outros módulos não recebem conexão SQLite, SQL, transações, tabelas, cursores de banco nem detalhes de armazenamento.
- Cada método público recebe **um objeto de requisição** e retorna `InventoryResult<ObjetoDeResultado>`. Manter exatamente as assinaturas e os tipos de [INTERFACES.md](INTERFACES.md); não substituir objetos coesos por muitos parâmetros posicionais.
- Construir requests por construtores, `Default` ou builders quando apropriado, para reduzir impacto de novos campos opcionais. **Não presumir** que adicionar campo a `pub struct` mantém compatibilidade de literais Rust.
- `LocalFile`, `RemoteFile`, `File`, `FileData`, `MetadataValue<T>`, `RemoteFileRef` e identidades compartilhadas pertencem a **Models**. Não importar `RemoteFile` nem redefinir os tipos em Inventário.
- O Inventário **persiste fatos e estados**. Não observa o filesystem/Google por conta própria, não analisa qual versão vence um conflito, não organiza a política de sincronização e não executa uploads, downloads, renomeações ou exclusões físicas.
- Contas globais, conexões entre pastas, configurações, segredos OAuth e autenticação pertencem a outros módulos. **Não criar nem administrar `app.sqlite3`** dentro do Inventário.

## Persistência e isolamento por sincronização

**Layout definido:**

```text
<sync-local-root>/
└── .drivesync/
    └── inventory.sqlite3
```

- Exatamente **um banco SQLite por pasta sincronizada**. O histórico está em `activity` **no mesmo banco**; não criar `history.sqlite3`.
- Abrir e validar `sync_info` contra o `SyncIdentity` solicitado (`sync_id`, `account_id`, `remote_root_id`). Nunca reaproveitar inventário pertencente a outra conta ou conexão.
- Tratar `.drivesync/` como **metadados reservados**: nunca listar como conteúdo do usuário, enviar ao Google nem interpretar alterações internas como arquivos para sincronizar.
- Raiz local indisponível, banco inacessível, faltando, corrompido ou com esquema incompatível: **falhar/suspender com erro explícito**, sem recriar, excluir, substituir, mesclar ou enviar dados silenciosamente.
- Se a pasta contém arquivos do usuário e não existe inventário válido, não inicializar sincronização automática: requer fluxo seguro de associação definido pelo componente responsável.
- Habilitar e verificar `PRAGMA foreign_keys=ON` em **todas** as conexões. Usar chaves, `UNIQUE`, `CHECK`, índices, transações, revisão otimista e validação de `PRAGMA user_version` conforme o README.
- Persistir campos estruturados em colunas/tabelas relacionais. **Não** serializar `SyncItem`, structs, filas, mapas ou listas em JSON/BLOB opaco. BLOB apenas para dado binário individual, como bytes de nome Unix não UTF-8.
- Não implementar migração ou compatibilidade legada por padrão; isso não autoriza perder dados do usuário.

## Invariantes obrigatórias do modelo

### Identidade, observações e baseline

1. `SyncItemId` é uma identidade lógica estável, independente de caminho, nome, inode ou ordem de leitura. Um item pode ter representação apenas local, apenas remota, ambas ou nenhuma representação atual quando houver tombstone.
2. `LocalFile` e `RemoteFile` são observações. `Option::None` e campos ausentes **não comprovam exclusão** e não viram zero, `false`, texto vazio nem dados inventados.
3. `PresenceState::Unknown`, `Present` e `AbsentConfirmed` são estados distintos. Somente uma observação com escopo e evidência suficientes pode registrar ausência confirmada.
4. Atualizações de observações **nunca avançam** `SyncBaseline`. O baseline existe apenas após estado comprovadamente sincronizado, e pode estar ausente antes da primeira confirmação.
5. Renomeação e movimentação preservam a identidade do item; pais locais e remotos podem divergir enquanto há mudanças pendentes. IDs da Google API não devem ser substituídos por chaves internas SQLite.
6. Atualizações parciais usam `FieldUpdate::Keep/Set/Clear` quando previsto; `Clear` é explícito e validado. `Option<T>` ausente não pode virar comando implícito de limpeza.
7. O Inventário registra `SyncStatus` decidido pelo consumidor autorizado; não converte falha de rede em conflito nem decide políticas de resolução.

### Operações, eventos e histórico

1. `sync_operations` é fila **persistente**, inclusive offline. Inserir ou reivindicar uma operação **não a executa**. Uma operação `Running` após reinício exige reavaliação antes de repetir efeito remoto com resultado desconhecido.
2. `claim_operation` deve impedir dois workers de obterem a mesma operação. Revisões e transições de estado têm precondições e erros explícitos.
3. A conclusão de operação comprovada usa `confirm_sync`, não atualização direta de `Completed`. Atualizar baseline, status, operação e atividade relevante **na mesma transação SQLite**, com rollback completo em falha.
4. O Inventário **não** publica cursores nem eventos do Detector. Checkpoint é responsabilidade do Detector Remoto; o Detector Storage recebe e entrega `File`. O Inventário registra apenas o estado derivado após análise.
5. Cada `SyncActivity` tem `item_id` **obrigatório**, referenciando um `SyncItem` do mesmo banco. `operation_id` é opcional. Eventos globais não entram na tabela `activity`.
6. Preservar atividades após renomeação, movimentação ou remoção do arquivo. Manter tombstones quando há histórico referenciando item; não usar cascata que o apague.
7. Retry, redelivery de evento e resposta perdida devem respeitar idempotência, deduplicação e verificação de estado. Não duplicar operações/atividades nem avançar baseline por repetição.

## Regra de integração com File e SyncItem

- `SyncItem` é **exclusivamente do Inventário**, com `local: Option<LocalFile>`, `remote: Option<RemoteFile>`, presença, baseline confirmado, status e revisão. `File` é observação transitória fornecida pelo Detector e interpretada pelo Analisador. `File.id` **não** é `SyncItemId`.
- `RecordLocalObservation.observation.observed` recebe `models::File` com origem local; `RecordRemoteObservation.observation.observed` recebe `models::File` com origem remota. Não aceitar variante incorreta nem construir `RemoteFile` diretamente a partir de `GoogleFile`.
- A identidade remota é `RemoteFileRef(account_id, file_id)`, com unicidade por conta; não é ID de linha SQLite, nome ou caminho. `LocalFile.local_id` pode estar ausente; não inventar identidade.
- `File.data=None`, `FileEventKind::Deleted`, `ScopeUnavailable` e falha de API **não comprovam ausência**. `AbsentConfirmed` exige evidência independente suficiente, após análise. Eventos globais sem vínculo seguro não criam `SyncItem` automático.
- `MetadataValue::NotCollected`, `Unsupported`, `Unavailable` e `PermissionDenied` **não** devem zerar campos conhecidos. `Known(false)`, `Known([])` e `Known(0)` são dados observados, não ausências.
- Uma observação **nunca** avança `SyncBaseline`; confirmação verificada de uma operação atualiza baseline, fila, estado e atividade na mesma transação.
- A fila de operações do Inventário é distinta da fila de notificações de `File` do Detector. **Propriedade resolvida:** `EventStore` mantém eventos/leases/ack; `RemoteCheckpoint` pertence ao Detector Remoto. Não recriar `remote_cursor`/`remote_events` em `inventory.sqlite3`.
- Persistência dos grupos de metadados e `ProviderFileData` exige **revisão pré-implementação**: catalogar tudo, escolher `manter/sob demanda/descartar`, definir codec e privacidade. Não prometer round-trip de atributo descartado nem serializar o objeto inteiro como blob opaco inadvertidamente.
- Proposta documental, não alterar código Rust, Detector ou Models apenas para adaptar o Inventário nesta etapa.

## Submódulos internos — contratos de referência

A fachada `InventoryManager` continua com **16 métodos públicos**. Implementar internamente por responsabilidade, usando os contratos documentais propostos de [items](items/INTERFACES.md), [metadata](metadata/INTERFACES.md), [baseline](baseline/INTERFACES.md), [operations](operations/INTERFACES.md), [activity](activity/INTERFACES.md) e [storage](storage/INTERFACES.md). Estes diretórios são **documentação de contratos**, não código Rust já implementado. As assinaturas internas são esboços para revisão, não autorização para mudar a API pública.

- `items/`: identidade, vínculos, presenças, status, revisão, tombstones; não coleta metadados nem resolve conflitos.
- `metadata/`: persistência dos campos aprovados de Models; sem `GoogleFile`, acesso ao filesystem ou rede.
- `baseline/`: exclusivamente último estado verificado; substituição apenas via confirmação transacional.
- `operations/`: fila de trabalho, retry e claim; sem execução de transferências.
- `activity/`: histórico imutável vinculado ao item; sem eventos técnicos do Detector.
- `storage/`: SQLite, abertura segura, transação, integridade e tradução de erros; nenhuma conexão por repositório.
- `InventoryManager` coordena `confirm_sync` em uma transação de `storage/` abrangendo item, baseline, operação e histórico. Nenhum submódulo executa commit independente.
- Nenhuma importação cíclica entre repositórios; compartilhar apenas tipos de contrato e contexto interno de armazenamento. Não criar `remote_cursor` nem `remote_events` por compatibilidade.
- Antes do código, revisar os detalhes **ainda pendentes** de campos persistidos, codec do payload específico do provedor, DDL, idempotência, ownership/lifetimes e prova de ausência.

## Método de trabalho: Contract-First + TDD

| Etapa | Ação | Condição para avançar |
| --- | --- | --- |
| 1. Selecionar capacidade | Identificar método e objetos no `INTERFACES.md` | Campos, opções, erro e efeitos estão claros |
| 2. Resolver lacuna | Conferir `README.md` e pendências em `TESTS.md` | Nenhuma decisão crítica foi inventada a partir do teste |
| 3. RED | Escrever teste público de compilação + integração de comportamento com SQLite real | Falha por funcionalidade ainda não implementada |
| 4. GREEN | Implementar apenas a capacidade necessária | Casos positivos/negativos e persistência passam |
| 5. REFACTOR | Reduzir duplicação sem alterar contratos | Regressão dos grupos anteriores continua passando |
| 6. Registrar entrega | Commit atômico por unidade lógica validada | Reportar testes efetivamente executados e pendências |

**Não implementar os 16 métodos documentados em um lote único sem validação intermediária.** Seguir conjuntos coesos: abertura/identidade; consultas; observações; associações/baseline; fila de operações; histórico. Métodos de eventos ou checkpoint remoto não pertencem ao Inventário. Uma implementação parcial não pode retornar sucesso fictício para métodos pendentes.

## Autonomia e conclusão da implementação

- **“Implemente o módulo Inventário”**, sem delimitação, exige concluir **os 16 métodos públicos e os casos aplicáveis** de [INTERFACES.md](INTERFACES.md) e [TESTS.md](TESTS.md). Não basta criar o esquema SQLite, os tipos ou apenas o primeiro grupo.
- Implementar por unidades coesas, **RED → GREEN → REFACTOR**, com SQLite real e isolado, reabertura, rollback, regressão e **commits locais incrementais**. Avançar para o próximo grupo autonomamente após validar o anterior, sem solicitar aprovação por método.
- Quando a solicitação definir somente abertura, observações, baseline, fila, eventos ou histórico, **limitar o trabalho àquela capacidade** e relatar os demais grupos pendentes.
- **Aceite integral:** todas as interfaces públicas implementadas e verificadas; testes aplicáveis executados no Flatpak com banco SQLite real temporário; invariantes de integridade, concorrência, ausência comprovada e histórico mantidos; relatório de comandos, resultados, falhas e pendências.
- Se definição essencial (por exemplo, seleção de atributos de `LocalFile` ou prova de ausência) estiver incompleta, documentar o impedimento antes de implementar esse comportamento; não inventar contrato, não reescrever teste apenas para passar e não afirmar que o módulo está concluído.
- Respeitar os limites do `AGENTS.md` global: contratos não mudam por conveniência, commits podem ser locais, **`git push`/PR exige autorização específica**. Exemplo reutilizável em [IMPLEMENTATION_REQUEST.md](../IMPLEMENTATION_REQUEST.md).

## Regras concretas dos testes

- **Banco SQLite real, isolado, um diretório temporário por teste.** Um mock do Inventário não demonstra FKs, índices, transações, consistência após fechamento nem durabilidade.
- `RemoteFile` e `LocalFile` sintéticos, nenhum Google Drive real, OAuth, credencial pessoal, serviço externo, arquivo de usuário nem interface gráfica.
- Verificar os **20 contratos públicos** e os cenários identificados em [TESTS.md](TESTS.md): casos positivos, negativos, opcionais, integração do banco, concorrência, rollback, operações offline e recuperação.
- Usar dois handles/threads reais onde necessário para demonstrar a correção da revisão otimista e a reivindicação única de operação.
- Injetar erros determinísticos em transações para provar rollback; uma conexão simplesmente fechada **não equivale** a simular crash de processo. Se necessário, usar subprocesso isolado para testar interrupção abrupta.
- Após escrita, fechar e reabrir o inventário e comparar dados/identidades, baseline, fila, eventos e histórico. Testar separadamente duas raízes e duas identidades de sincronização.
- Testar nomes Unix não UTF-8, Google Workspace sem checksum/tamanho convencional, projeções remotas parciais, observação incompleta e evidência inválida de ausência.
- Verificar `InventoryFailure` tipado e sanitizado; não expor SQL, conteúdos dos arquivos, caminhos pessoais, tokens ou segredos no erro.
- Cobertura percentual **não substitui** os testes de risco. Não declarar PASS nem integração funcional sem execução verificável.

## Pontos ainda abertos — não improvisar contratos

Antes de implementar cenários dependentes, resolver no `INTERFACES.md` a seleção de campos a persistir de `LocalFile`/`RemoteFile`, os critérios de `ObservationProof`, a paginação **de consultas de itens**, idempotência de registros, transições da inicialização e retenção de tombstones. O checkpoint de detecção não pertence a este módulo.

**Regras de segurança já são obrigatórias mesmo antes desses detalhes:** não presumir exclusão; não duplicar operações por erro de replay; não declarar uma transferência concluída sem comprovação; não apagar histórico por limpeza de inventário.

## Ambiente e alterações permitidas

- Toda execução de `cargo`, `rustc`, `rustfmt`, `clippy`, testes e geração de documentação deve ocorrer **somente no ambiente Flatpak** do manifesto `flatpak/com.rtosta.DriveSync.Devel.yml`, como determina [AGENTS.md da raiz](../../../AGENTS.md). Não usar o Rust do host como substituto.
- Não instalar SDKs/extensões automaticamente se o ambiente estiver ausente. Registrar bloqueios e não afirmar verificações não executadas.
- Respeitar organização Rust do repositório (`mod.rs` como ponto de organização pública, modelos relacionados em arquivos coesos), sem impor subdivisões artificiais.
- Comentários de código, rustdoc e mensagens de commit **em inglês**. Preferir documentação por motivo, precondição, resultado e invariantes, não simplesmente repetir assinatura.
- Não refatorar GUI, `providers/google/api/`, `accounts/`, configurações ou Orquestrador para facilitar testes do Inventário. Mudanças de contrato entre módulos exigem revisão explícita de seu proprietário.
- No ambiente local, commits atômicos seguem as regras do `AGENTS.md` da raiz; **não publicar push/PR sem autorização explícita**.

## Entrega por unidade de implementação

Ao concluir uma etapa, informar: **métodos públicos implementados**, **testes escritos antes do código**, **ambiente e comandos realmente executados**, **resultados**, **persistência/rollback revalidados**, **contratos ou dependências ainda pendentes** e eventuais limitações. Se a mudança for apenas documental, identificar explicitamente que **nenhum teste foi executado**.
