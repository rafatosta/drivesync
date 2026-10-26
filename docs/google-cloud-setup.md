# Configurar Google Cloud para testar o DriveSync

Este guia mostra como criar **seu próprio projeto Google Cloud**, permitir acesso à API Google Drive e gerar o arquivo OAuth exigido pelo DriveSync. Serve tanto para o mantenedor quanto para quem deseja testar usando sua própria conta, sem solicitar inclusão na lista de testadores do projeto oficial.

> **Situação atual:** o DriveSync permite login, consulta de pastas e configuração de destinos. Downloads e sincronização de arquivos ainda não estão implementados.

## 1. Criar o projeto

1. Acesse o [Google Cloud Console](https://console.cloud.google.com/projectcreate) e entre com sua conta Google.
2. Clique em **Criar projeto** e informe um nome, por exemplo, **DriveSync Testing**.
3. Selecione o projeto recém-criado no seletor de projetos do console.
4. Confirme que ele está selecionado antes de configurar APIs e OAuth.

Para isolar testes da distribuição oficial, mantenha **projetos Google Cloud separados para Testing e Production**. O status de publicação é configurado para o aplicativo OAuth do projeto, não individualmente para cada cliente Desktop.

## 2. Ativar a Google Drive API

1. Com o projeto correto selecionado, acesse [Google Drive API — Biblioteca](https://console.cloud.google.com/apis/library/drive.googleapis.com).
2. Clique em **Ativar** (ou **Enable**).
3. Verifique se a API aparece como ativada no projeto.

Não é necessário ativar APIs de escrita ou criar contas de serviço para o login desktop do DriveSync.

## 3. Configurar o consentimento OAuth

1. Abra [Google Auth Platform — Branding](https://console.cloud.google.com/auth/branding).
2. Se aparecer **Get started / Começar**, inicie a configuração.
3. Em **App name**, informe um nome como **DriveSync Testing**.
4. Escolha um **User support email** (e-mail de suporte).
5. Em [Audience / Público-alvo](https://console.cloud.google.com/auth/audience), escolha **External / Externo**. A opção Internal é destinada a organizações Google Workspace elegíveis, não a contas Google comuns.
6. Em **Contact information**, informe um e-mail para notificações do Google.
7. Leia os termos e finalize a configuração. Mantenha o aplicativo em **Testing / Em teste** para testes privados.

A interface do Google Cloud pode apresentar nomes ligeiramente diferentes conforme o idioma e a evolução do console.

## 4. Configurar as permissões (Data Access)

1. Abra [Google Auth Platform — Data Access](https://console.cloud.google.com/auth/scopes).
2. Em **Add or remove scopes / Adicionar ou remover escopos**, inclua **exatamente** as permissões utilizadas pelo DriveSync:

| Permissão OAuth | Utilização |
| --- | --- |
| `openid` | Identifica a conta Google autenticada. |
| `email` | Permite obter o endereço de e-mail da conta, quando disponibilizado pelo Google. |
| `https://www.googleapis.com/auth/drive` | Permite ler os arquivos existentes e criar novos arquivos quando o upload opcional for ativado. |

3. Salve as permissões selecionadas.

**Atenção:** `drive` é uma permissão **restrita** e ampla. O DriveSync solicita essa permissão já na conexão inicial para evitar uma segunda autorização ao ativar upload. A opção de upload continua desativada inicialmente, e nenhuma operação de upload ocorre até que o usuário a habilite. Para distribuição pública, o Google pode exigir verificação do aplicativo e requisitos adicionais. Consulte a [classificação oficial dos escopos](https://developers.google.com/workspace/drive/api/guides/api-specific-auth).

## 5. Adicionar usuários de teste

1. Abra [Google Auth Platform — Audience](https://console.cloud.google.com/auth/audience).
2. Em **Test users / Usuários de teste**, clique em **Add users / Adicionar usuários**.
3. Adicione **o seu próprio e-mail Google** e os e-mails das demais pessoas que você deseja autorizar.
4. Salve a lista.

Cada usuário deve fazer login com um e-mail cadastrado **nesse mesmo projeto Google Cloud**. Em modo Testing, podem existir limites para quantidade de testadores e duração de autorizações; usuários podem precisar autorizar novamente após a expiração do refresh token.

## 6. Criar o cliente OAuth Desktop e baixar o JSON

1. Abra [Google Auth Platform — Clients](https://console.cloud.google.com/auth/clients).
2. Clique em **Create client / Criar cliente**.
3. Em **Application type / Tipo de aplicativo**, escolha **Desktop app / Aplicativo para computador**.
4. Informe um nome, por exemplo, **DriveSync Development Desktop**, e clique em **Create / Criar**.
5. Baixe o arquivo **JSON** do cliente recém-criado.

**Use o tipo Desktop app**, não Web application, Android ou Service account. O DriveSync usa o navegador do sistema, OAuth com PKCE e uma resposta local em `127.0.0.1` com porta dinâmica. Não é necessário cadastrar manualmente uma URL fixa de redirecionamento na configuração de um cliente Desktop. Saiba mais no [guia oficial de OAuth para aplicativos desktop](https://developers.google.com/identity/protocols/oauth2/native-app).

O JSON contém informações de identificação do cliente OAuth, como `client_id` e, dependendo da configuração, `client_secret`. **Ele não contém os tokens de acesso da sua conta Google.** Não compartilhe tokens reais, códigos de autorização ou dados do seu perfil. Evite publicar o JSON completo no GitHub.

## 7. Usar o JSON no DriveSync

Para uma compilação local pelo **GNOME Builder**, renomeie o JSON baixado para `development.json` e coloque-o na seguinte pasta do repositório:

```text
drivesync/
├── data/
│   └── oauth/
│       └── development.json
└── flatpak/
    └── com.rtosta.DriveSync.Devel.yml
```

1. Clone o [repositório DriveSync](https://github.com/rafatosta/drivesync).
2. Coloque o JSON em `data/oauth/development.json` (crie a pasta `oauth` se necessário).
3. Abra a pasta do projeto no **GNOME Builder**.
4. Selecione o manifesto de desenvolvimento, `com.rtosta.DriveSync.Devel.yml`.
5. Compile e execute com **Run**. Se o aplicativo já estiver aberto, reconstrua-o após adicionar ou trocar o JSON.
6. Clique em **Connect Google Account**, use o e-mail adicionado em *Test users*, conceda as permissões no navegador e retorne ao DriveSync.

Não é necessário modificar o código Rust, configurar um servidor ou copiar tokens de usuário. O arquivo local `development.json` é ignorado pelo Git.

### Compilação personalizada

O build também permite indicar outro JSON explicitamente:

```bash
DRIVESYNC_BUILD_GOOGLE_OAUTH_FILE=/caminho/cliente-desktop.json cargo build
```

Essa variável deve estar **disponível no ambiente de compilação**. Para um build Flatpak, o arquivo também precisa existir dentro do ambiente de build; não basta apontar para um caminho do computador hospedeiro que não seja acessível ao sandbox.

### Testes com um projeto Google próprio

Quem criar o próprio projeto pode testar **sem entrar na lista de testadores do projeto oficial DriveSync**, desde que adicione seu e-mail na lista de testes do **próprio projeto**, baixe o JSON Desktop desse projeto e compile localmente conforme os passos acima. O DriveSync utilizará a identidade OAuth incorporada nessa compilação.

### Futuro pacote de produção

O perfil estável utiliza `com.rtosta.DriveSync` e `data/oauth/production.json`. Deve ser configurado com **outro projeto OAuth**, que posteriormente poderá passar pelas verificações exigidas para distribuição pública. O manifesto estável exige o JSON na compilação, mas **não está pronto para publicação no Flathub** apenas por existir: o empacotamento ainda precisa atender às regras da plataforma. Veja [perfis de build](build-profiles.md).

## 8. Problemas comuns

| Sintoma | O que conferir |
| --- | --- |
| **Access blocked / App is testing** | O e-mail usado está em **Audience → Test users** do projeto que emitiu o JSON? |
| **Google connection unavailable** | O arquivo `development.json` existe antes do build? O aplicativo foi recompilado? |
| **Drive API disabled** | A **Google Drive API** foi ativada no projeto correto? |
| **invalid_client / unauthorized_client** | O JSON foi gerado para **Desktop app**? Pertence ao projeto selecionado? |
| **A permissão não aparece ou login falha** | Os três escopos foram incluídos em **Data Access** e concedidos no consentimento? |
| **Conta exige novo login após alguns dias** | Em modo **Testing**, os refresh tokens podem expirar; autorize novamente. |

## Documentação oficial

- [Google Drive API — início rápido](https://developers.google.com/workspace/drive/api/quickstart/python)
- [Google Auth Platform — Branding](https://console.cloud.google.com/auth/branding)
- [Google Auth Platform — Audience](https://console.cloud.google.com/auth/audience)
- [Google Auth Platform — Clients](https://console.cloud.google.com/auth/clients)
- [Escopos e verificação da Drive API](https://developers.google.com/workspace/drive/api/guides/api-specific-auth)
- [OAuth 2.0 para aplicativos desktop](https://developers.google.com/identity/protocols/oauth2/native-app)
