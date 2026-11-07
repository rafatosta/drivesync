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
| Autenticação e segredos | Tokens OAuth e outros materiais sensíveis | `accounts/auth/`, via Secret Service/keyring |
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
