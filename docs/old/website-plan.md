# Plano do site público do DriveSync

**Estado:** planejamento; nenhum domínio, site ou workflow é criado por este documento.  
**Revisão:** 26/10/2026.  
**Finalidade:** apresentar o projeto, captar apoio e voluntários para testes e servir como **site verificável** para a aprovação do Google OAuth.

Documentação relacionada: [Plano de publicação e verificação Google](google-publication-plan.md).

## 1. Organização proposta no GitHub

**Decisão proposta para implementação posterior:** manter o site no **mesmo repositório** `rafatosta/drivesync`, com o código da aplicação na `main` e o código-fonte do site em uma branch chamada **`web`**.

- A branch `web` pode ser independente/orphan para não duplicar e misturar o histórico de arquivos do aplicativo. Essa escolha é opcional; o objetivo é isolar o **código-fonte do site**.
- O Pages deve usar **GitHub Actions** como origem de publicação, com workflow disparado em push à `web`.
- O workflow é responsável por gerar e enviar o diretório estático como **Pages artifact** e publicar com `actions/deploy-pages`.
- Configurar permissões mínimas de implantação `pages: write` e `id-token: write`, com o ambiente `github-pages` e proteções apropriadas; o site não precisa de acesso aos tokens OAuth nem ao JSON de build do aplicativo.
- A branch `web` é **fonte**, não a branch `gh-pages` de resultados. Não é necessário fazer commit dos arquivos compilados na `main` nem na `web`.
- CI do site e CI/build Flatpak permanecem separados mesmo pertencendo ao mesmo repositório.
- As URLs públicas de política e termos devem permanecer estáveis entre deploys para não quebrar a verificação OAuth.

### Estrutura ilustrativa da branch `web`

```text
/
├── .github/
│   └── workflows/
│       └── pages.yml
├── src/                  # site estático / gerador escolhido
├── public/               # ícones e demais assets públicos
├── package.json          # somente se houver build frontend
└── README.md             # instruções de manutenção do site
```

> Não foi escolhida stack web, nem criado workflow. Uma página HTML/CSS estática também atende ao objetivo; só usar framework/build tool se houver necessidade real.

### Alternativa a avaliar

Usar a pasta `site/` na `main` simplificaria revisões entre app e site. Entretanto, a branch `web` atende à intenção de separar os ciclos de edição. O custo é manter contextos, histórico e workflows separados dentro de um mesmo repositório. **Não é necessário criar uma branch `develop` para viabilizar isso.**

## 2. Domínio e publicação

**Exemplo de endereço, condicionado ao controle do DNS:** `drivesync.rtosta.com`. O nome é apenas uma proposta; não está registrado como configuração neste plano.

- [ ] Confirmar controle do domínio e definir o endereço canônico (por exemplo, um subdomínio).
- [ ] Criar `web` e pipeline GitHub Actions → GitHub Pages.
- [ ] Em *Settings → Pages*, escolher **GitHub Actions** como *Build and deployment → Source*.
- [ ] Configurar domínio personalizado no Pages e apontar registro DNS compatível.
- [ ] Habilitar HTTPS e verificar URL, redirecionamentos e certificado.
- [ ] **Verificar a propriedade do domínio no Google Search Console** com uma conta que tenha acesso ao projeto Google Cloud.
- [ ] Considerar **verificação de domínio no GitHub Pages**, também, para mitigar takeover; isso é diferente da verificação exigida pelo Google.
- [ ] Garantir que home, privacidade, termos e suporte funcionam sem login, em dispositivos móveis e sem JavaScript complexo.
- [ ] Confirmar que links publicados no Google Auth Platform apontam para **o mesmo domínio e as mesmas páginas**, sem placeholders.
- [ ] Evitar criar registro DNS curinga apenas para a hospedagem do Pages.

GitHub Pages é suficiente para hospedar o site informativo e as páginas jurídicas do OAuth. Não é necessário hospedar proxy, backend de autenticação, APIs ou arquivos pessoais dos usuários.

## 3. Mapa de conteúdo proposto

| Página/seção | Conteúdo | Papel |
| --- | --- | --- |
| **Início** (`/`) | Nome, logo, objetivo, diferenças em relação à montagem remota, funções reais e estado experimental | **Obrigatória**: identificação da marca e descrição do aplicativo |
| **Privacidade** (`/privacy/`) | Política de acesso/uso/armazenamento/compartilhamento de dados e dados Google | **Obrigatória** para verificação OAuth |
| **Termos de uso** (`/terms/`) | Regras de uso, caráter experimental, responsabilidades e limitações | Requerida para aplicativo externo em produção conforme Branding |
| **Suporte e contato** (`/support/`) | Canal de contato, problemas frequentes, GitHub Issues, como revogar acesso | Evidência de suporte e transparência |
| **Testes** (`/testing/`) | Formulário de interesse, pré-requisitos, responsabilidades, versão experimental | Comunidade e recrutamento |
| **Downloads** (`/download/`) | Instruções, versões, hashes, requisitos, fonte dos binários | Distribuição quando autorizada |
| **Contribuir/Apoiar** (`/support-project/`) | GitHub, colaboração, doações, Discord e transparência | Captação de recursos e comunidade |
| **FAQ** (página/seção) | OAuth, armazenamento, segurança, limitações, sem vínculo com Google | Clareza para usuário e avaliador |

Os caminhos são **propostas** e não precisam ser esses nomes exatos. A homepage deve ter **links visíveis** para Privacidade, Termos e Suporte. Não basta apresentar apenas login, um formulário ou links externos.

## 4. Conteúdo que o Google precisa conseguir verificar

### Página inicial e identidade

- [ ] Nome exibido é **DriveSync**, consistente no aplicativo, site e tela de consentimento.
- [ ] Identifica responsável ou mantenedor e e-mail de suporte real.
- [ ] Explica que é um aplicativo Linux/GNOME de sincronização de arquivos do Google Drive.
- [ ] Descreve com exatidão o que já funciona: conexão, seleção de pasta, download, criação opcional de arquivos remotos, consultas de mudanças e modos de sincronização disponíveis.
- [ ] Declara limitações relevantes: versão de testes; sem sincronização destrutiva ou sobrescrita bidirecional integral; não é ferramenta de backup.
- [ ] Justifica por que pede acesso a dados do Drive, inclusive o alcance amplo do escopo `drive`.
- [ ] Evita apresentar como cliente oficial do Google ou afirmar aprovação/certificação antes que ocorra.
- [ ] Apresenta link para **Política de privacidade** com a **mesma URL** cadastrada no Google Cloud.
- [ ] Possui conteúdo real, público e navegável sem autenticação.

### Política de privacidade

A política precisa descrever o comportamento **comprovado do software**. Não publicar uma declaração genérica nem prometer ações não auditadas. Preparar texto claro em português; versão inglesa pode ajudar na análise internacional e deve ser consistente.

- [ ] Identificar aplicativo, mantenedor e forma de contato.
- [ ] Listar dados autorizados pela conta Google: ID da conta, e-mail quando concedido, metadados, conteúdo dos arquivos e pastas usados na sincronização.
- [ ] Especificar para quais funções esses dados são acessados, usados e transformados.
- [ ] Informar o escopo OAuth `https://www.googleapis.com/auth/drive` e por que o aplicativo necessita desse alcance.
- [ ] Descrever **fluxos de dados**: Google Drive API ↔ aplicativo local; verificar eventual passagem por servidores, integrações ou serviços externos.
- [ ] Detalhar armazenamento local: arquivos sincronizados, configuração, índices e estado em `.drivesync/`, armazenamento das credenciais via Secret Service; corrigir após inventário técnico final.
- [ ] Declarar existência (ou ausência comprovada) de analytics, telemetria, crash reports, anúncios e monetização com dados.
- [ ] Informar se dados são compartilhados com terceiros e para quais finalidades (incluindo eventual formulário de inscrições, caso aplicável).
- [ ] Explicar duração/retensão dos dados, desconexão, revogação de permissão, exclusão dos dados mantidos e limitações.
- [ ] Informar como usuário solicita correção/exclusão de dados sob controle do projeto, quando aplicável.
- [ ] Explicar armazenamento local de tokens e salvaguardas de proteção **sem prometer segurança absoluta**.
- [ ] Alinhar as declarações à **Google API Services User Data Policy / Limited Use**.
- [ ] Informar data da política, versão/revisões e como mudanças relevantes serão comunicadas.
- [ ] Expor a política também por link facilmente acessível na interface do aplicativo.

**Separar consentimentos:** quem preenche o formulário de testes fornece dados de contato ao operador do formulário. Isso não se confunde com os dados Google autorizados ao app por OAuth. A página do formulário deverá indicar finalidade, ferramenta de coleta, acesso às respostas, prazo de retenção e como solicitar exclusão.

### Termos de serviço/uso

- [ ] Definir responsável, aplicação e condições de uso.
- [ ] Explicar que a distribuição inicial é **experimental**, com riscos, limitações conhecidas e orientação para usar arquivos de teste.
- [ ] Descrever responsabilidade do usuário sobre cópias de segurança independentes e uso adequado.
- [ ] Informar direitos de uso e distribuição de acordo com a **licença efetivamente publicada** no repositório; definir licença antes de afirmar “código aberto” com permissão de reutilização.
- [ ] Explicar como suporte, contribuição voluntária e doações funcionam, sem prometer funcionalidades ou disponibilidade contínua.
- [ ] Indicar política de alterações dos termos e canal de contato.
- [ ] Revisar redação e requisitos legais locais antes de publicar como texto definitivo.

### Demonstração para verificação

- [ ] Gravar vídeo funcional de ponta a ponta, com nome e identidade visual iguais à página pública.
- [ ] Incluir consentimento OAuth completo com **idioma inglês**, mostrando os escopos solicitados.
- [ ] Demonstrar as funcionalidades que justificam o acesso amplo (pasta existente, listagem, download/upload permitido).
- [ ] Usar conta e arquivos descartáveis, sem dados pessoais em gravações.
- [ ] Hospedar vídeo não listado em URL acessível ao revisor e fornecer link no formulário de verificação.
- [ ] Oferecer documentação sobre como instalar/operar build de teste quando solicitada pela equipe Google.

## 5. Design, conteúdo e funcionamento

Diretriz visual: identidade GNOME/libadwaita, apresentação técnica clara e enxuta; evitar claims publicitários como “100% seguro”, “oficial” ou “sincronização completa” enquanto não verificados.

- [ ] Página inicial com hero, o que faz, recursos disponíveis, limitações, capturas reais, CTA para testes e CTA para contribuir.
- [ ] Layout responsivo e navegação por teclado; preferir HTML semântico e boa legibilidade.
- [ ] Preparar PT-BR; decidir necessidade de tradução EN para revisores.
- [ ] Não expor arquivos `production.json` de usuário, tokens ou logs privados em assets.
- [ ] Não depender de Google Analytics/trackers para validar o site; documentar qualquer analytics futuro.
- [ ] Construir páginas privadas do projeto (como política e termos) com links permanentes estáveis no domínio escolhido.
- [ ] Tratar links externos de download/doação/Discord como opcionais e só publicá-los quando existirem.
- [ ] Manter FAQ de instruções de revogação da autorização Google e de remoção do aplicativo.

## 6. Formulário para voluntários e feedback

**Proposta:** Google Forms + Google Sheets ou alternativa simples, sem backend próprio no site.

Campos mínimos sugeridos: apelido, e-mail, distribuição/versão Linux, ambiente GNOME e disponibilidade para testes; eventual interesse em contribuição técnica. Não pedir senha, tokens, Drive compartilhado, arquivos pessoais ou dados confidenciais.

- [ ] Publicar política de privacidade do cadastro e checkbox de consentimento/contato aplicável.
- [ ] Restringir o acesso à planilha de respostas.
- [ ] Permitir cancelamento do interesse e pedido de exclusão de contato.
- [ ] Não apresentar cadastro como garantia de acesso imediato: OAuth Testing e distribuição dos artifacts têm regras próprias.
- [ ] Utilizar GitHub Issues para relatos públicos; orientar redatores a apagar logs sensíveis.

## 7. Critérios de aceite para publicar o site

- [ ] Branch `web` com código fonte próprio e workflow GitHub Pages validado.
- [ ] Site acessível via HTTPS em domínio sob controle do projeto.
- [ ] Domínio verificado no Search Console e, se aplicável, autorizado no Google Auth Platform.
- [ ] Página inicial, política, termos e suporte publicados, consistentes com a implementação atual.
- [ ] Links do Branding testados em navegação anônima e celular.
- [ ] Nenhum segredo/tokens do usuário no repositório, workflow ou código público.
- [ ] Vídeo e justificativas OAuth preparados para posterior submissão.
- [ ] Aplicação ainda pode continuar em desenvolvimento; site não constitui anúncio de versão estável.

## 8. Referências oficiais

- [Google — Verification requirements](https://support.google.com/cloud/answer/13464321)
- [Google — App Homepage requirements](https://support.google.com/cloud/answer/13807376)
- [Google — Manage OAuth App Branding](https://support.google.com/cloud/answer/15549049)
- [Google — Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy)
- [Google — Restricted scope verification](https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification)
- [GitHub — Pages publishing sources](https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site)
- [GitHub — Pages custom workflows](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages)
- [GitHub — custom domain and DNS](https://docs.github.com/en/pages/configuring-a-custom-domain-for-your-github-pages-site/managing-a-custom-domain-for-your-github-pages-site)
- [GitHub — domain verification](https://docs.github.com/en/pages/configuring-a-custom-domain-for-your-github-pages-site/verifying-your-custom-domain-for-github-pages)
