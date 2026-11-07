# Configuração — responsabilidades e persistência global (provisório)

## Objetivo

O módulo `configuration/` é responsável por **definir, consultar e alterar as preferências da aplicação e as políticas de sincronização**, sem conhecer a implementação do inventário de arquivos. **Temporariamente**, este módulo também é o responsável técnico por avaliar e organizar a **persistência global da aplicação**, até que a arquitetura de dados e a responsabilidade dos demais módulos sejam detalhadas.

Essa atribuição **não significa** que `configuration/` seja dono dos conceitos de conta e conexão. O módulo `accounts/` continua responsável pelo cadastro e ciclo de vida das contas. A responsabilidade de criar/validar pares entre pasta local e remota deve ser definida com o componente correspondente; não atribuí-la a `configuration/` apenas porque um banco pode armazená-los.

## Separação entre tipos de dados

| Informação | Proprietário conceitual | Forma de persistência a avaliar |
| --- | --- | --- |
| Preferências globais: comportamento, notificações, limites e opções da UI | `configuration/` | **GSettings/GLib**, preferencialmente, mediante validação no projeto |
| Preferências por sincronização: limites e políticas para um par de pastas | `configuration/` | GSettings (se o esquema couber) ou outra representação estruturada |
| Identidade e cadastro de contas: `account_id`, `provider_user_id`, nome e e-mail | `accounts/` | Dados globais persistentes, possivelmente SQLite ou arquivo estruturado |
| Conexões: conta + raiz remota + raiz local + estado de ativação | Componente responsável por conexões, ainda a formalizar | Dados globais persistentes, possivelmente SQLite ou arquivo estruturado |
| Credenciais, tokens OAuth e outros segredos | `accounts/auth/` | **Secret Service/keyring**, não SQLite nem GSettings |
| Arquivos, baseline, operações, cursor, eventos e histórico por item | `inventory/` | **SQLite local por pasta sincronizada**, em `.drivesync/inventory.sqlite3` |
| Logs técnicos e estado transitório | Componente produtor / infraestrutura apropriada | `XDG_STATE_HOME`, cache ou logging, conforme a natureza |

**Regra:** proprietário do **dado** não precisa ser o mesmo proprietário da **tecnologia de persistência**. Se houver um armazenamento global compartilhado, `configuration/` poderá inicialmente oferecer a infraestrutura, enquanto `accounts/` e os demais módulos oferecem as operações e regras de domínio, sem expor SQL.

## GTK4, GNOME e Wayland: qual padrão usar?

- **GSettings** é a API de configurações de aplicações da GLib, normalmente com dconf no GNOME. É candidato para valores de preferências com chaves e tipos definidos em um esquema `.gschema.xml`: [GNOME GSettings](https://wiki.gnome.org/HowDoI/GSettings).
- **`GtkSettings` não é um sistema de preferências da aplicação**. Refere-se às configurações compartilhadas de apresentação da plataforma/desktop: [GTK4 GtkSettings](https://docs.gtk.org/gtk4/class.Settings.html).
- **Wayland não determina como os dados do aplicativo são persistidos**; afeta a interação com o sistema gráfico e portais. Não escolher SQLite ou JSON por causa do compositor.
- **XDG Base Directory** separa dados persistentes (`XDG_DATA_HOME`), configurações (`XDG_CONFIG_HOME`), estado (`XDG_STATE_HOME`) e cache (`XDG_CACHE_HOME`). A GLib fornece acesso a esses caminhos.
- JSON, TOML ou arquivos keyfile não são proibidos, mas requerem serialização, escrita atômica, validação e estratégia de evolução. Evitar introduzir um banco apenas para um pequeno conjunto de opções.

## Situação do `app.sqlite3`

**Ainda não há decisão definitiva de criar `app.sqlite3`.** O esquema proposto anteriormente com `accounts`, `sync_connections`, `app_preferences` e `sync_preferences` é apenas uma hipótese e não deve orientar a implementação automaticamente.

Antes de escolher armazenamento, responder:

1. Quais entidades globais precisam persistir independentemente das pastas e do processo?
2. Quais são preferências escalares adequadas para GSettings e quais são dados relacionais?
3. Qual módulo é proprietário do cadastro e das operações de conta?
4. Qual módulo é proprietário das conexões entre conta, pasta remota e pasta local?
5. Essas entidades precisam de consultas, índices, relações e transações que justifiquem SQLite?
6. Como preservar os vínculos e reabrir inventários quando uma pasta está temporariamente indisponível?

**Decisão provisória:** somente o inventário por pasta tem SQLite confirmado; a persistência global será estudada sob `configuration/`, sem impor banco, formato ou tabelas globais nesta etapa.

## Fronteiras e continuidade

- `configuration/` não deve consultar diretamente tabelas de `inventory.sqlite3`; o Inventário expõe as consultas relevantes.
- `inventory/` não armazena preferências nem abre bancos globais.
- `accounts/` é responsável pelas regras de negócio das contas, independentemente de como seus dados sejam persistidos.
- A separação entre configuração, cadastro, conexão e persistência técnica será revisada antes de criar `INTERFACES.md` e testes de persistência global.
- Não criar esquema, adicionar dependências nem modificar código existente nesta etapa documental.
