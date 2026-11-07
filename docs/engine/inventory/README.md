# Inventário — InventoryManager

## Objetivo

O **Inventário** é inicialmente o componente responsável pelo **acesso e pela persistência dos dados estruturados do DriveSync**, tanto os dados globais da aplicação quanto os dados de cada pasta sincronizada. Todos os demais módulos usam suas interfaces: **nenhum consumidor executa SQL diretamente**.

O Inventário registra identidades, metadados observados, vínculos, último estado confirmado, operações pendentes, cursores e atividades. Ele **não** observa o filesystem ou o Google Drive por conta própria, nem executa upload/download ou escolhe qual versão deve prevalecer em um conflito.

## Bancos de dados: quantos são e onde ficam?

O modelo prevê **dois tipos de banco SQLite**: **um global da aplicação** e **um para cada pasta sincronizada**. O histórico é uma tabela do próprio inventário, não um banco à parte.

| Banco | Quantidade | Localização | Finalidade |
| --- | --- | --- | --- |
| **Aplicação — `app.sqlite3`** | **1 por instalação/perfil** | `$XDG_DATA_HOME/drivesync/app.sqlite3` | Contas cadastradas, pares de pastas e preferências |
| **Inventário — `inventory.sqlite3`** | **1 por pasta sincronizada** | `<pasta-local>/.drivesync/inventory.sqlite3` | Arquivos, vínculos, baseline, fila, eventos, cursores **e atividades históricas** |

**Exemplo:** com três pastas sincronizadas, haverá um banco global e três inventários: **quatro arquivos SQLite** ao todo.

```text
$XDG_DATA_HOME/drivesync/
└── app.sqlite3                         # Aplicação inteira

<Pasta A>/
├── .drivesync/
│   └── inventory.sqlite3               # Arquivos, processamento e histórico da Pasta A
└── arquivos-do-usuario/

<Pasta B>/
└── .drivesync/
    └── inventory.sqlite3               # Independente da Pasta A
```

### Por que a pasta oculta `.drivesync/` existe?

É o diretório de **controle daquela sincronização**: acompanha a própria pasta, registra a última situação conhecida e permite retomar o trabalho após reiniciar a aplicação. Não é um diretório do usuário para enviar ao Google.

- **Nunca enviar** `.drivesync/` ao Google Drive; ignorá-lo no detector e nas operações.
- O banco acompanha **somente aquela pasta**, não compartilha dados de outra sincronização ou conta.
- A ausência/inacessibilidade da pasta local **suspende** a sincronização: não equivale a exclusão remota.
- Se não existe inventário válido e a pasta contém arquivos de usuário, **bloquear a sincronização inicial automática**. Não mesclar, sobrescrever nem excluir sem processo específico de associação.
- A pasta `.drivesync/` não conta como conteúdo do usuário ao verificar se uma pasta nova está vazia.

**Credenciais OAuth não ficam em nenhum desses bancos.** Tokens e segredos pertencem ao Secret Service/keyring. Logs técnicos gerais também não são atividades do histórico de uma pasta.

## 1. Banco da aplicação (`app.sqlite3`)

O banco global existe independentemente de as pastas estarem acessíveis. Guarda o **cadastro de contas**, os **pares de sincronização** e as **preferências**. Não contém cópias de arquivos ou histórico detalhado de operações por arquivo.

| Tabela | Identidade | Dados principais |
| --- | --- | --- |
| `accounts` | `account_id` (PK) | `provider`, `provider_user_id` (Google OIDC `sub`), `email`, `display_name`, datas |
| `sync_connections` | `sync_id` (PK) | `account_id` (FK), `remote_root_id`, `local_root`, `enabled`, nome e data de criação |
| `app_preferences` | `key` (PK) | Tipo e valor escalar de uma preferência global |
| `sync_preferences` | `(sync_id, key)` (PK) | Tipo e valor escalar de uma preferência de uma pasta |

**Relações:** uma conta pode ter várias conexões; cada conexão associa exatamente uma conta a uma raiz remota e uma raiz local. Preferências específicas referenciam a conexão. Um e-mail **não** substitui a identidade estável `provider_user_id`.

**Regras:** não guardar árvores de arquivos, filas, tokens ou eventos remotos no banco global. A identidade da pasta deve impedir conexões indevidamente sobrepostas. No Flatpak, o XDG é resolvido dentro do ambiente da aplicação.

### Diagrama do banco da aplicação

```mermaid
erDiagram
    ACCOUNTS ||--o{ SYNC_CONNECTIONS : possui
    SYNC_CONNECTIONS ||--o{ SYNC_PREFERENCES : configura
    ACCOUNTS {
        TEXT account_id PK
        TEXT provider
        TEXT provider_user_id UK
        TEXT email
        TEXT display_name
        TEXT created_at
        TEXT updated_at
    }
    SYNC_CONNECTIONS {
        TEXT sync_id PK
        TEXT account_id FK
        TEXT remote_root_id
        TEXT local_root UK
        BOOLEAN enabled
        TEXT display_name
    }
    SYNC_PREFERENCES {
        TEXT sync_id PK,FK
        TEXT key PK
        TEXT value_type
        TEXT value_text
        INTEGER value_integer
        REAL value_real
    }
    APP_PREFERENCES {
        TEXT key PK
        TEXT value_type
        TEXT value_text
        INTEGER value_integer
        REAL value_real
    }
```

**Como funciona:** o usuário cadastra uma conta em `accounts`; cada pasta sincronizada é uma linha em `sync_connections`, vinculada à conta. Configurações que afetam todas as contas ficam em `app_preferences`; configurações de uma pasta específica ficam em `sync_preferences`, vinculadas a `sync_id`. `app_preferences` não exige FK porque suas preferências pertencem à aplicação inteira.

### Atributos e restrições do banco global

| Tabela | Campos e regras |
| --- | --- |
| `accounts` | `account_id TEXT PRIMARY KEY`; `provider TEXT NOT NULL`; `provider_user_id TEXT NOT NULL` com `UNIQUE(provider, provider_user_id)`; `email TEXT?`; `display_name TEXT?`; `created_at` e `updated_at` obrigatórios. **Tokens não entram nessa tabela.** |
| `sync_connections` | `sync_id TEXT PRIMARY KEY`; `account_id TEXT NOT NULL` com FK para `accounts`; `remote_root_id TEXT NOT NULL`; `local_root TEXT NOT NULL` e exclusivo; `enabled BOOLEAN NOT NULL`; `display_name TEXT?`; `created_at` obrigatório. Não eliminar conta com vínculos ativos por cascata silenciosa. |
| `app_preferences` | `key TEXT PRIMARY KEY`; `value_type` obrigatório (`text`, `integer`, `real`, `boolean`); exatamente uma coluna escalar tipada recebe o valor. Sem JSON de preferências agrupadas. |
| `sync_preferences` | PK composta `(sync_id, key)`; FK de `sync_id` para `sync_connections`; os mesmos tipos escalares de `app_preferences`. Só substitui a preferência global quando houver configuração específica. |

**Exemplo de consulta:** listar contas com suas conexões (`accounts` → `sync_connections`) e preferências por conexão (`sync_preferences`) sem abrir nenhum banco dentro de `.drivesync/`. Informações de progresso e arquivos dependem do inventário da conexão.

## 2. Banco da pasta sincronizada (`inventory.sqlite3`)

Este banco representa os arquivos e o processamento **de um único par de pastas**.

### Tabelas e relacionamentos

| Tabela | Chave | Conteúdo |
| --- | --- | --- |
| `sync_info` | `singleton=1` | `sync_id`, conta, raiz remota, geração, conclusão da sincronização inicial |
| `local_files` | `local_file_id` | Nome/caminho local, tipo, tamanho, horários, dispositivo, inode, checksum |
| `google_files` | `google_file_id` | `remote_id` exclusivo, nome, MIME, versão, tamanho, datas, hashes, lixeira e drive |
| `google_file_parents` | `(google_file_id, parent_remote_id)` | Uma linha por relacionamento pai do arquivo no Drive |
| `sync_items` | `item_id` | Vínculo com `local_file_id` e `google_file_id`, presença, status, revisão e horários |
| `sync_baselines` | `item_id` | Estado anterior **confirmado**: presença, nomes, pais, versões, hashes e instante |
| `sync_operations` | `operation_id` | Fila persistente de ações, estado, tipo, direção, tentativas e falhas |
| `remote_cursor` | `corpus` | Cursor de alterações remoto e geração correspondente |
| `remote_events` | `event_id` | Eventos de alterações remotas recebidos, pendentes ou reconhecidos |
| `activity` | `activity_id` | Histórico cronológico de operações e acontecimentos relevantes, com referências ao item e à operação quando existentes |

### Diagrama do banco de cada sincronização

```mermaid
erDiagram
    SYNC_ITEMS o|--o| LOCAL_FILES : "local_file_id"
    SYNC_ITEMS o|--o| GOOGLE_FILES : "google_file_id"
    GOOGLE_FILES ||--o{ GOOGLE_FILE_PARENTS : "possui pais"
    SYNC_ITEMS ||--o| SYNC_BASELINES : "ultimo confirmado"
    SYNC_ITEMS ||--o{ SYNC_OPERATIONS : "fila por item"
    SYNC_ITEMS o|--o{ REMOTE_EVENTS : "evento associado"
    REMOTE_CURSOR ||--o{ REMOTE_EVENTS : "origem do evento"
    SYNC_ITEMS o|--o{ ACTIVITY : "historico do item"
    SYNC_OPERATIONS o|--o{ ACTIVITY : "resultado da operacao"

    SYNC_INFO {
        INTEGER singleton PK
        TEXT sync_id
        TEXT account_id
        TEXT remote_root_id
        INTEGER generation
    }
    LOCAL_FILES {
        TEXT local_file_id PK
        BLOB name
        INTEGER size
        INTEGER modified_ns
        INTEGER inode
    }
    GOOGLE_FILES {
        TEXT google_file_id PK
        TEXT remote_id UK
        TEXT name
        TEXT mime_type
        TEXT version
    }
    GOOGLE_FILE_PARENTS {
        TEXT google_file_id PK,FK
        TEXT parent_remote_id PK
    }
    SYNC_ITEMS {
        TEXT item_id PK
        TEXT local_file_id FK,UK
        TEXT google_file_id FK,UK
        TEXT local_presence
        TEXT remote_presence
        TEXT sync_status
        INTEGER revision
    }
    SYNC_BASELINES {
        TEXT item_id PK,FK
        TEXT confirmed_at
        TEXT remote_version
        TEXT local_checksum
    }
    SYNC_OPERATIONS {
        INTEGER operation_id PK
        TEXT item_id FK
        TEXT operation_kind
        TEXT direction
        TEXT state
    }
    REMOTE_CURSOR {
        TEXT corpus PK
        TEXT cursor
        INTEGER generation
    }
    REMOTE_EVENTS {
        INTEGER event_id PK
        TEXT corpus FK
        TEXT item_id FK
        TEXT event_kind
        TEXT processed_at
    }
    ACTIVITY {
        INTEGER activity_id PK
        TEXT item_id FK
        INTEGER operation_id FK
        TEXT event_type
        TEXT origin
        TEXT outcome
        TEXT occurred_at
    }
```

As relações `sync_items.local_file_id` e `sync_items.google_file_id` são **FKs opcionais e únicas**. Assim, um item lógico pode existir só no local, só no Google, nos dois lados ou ficar temporariamente sem representação atual. A falta da referência **não prova exclusão**.

### `LocalFile`, `GoogleFile` e `SyncItem`

| Objeto Rust | Dono do contrato | Papel no Inventário |
| --- | --- | --- |
| `LocalFile` | Componente responsável pelos metadados locais (a definir integralmente) | Produz/representa uma observação local |
| `GoogleFile` | `google_api/` | Produz/representa uma observação remota |
| `SyncItem` | `inventory/` | Reúne as duas representações, baseline e situação de sincronização |

```rust
// Contrato conceitual, não assinatura definitiva.
pub struct SyncItem {
    pub id: SyncItemId,
    pub local: Option<LocalFile>,
    pub remote: Option<GoogleFile>,
    pub baseline: Option<SyncBaseline>,
    pub local_presence: PresenceState,
    pub remote_presence: PresenceState,
    pub status: SyncStatus,
}
```

O Inventário constrói o `SyncItem` a partir das tabelas relacionadas (por exemplo, com `LEFT JOIN`). A aplicação não precisa conhecer o esquema para consultar um arquivo.

Os atributos de `LocalFile` ainda poderão evoluir. Os de `GoogleFile` são orientados pelo contrato da Google API. **As tabelas guardam os atributos necessários à persistência**, não necessariamente uma cópia integral de todos os objetos externos. Campos opcionais não recebidos não se convertem automaticamente em zero, `false` ou exclusão.

### Presença, estado e baseline

| Campo/estado | Valores | Significado |
| --- | --- | --- |
| `local_presence`, `remote_presence` | `unknown`, `present`, `absent_confirmed` | Desconhecido, presença observada ou ausência efetivamente comprovada |
| `sync_status` | `pending_reconciliation`, `pending_upload`, `pending_download`, `synced`, `conflict`, `error` | Condição registrada para o Analisador/Orquestrador |
| `revision` | Inteiro positivo | Versão para evitar atualizações concorrentes obsoletas |
| `SyncBaseline` | Registro opcional por item | Último estado **efetivamente confirmado** em ambos os lados |

Uma observação atualiza metadados locais ou remotos **sem avançar o baseline**. Apenas a confirmação de uma operação concluída e verificada altera o baseline.

O baseline deve reter ao menos presença, nome, pai/localização, versão/assinatura e instante de confirmação. Ele é separado das tabelas de estado observado para evitar sobrescrever o passado ao detectar mudanças atuais.

### Fila de operações, cursores e eventos

| Estrutura | Campos/estados principais | Regra |
| --- | --- | --- |
| `sync_operations` | `item_id`, `direction` (upload/download), `operation_kind` (criar, conteúdo, renomear, mover, lixeira, excluir, restaurar), `state` (pendente, executando, retry, bloqueada, concluída, falha), `attempt_count`, `next_attempt_at`, precondição de versão | Registra trabalho durável, inclusive offline; não executa a ação |
| `remote_cursor` | `corpus`, `cursor`, `generation` | Representa até onde as alterações remotas foram observadas |
| `remote_events` | `corpus`, `remote_id`, `item_id?`, tipo, versão, `observed_at`, `processed_at?` | Evento só fica reconhecido após reavaliação explícita |

- Publicar o cursor de uma página e seus eventos deve ser **transacional** no inventário.
- Avançar o cursor não significa que os eventos foram processados.
- Depois de interrupção, recuperar a fila e os eventos pendentes e **reavaliar antes de repetir** escritas com resultado desconhecido.
- O Inventário persiste as operações, mas a **decisão de executá-las** pertence aos módulos de coordenação e análise.

## 3. Histórico como parte de `inventory.sqlite3`

**Histórico não é outro banco.** É a tabela `activity` dentro do mesmo `inventory.sqlite3` de cada pasta. Registra **o que aconteceu e qual foi o resultado**, enquanto `sync_items` representa o estado atual conhecido e `sync_operations` a fila operacional.

| Componente de dados | Pergunta respondida | Exemplo |
| --- | --- | --- |
| `sync_items` e `sync_baselines` | Como está o arquivo agora? Qual foi seu último estado sincronizado? | `relatorio.pdf` está pendente de upload |
| `sync_operations` | O que precisa ser executado, está em execução ou deve ser retomado? | Atualizar conteúdo remoto após reconexão |
| `remote_events` | Quais sinais de mudança remota ainda exigem reavaliação? | Google sinalizou alteração no arquivo X |
| `activity` | O que foi feito ou tentado, quando, por quem/qual origem e com qual resultado? | Upload concluído às 15:32; tentativa anterior falhou |

### Quando registrar uma atividade

| Situação | Histórico? | Por quê? |
| --- | --- | --- |
| Upload/download concluído | Sim | Usuário pode consultar o que foi sincronizado |
| Renomeação, movimentação, criação, lixeira ou exclusão confirmadas | Sim | Registrar ação efetiva, direção e resultado |
| Operação que falhou ou foi interrompida | Sim, para falhas relevantes | Explicar a tentativa e permitir diagnóstico posterior |
| Conflito identificado ou resolvido | Sim | Registrar a ocorrência/resolução, sem criar tabela de conflitos |
| Suspensão/retomada significativa da sincronização | Sim, como atividade de sistema | Explicar mudanças de funcionamento |
| Evento remoto recebido ainda não processado | **Não automaticamente** | É sinal técnico na tabela `remote_events`, não ação concluída |
| Releitura repetitiva sem alteração | Não | Evitar ruído e crescimento do histórico |
| Progresso de cada bloco transferido | Não | Progresso é estado de transferência, não acontecimento histórico |

A camada de análise/orquestração determina **quando o evento merece registro**; o Inventário valida e persiste a atividade. O histórico **não** determina automaticamente a política ou o resultado da sincronização.

### Campos da tabela `activity`

| Campo | Tipo lógico | Obrigatório? | Significado |
| --- | --- | --- | --- |
| `activity_id` | Inteiro / PK | Sim | Identificador próprio da atividade |
| `item_id` | FK → `sync_items.item_id` | Não | Arquivo associado; ausente em evento geral da sincronização |
| `operation_id` | FK → `sync_operations.operation_id` | Não | Operação que originou o registro, quando aplicável |
| `event_type` | Enum textual | Sim | `upload`, `download`, `rename`, `move`, `delete`, `conflict`, `sync_paused` etc. |
| `origin` | Enum textual | Sim | `local`, `remote` ou `system` |
| `outcome` | Enum textual | Sim | `completed`, `failed`, `cancelled`, `blocked` ou `informational` |
| `occurred_at` | Timestamp | Sim | Instante registrado do acontecimento |
| `description` | Texto | Não | Resumo legível e sanitizado |
| `error_kind` | Texto/enum | Não | Classificação segura da falha, sem token nem dados secretos |
| `item_name` | Texto | Não | Nome conhecido naquele momento, para histórico legível após mudanças |
| `item_remote_id` | Texto | Não | ID remoto conhecido naquele momento, como evidência histórica |
| `item_local_path` | BLOB | Não | Caminho local da época, se necessário preservar bytes Unix |

**Imutabilidade:** uma atividade registrada não muda quando o arquivo é renomeado, movido ou removido posteriormente. Campos de referência podem ser opcionais e protegidos por política de retenção. A descrição não guarda payloads nem dados pessoais desnecessários.

### Integridade, retenção e transações

- `item_id` e `operation_id` podem ser FKs reais porque agora estão **no mesmo banco**. Para preservar atividades após remover o registro referenciado, usar `ON DELETE SET NULL` ou manter tombstones; **nunca `ON DELETE CASCADE` do item para o histórico**.
- Confirmar o resultado de uma operação, atualizar baseline, marcar a fila como concluída e inserir a atividade deve, quando for um mesmo resultado lógico, ocorrer na **mesma transação SQLite**.
- Um retry falho pode gerar atividade de falha, seguido de outra atividade de conclusão; são acontecimentos distintos, associados à mesma operação.
- Consultas do histórico devem ser paginadas e filtráveis por `item_id`, período, ação e resultado. Índices recomendados: `(occurred_at, activity_id)`, `(item_id, activity_id)` e `(operation_id, activity_id)`.
- Retenção e limpeza são políticas explícitas e independentes da limpeza de operações; não apagar registros silenciosamente durante uma sincronização.
- O histórico **não** substitui logs técnicos estruturados: stack traces, detalhes HTTP e diagnósticos operacionais detalhados pertencem ao sistema de logs, não à timeline do usuário.

### Exemplo de fluxo

1. O detector identifica alteração de `notas.txt`. O Inventário atualiza `local_files`; o baseline **não** muda.
2. O Analisador autoriza upload; `sync_operations` recebe uma ação pendente.
3. O operador executa a ação e devolve resultado verificável.
4. Em transação, o Inventário atualiza os metadados confirmados, avança `sync_baselines`, marca a operação como concluída e insere em `activity`: **upload, origem local, concluído, horário**.
5. A interface pode consultar a atividade sem depender da fila ou dos nomes atuais do arquivo.

## 4. Regras relacionais e segurança

| Regra | Exigência |
| --- | --- |
| Persistência | Dados estruturados em linhas e colunas; **sem JSON/BLOB para serializar structs, listas, mapas ou filas** |
| Exceção BLOB | Somente valor binário individual, como bytes de nome Unix não UTF-8 |
| Integridade | FKs dentro do mesmo banco, **inclusive histórico**, índices, `UNIQUE`, `CHECK`, transações e `PRAGMA foreign_keys=ON` |
| Isolamento | `sync_info` deve corresponder ao `sync_id`, à conta e à raiz remota cadastradas |
| Versionamento | `PRAGMA user_version` valida a versão física; não implementar compatibilidade legada por padrão |
| Segurança | Tokens, client secrets e links de sessão confidenciais não são persistidos nesses bancos |
| Escalabilidade | Alterar um item não exige carregar ou regravar a árvore inteira |
| Exclusões | `unknown` não pode ser convertido em `absent_confirmed` por falha de leitura, rede ou acesso |
| Execução | Nenhum SQL exposto aos consumidores; nenhum upload/download/exclusão física pelo Inventário |
| Consulta | Associações e itens paginados; nomes/caminhos não substituem a identidade lógica |

## 5. Capacidades oferecidas pelo Inventário

| Interface conceitual | Resultado |
| --- | --- |
| Abrir/validar banco global | Contas, conexões e preferências disponíveis |
| Cadastrar/consultar conta e conexão | Identidades estáveis e par de raízes |
| Ler/gravar preferências globais e por sincronização | Valores escalares tipados |
| Abrir/validar inventário de uma pasta | Escopo, esquema e integridade verificados |
| Buscar `SyncItem` por ID, ID remoto ou identidade local | Objeto unificado ou ausência |
| Listar itens por status, presença ou hierarquia | Resultados paginados |
| Registrar `LocalFile` / `GoogleFile` | Atualização de um lado sem mudar o baseline |
| Associar registros | Chaves e unicidade preservadas |
| Confirmar baseline | Estado confirmado com transação e revisão |
| Inserir/consultar/reconhecer operação pendente | Fila durável |
| Atualizar cursor e registrar eventos | Escrita consistente |
| Consultar/registrar atividades | Tabela `activity` do mesmo inventário, com filtros e paginação |

A especificação de assinaturas Rust, parâmetros, retornos e erros pertence ao futuro `INTERFACES.md`; os cenários TDD pertencerão ao futuro `TESTS.md`. **O README é a única referência do modelo de bancos e das tabelas do Inventário.**
