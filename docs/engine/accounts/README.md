# Accounts — cadastro de contas e guarda segura de credenciais

## Objetivo

O módulo `accounts/` é responsável por **cadastrar, identificar, listar, consultar, atualizar e remover contas conectadas** no DriveSync. Também mantém o **vínculo da conta com credenciais armazenadas de modo seguro**, sem expor segredos a consumidores.

**Não existe submódulo `accounts/auth/`.** Os mecanismos técnicos de autenticação, renovação de tokens e identificação remota específicos de cada serviço pertencem às APIs documentadas em [Providers](../providers/INTERFACES.md), como [Google OAuth/OIDC](../providers/google/INTERFACES.md). Accounts utiliza os resultados autorizados do Provider para registrar uma conta e vincular credenciais, mas **não implementa OAuth, OpenID Connect, PKCE, endpoints ou HTTP**.

O cadastro de contas e o armazenamento de segredos têm responsabilidades diferentes, embora ambos sejam gerenciados por Accounts. O meio físico de persistência é privado; não é obrigatório usar SQLite no cadastro.

## Dados pertencentes a Accounts

| Campo | Responsabilidade |
| --- | --- |
| `account_id` | Identidade interna estável da conta na aplicação |
| `provider` | Qual serviço fornece essa conta (por exemplo, Google) |
| `provider_user_id` | Identidade externa estável verificada pelo provedor |
| `email` | E-mail opcional, mutável e apenas de apresentação |
| `display_name` | Nome de apresentação opcional |
| `state` | Estado conhecido de disponibilidade da conta/credencial, não teste de conectividade |
| `created_at`, `updated_at` | Histórico mínimo de cadastro e atualização |

A identidade natural para evitar duplicação é `(provider, provider_user_id)`. E-mail **não** identifica a conta de forma estável. Não armazenar `access_token`, `refresh_token` ou outros segredos nesses metadados.

### Relação com conexões de sincronização

Uma conta pode estar associada a várias sincronizações (`sync_id`, raiz local e raiz remota). **O proprietário definitivo do cadastro de conexões ainda não foi definido.** Accounts identifica as contas, mas não assume implicitamente a propriedade desses vínculos. Uma exclusão deve verificar vínculos ativos por contrato explícito e não remover arquivos ou sincronizações por efeito colateral.

## Fronteiras

| Responsabilidade | Proprietário |
| --- | --- |
| Cadastro, identidade local e metadados da conta | `accounts/` |
| Armazenamento seguro e associação de credenciais à conta | `accounts/`, internamente via Secret Service/keyring ou equivalente |
| Protocolo OAuth/OIDC, consentimento, refresh e revogação pela API | `providers/<provedor>/` |
| API externa de arquivos e metadados do serviço | `providers/<provedor>/` |
| Preferências globais e políticas transversais | `configuration/` |
| Estado de arquivos, baseline e fila de operações | `inventory/` e `detectors/storage/`, cada um em seu escopo |
| Cadastro de conexões de sincronização | **Pendente de decisão**, não presumido em Accounts |

O Provider **não é proprietário da persistência local de credenciais**. Sua interface de autenticação pode receber sessões e dados de autorização adequados ao serviço, sem exigir um contrato universal OAuth para todos os provedores. A forma concreta de integração deverá evitar o trânsito desnecessário de tokens brutos entre módulos.

## Ciclo de vida de uma conta

1. Usuário solicita adicionar uma conta e seleciona um provedor.
2. A interface técnica do Provider executa a autorização e retorna identidade remota verificada e resultado autorizado.
3. Accounts verifica a identidade, cadastra/associa a conta e **guarda segredos no armazenamento seguro local**.
4. Consumidores autorizados requisitam uma sessão/referência segura associada a `account_id` para chamar a API específica do Provider.
5. Se houver necessidade de reautorização, a condição é representada no estado da conta; Accounts coordena a atualização do vínculo, e Provider realiza o fluxo externo.
6. Remoção elimina cadastro e credenciais locais somente após checar conexões vinculadas; não implica revogação remota ou remoção de arquivos.

Não presumir transação única entre cadastro e keyring: falhas intermediárias devem ser recuperáveis/idempotentes; nunca anunciar conta conectada sem material de autorização persistido.

## Estado das contas e acompanhamento da execução

A condição conhecida de cada conta permanece em `Account.state` (`Connected`, `ReauthorizationRequired` ou `Unavailable`). **Esse campo não informa atividade em andamento nem comprova conectividade atual.**

As operações de inclusão, remoção, obtenção de sessão e atualização da autorização podem ser assíncronas por dependerem de armazenamento seguro e de contratos técnicos externos. A nova consulta `Accounts::status()` apresenta **somente as operações ativas nesta instância**, sua categoria e fase atual (`Validating`, `CheckingAssociations`, `AccessingSecureStorage`, `PersistingCredentials`, `PersistingMetadata`, `Finalizing`). `add` aceita um `request_id` opcional para identificar o trabalho antes de a conta obter `AccountId`.

O status é um snapshot rápido, **sem consultar rede, keyring ou cadastro a cada chamada**. Não informa progresso percentual nem aguarda OAuth realizado pelo Provider. As atividades deixam o snapshot ao terminar e o resultado/erro é devolvido pelo método original. `get`, `list` e `update` continuam consultas/alteração local delimitadas, sem tarefa monitorada. Não criar fila de jobs, `pause/resume/stop` nem expor segredos no acompanhamento.

## Persistência e recuperação

O cadastro pode usar SQLite, arquivo estruturado ou outro meio conforme necessidade real. Consumidores não recebem handles de banco, queries SQL ou caminhos físicos. Credenciais devem permanecer em Secret Service/keyring (ou equivalente seguro); sem fallback para texto claro ou tabelas comuns.

Se for adotada uma persistência própria para o cadastro, sua política de integridade e recuperação deve funcionar com padrões internos, independentemente do módulo auxiliar [Configuration](../configuration/README.md). Preferências opcionais poderão ser adicionadas por demanda, mas **não exportam nem restauram automaticamente segredos**. Falha no keyring e necessidade de reautorização são condições próprias e não justificam reconstrução de inventários de sincronização.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Orientações para agentes](AGENTS.md)

## Decisões de implementação

A adaptação de `GoogleTokenResponse` e `GoogleSession` pertence a Accounts e usa a API existente de Providers. Não é necessária uma alteração em Providers. Cadastro JSON privado, Secret Service, guarda de remoção e recuperação de intenções estão detalhados em [INTERFACES.md](INTERFACES.md#8-decisões-concretas-da-implementação).

O proprietário definitivo das conexões continua externo; ele deverá fornecer a guarda de remoção contratada. A integração com a interface GTK e migrações de formatos antigos não pertencem a esta entrega.

## Implementação e limites de integração

A implementação está em `engine/src/accounts/`, com os nove métodos públicos, armazenamento privado de metadados e credenciais e acompanhamento transitório. O único serviço habilitável nesta versão é Google, correspondente ao Provider disponível; serviços desconhecidos retornam `UnsupportedProvider`. Seus metadados continuam consultáveis sem autenticação.

A validação usa armazenamento temporário real, dependências controladas, cliente Google contra HTTP local e Secret Service real em barramento privado descartável. Não acessa Google real nem o keyring pessoal. O cadastro de conexões e a interface GTK permanecem externos. Consulte [TESTS.md](TESTS.md) para os resultados e o bloqueio preexistente da suíte global.
