# Configurações — preferências e políticas do DriveSync

## Objetivo

O módulo `configuration/` define, valida, consulta e atualiza **preferências da aplicação** e **políticas por sincronização**. Apresenta essas operações por uma API pública: seus consumidores **não conhecem nem acessam** arquivos de configuração, schemas, bancos ou chaves internas de armazenamento.

**Cada módulo é proprietário de seus dados e pode persistir o que precisar**, com SQLite, GSettings, arquivos ou outro mecanismo, desde que a persistência fique privada atrás de sua API. Não existe obrigação de reunir tudo em um único banco global.

## Dados de configuração

| Categoria | Exemplos | Proprietário |
| --- | --- | --- |
| Preferências globais | Notificações, comportamento da interface, políticas padrão e limites gerais | `configuration/` |
| Preferências por sincronização | Limites de transferência, opções aplicáveis à conexão e políticas selecionadas | `configuration/`, usando `sync_id` fornecido pelo proprietário das conexões |
| Dados de identidade da conta | `account_id`, `provider_user_id`, nome, e-mail | `accounts/` |
| Cadastro de conexões | `sync_id`, `account_id`, raízes local/remota | Responsabilidade ainda a decidir; levantamento preservado em [`accounts/README.md`](../accounts/README.md) |
| Autenticação e segredos | Tokens OAuth e outros materiais sensíveis | `accounts/`, internamente via Secret Service/keyring; protocolo externo em `providers/<provedor>/` |
| Estado de arquivos e histórico por item | `SyncItem`, baseline, eventos, fila, atividades | `inventory/`, em SQLite por pasta |

Esta separação trata **quem é dono do dado**, não impõe que módulos tenham necessariamente arquivos físicos diferentes. Um eventual mecanismo compartilhado exige contrato explícito; não autoriza um módulo a acessar tabelas de outro.

## Convenções GNOME/GTK4 para preferências

- **GSettings (GLib)** é o candidato principal para preferências tipadas e com chaves pré-definidas, geralmente armazenadas pelo backend dconf no GNOME. Ver [documentação da GLib](https://docs.gtk.org/gio/class.Settings.html).
- `GtkSettings` trata opções de apresentação do GTK, não o cadastro de configurações próprias do aplicativo.
- **Wayland não define o mecanismo de persistência** de preferências ou de dados do aplicativo.
- Os diretórios XDG separam configurações (`XDG_CONFIG_HOME`), dados persistentes (`XDG_DATA_HOME`), estado (`XDG_STATE_HOME`) e cache (`XDG_CACHE_HOME`).
- Se as configurações forem dinâmicas, complexas ou relacionais, avaliar outro mecanismo próprio. Arquivos JSON/TOML/keyfile ou SQLite não são proibidos; a escolha depende dos dados, consultas, necessidade de transações e testes.
- **Não criar `app.sqlite3` automaticamente** apenas porque a aplicação tem contas e preferências.

## Persistência privativa e testabilidade

1. Definir primeiro os contratos públicos de leitura, atualização, validação e notificação de mudanças.
2. Escolher a persistência mais apropriada após classificar as opções e os seus tipos.
3. Encapsular completamente a persistência; nenhum consumidor recebe conexão SQLite nem precisa saber onde fica o arquivo.
4. Se houver dados por sincronização, a configuração utiliza `sync_id` e as APIs do módulo proprietário da conexão, sem consultar seu banco diretamente.
5. Testar os contratos com armazenamento isolado/simulado e, quando aplicável, testar a persistência real do próprio módulo.

**Situação atual:** `configuration/` não é o módulo central de todos os bancos; suas decisões de persistência são locais. O inventário tem SQLite por pasta definido. O armazenamento de contas e conexões será definido nos módulos responsáveis.

## Próximos passos

- Enumerar as preferências globais e por sincronização.
- Verificar quais chaves se adaptam a GSettings e quais requerem outro armazenamento.
- Especificar APIs e cenários TDD sem expor detalhes da implementação.

## Proteção e recuperação dos bancos dos módulos

`configuration/` coordena **a política transversal de backup e recuperação** de armazenamentos persistentes, sem assumir propriedade dos dados: cada módulo continua responsável pelo próprio banco, esquema, conexões, integridade e restauração física via seu Storage. Configurations seleciona e valida a política de cópias, retenção e fallback e recebe solicitações tipadas; não executa SQL sobre bancos alheios.

**Fluxo obrigatório diante de corrupção confirmada:**
1. O Storage identifica corrupção real, suspende o uso do banco afetado e emite **mensagem de erro ao usuário** pelo mecanismo normal de notificações.
2. O Storage solicita a Configurations **recuperação automática por backup**. O Storage fecha/isola conexões e WAL, restaura cópia consistente compatível, valida integridade e identidade, e reabre.
3. Se a restauração falhar ou não houver backup válido, emitir **nova mensagem ao usuário** e **reconstruir uma estrutura de banco vazia**, preservando a base danificada e backups para eventual diagnóstico; nunca presumir dados históricos que foram perdidos.
4. O módulo proprietário retoma sua API habitual; a coordenação da inicialização recebe uma indicação `Normal`, `Restored` ou `Rebuilt`, suficiente para exigir observações novas quando necessário. Os algoritmos do Scanner, Detectores, Reconciliador e Analisador não mudam.
5. Se nem a reconstrução física for possível, informar a falha e **não executar sincronização sem armazenamento confiável**.

**Distinções obrigatórias:** `SQLITE_BUSY`, disco cheio, problemas de permissão, dispositivo indisponível, esquema incompatível e identidade divergente **não são corrupção** e não autorizam reconstrução automática. Backups devem ser consistentes (para SQLite, Online Backup API ou equivalente), íntegros, vinculados à identidade correta, rotacionados sem perder a última cópia válida e protegidos contra cópias parciais. WAL/rollback de transação **não são histórico permanente**.

**Após recuperação:** backup antigo pode ter baseline e checkpoints defasados; banco reconstruído não possui baseline confiável. Nem o Scanner nem o Analisador fazem tratamento especial de corrupção: trabalham com os estados disponíveis, e a ausência de informação é `Unknown`, nunca exclusão ou permissão para sobrescrever. Observações locais e remotas precisam ser refeitas/validadas antes de decisões destrutivas; decisões sem evidência suficiente são encaminhadas ao usuário **por item**. Perda de fila ou cursor dos Detectores exige nova cobertura de observação, não assumir que fila vazia significa que não houve alterações.

**Estado:** política documental; integração e interface de notificação ainda não implementadas.
