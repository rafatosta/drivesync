# Plano de publicação e verificação Google OAuth

**Estado:** planejamento, sem execução das etapas de publicação ou verificação.  
**Revisão:** 26/10/2026.  
**Objetivo:** permitir que usuários de contas Google autorizem o DriveSync sem criar projetos Google Cloud individuais, começando por builds experimentais distribuídas pelo GitHub, independentemente de publicação na Flathub.

> Este documento é um checklist de trabalho, não a confirmação de que o aplicativo já foi aprovado pelo Google ou que um pacote de produção já foi publicado. Revalidar os requisitos oficiais antes do envio.

## 1. Decisões e limites

- [x] Manter perfis técnicos distintos: **Development** e **Production** (já previstos no projeto).
- [x] Manter **projetos Google Cloud/OAuth separados** para desenvolvimento e produção.
- [ ] Validar o aplicativo com contas e arquivos descartáveis antes de ampliar o grupo de testes.
- [ ] Publicar um site estático verificável com política de privacidade e termos de serviço.
- [ ] Submeter marca e escopos necessários para verificação no Google.
- [ ] Gerar um **pacote candidato de produção** pelo GitHub Actions, mesmo sem criar um GitHub Release e sem publicar na Flathub.
- [ ] Decidir quando disponibilizar esse pacote a testadores externos e, posteriormente, ao público.

**Três assuntos independentes:** (1) ambiente de compilação/aplicação; (2) status do Google OAuth; (3) canal de distribuição. Um Flatpak compilado pelo GitHub Actions não recebe aprovação OAuth automaticamente. O Google não exige distribuição pela Flathub nem GitHub Releases para verificar a aplicação.

## 2. Perfis e fluxos de distribuição

| Aspecto | Desenvolvimento local | Candidato de produção |
| --- | --- | --- |
| Identificador Flatpak | `com.rtosta.DriveSync.Devel` | `com.rtosta.DriveSync` |
| Manifesto atual | `flatpak/com.rtosta.DriveSync.Devel.yml` | `flatpak/com.rtosta.DriveSync.yml` |
| OAuth no build | `data/oauth/development.json` | `data/oauth/production.json` |
| Projeto Google Cloud | Testing (usuários permitidos manualmente) | Projeto próprio de Production (a verificar/publicar) |
| Origem do pacote | GNOME Builder ou compilação local | GitHub Actions com execução controlada |
| Distribuição inicial | Execução local do mantenedor | **Artifact** privado ao fluxo de trabalho do GitHub, sem release público |
| Público pretendido | Mantenedor/desenvolvedores | Primeiros testadores autorizados; depois qualquer conta Google, após verificação |

**Importante:** os arquivos JSON citados representam **metadados do cliente OAuth de desktop**, não tokens pessoais. A configuração pode ser incorporada ao binário em tempo de compilação, como já previsto em [google-oauth.md](google-oauth.md). Os tokens de usuário **nunca** entram no GitHub, em CI ou nos pacotes. Credenciais de aplicativo desktop não constituem um segredo confidencial por estarem em um executável distribuído.

- [ ] Construir workflow de CI de produção sem modificar o manifesto de desenvolvimento.
- [ ] Provisionar o OAuth de produção no job de compilação e falhar caso esteja ausente ou inválido.
- [ ] Gerar artefato Flatpak instalável, com identificação da versão, commit e checksum SHA-256.
- [ ] Executar testes automatizados e conferir permissões, metadados, sandbox e fluxo OAuth.
- [ ] Testar instalação, atualização, desinstalação e coabitação com `.Devel`.
- [ ] Definir retenção dos artifacts e regras para acesso dos testadores.
- [ ] Promover uma compilação testada para um **GitHub Release marcado como prerelease** somente quando a distribuição externa for desejada.
- [ ] Tratar envio à Flathub como decisão posterior e independente.

**Artifact não é Release:** o artifact de uma execução do Actions serve para distribuição controlada e diagnóstico; seu acesso e sua retenção são limitados pelas regras do GitHub. Uma pré-release facilita downloads para testadores externos, mas não é condição para validar o OAuth.

## 3. Estratégia de Git e decisões pendentes

**Proposta inicial (não implementada):** desenvolvimento contínuo na `main`, protegido por testes e branches de curta duração (`feat/*`, `fix/*`). Produzir candidatos de produção por **workflow manual** (`workflow_dispatch`) ou por tags de versão no commit escolhido, e não automaticamente a cada push na `main`.

Não há necessidade técnica de uma branch `develop` permanente apenas porque existem perfis Flatpak Development e Production. **Branch Git, perfil de build e projeto OAuth são conceitos distintos.** Uma branch `develop` só justificaria manutenção extra se houver efetivamente uma linha de estabilização paralela.

- [ ] Decidir política de proteção da `main` e testes obrigatórios.
- [ ] Definir convenção de versões e candidatos (`v0.x.y-rc.N`) e critérios de promoção.
- [ ] Implementar build de candidato pela Actions a partir de commit/tag identificável.
- [ ] Manter código-fonte do site na branch independente `web` (ver [website-plan.md](website-plan.md)); não usar essa branch para o aplicativo.
- [ ] Evitar usar a branch `gh-pages` como fonte do site se a implantação for feita pelo mecanismo oficial de artifacts/Actions do Pages.
- [ ] Reavaliar uma branch `develop` apenas caso se torne necessária uma estabilização duradoura de versões em paralelo.

## 4. Google Cloud e escopos

O código atual solicita `openid`, `email` e `https://www.googleapis.com/auth/drive`. Este último é **restrito**, permitindo leitura/escrita ampla. Segundo o Google, aplicativos de backup e sincronização podem se qualificar para esse escopo, mas devem justificar necessidade e cumprir a política de dados.

- [ ] Confirmar se os escopos usados pelo código correspondem exatamente aos declarados no Console.
- [ ] Documentar por que `drive` é necessário para selecionar e sincronizar **pastas preexistentes arbitrárias**.
- [ ] Avaliar `drive.file` com rigor, sem substituí-lo automaticamente: pode exigir Picker/seleção por arquivo e mudar a experiência do sincronizador.
- [ ] Criar/separar projeto Google Cloud Production e habilitar Drive API.
- [ ] Criar cliente OAuth do tipo **Desktop app**, não Web nem Service Account.
- [ ] Configurar o projeto como público **External**, com e-mail de suporte e contatos consistentes.
- [ ] Completar Branding com domínio, página inicial, privacidade, termos e logotipo apropriados.
- [ ] Declarar somente os escopos efetivamente solicitados; não solicitar permissões para recursos hipotéticos.
- [ ] Confirmar que o nome/identidade da tela OAuth coincide com site, vídeo e aplicativo.
- [ ] Registrar evidências do fluxo OAuth com navegador externo, PKCE e retorno local, sem expor credenciais.

### Estados de autorização (não confundir com o tipo de build)

- **Testing:** só contas na lista de testadores (limite usual de 100); refresh tokens para autorização em teste podem expirar em 7 dias, com exceções específicas de escopo.
- **In production, ainda não verificado:** não equivale a aprovado; para escopos sensíveis/restritos há aviso de app não verificado e limite de usuários não verificados.
- **Verificado:** público elegível pode autorizar sem o aviso causado pela falta de verificação (ainda sujeito a decisões individuais de consentimento e políticas de administradores Workspace).

Mudanças de branding/escopos e a transição de status podem exigir etapas separadas de revisão. Confirmar os status reais no **Verification Center**, sem supor aprovação por simplesmente colocar o projeto em Production.

## 5. Checklist do site como pré-requisito da verificação

O detalhamento fica em [Plano do site](website-plan.md). Para o envio ao Google, verificar pelo menos:

- [ ] Site público, acessível sem login e associado à mesma identidade de marca do DriveSync.
- [ ] Domínio próprio sob controle do mantenedor e **propriedade verificada no Google Search Console**.
- [ ] Página inicial descrevendo funções reais, finalidade do acesso Google e limitações conhecidas.
- [ ] Política de privacidade pública no mesmo domínio e ligada pela página inicial; **URL idêntica à cadastrada no OAuth**.
- [ ] Termos de serviço acessíveis, com URL cadastrada no Branding quando solicitada/obrigatória.
- [ ] Contato de suporte verificável e aviso de que o projeto não é afiliado ao Google.
- [ ] URLs estáveis, HTTPS ativo, sem redirecionamentos quebrados ou conteúdo genérico/placeholder.
- [ ] Domínios autorizados em Branding coerentes com os URLs reais do site.

## 6. Revisão de segurança e privacidade

- [ ] Fazer inventário dos fluxos de dados: API Google ↔ dispositivo, cache e arquivos locais, estado SQLite/`.drivesync/`, Secret Service, diagnósticos e eventuais serviços de terceiros.
- [ ] Garantir que refresh tokens não saiam do armazenamento de credenciais do usuário e não apareçam em logs.
- [ ] Documentar revogação, desconexão, exclusão dos dados locais gerenciados pelo app e limites de recuperação.
- [ ] Testar falhas/retries, escopos insuficientes, tokens expirados, navegação OAuth cancelada e indisponibilidade do cofre.
- [ ] Validar regras para conflitos, deleções e operações incompletas antes de oferecer sincronização destrutiva automática.
- [ ] Informar claramente se **não** há servidores intermediários; não afirmar isso antes de auditar código/telemetria/dependências.
- [ ] Confirmar com o Google se há exigência de **avaliação independente CASA**: ela pode se aplicar quando dados de escopo restrito passam por servidores de terceiros. Não presumir isenção automática por ser desktop/local.
- [ ] Verificar a conformidade com a política de **Limited Use** e eventuais atualizações da política Google.

## 7. Preparar e enviar a verificação

- [ ] Concluir e publicar página inicial, política e termos, com domínio verificado.
- [ ] Concluir verificação/publicação do **Branding**, conforme etapas vigentes no Google Auth Platform.
- [ ] Gravar **vídeo não listado** demonstrando o aplicativo real, nome/logotipo e fluxo OAuth de ponta a ponta, incluindo **tela de consentimento completa em inglês**, escopos e operações relevantes com arquivos de teste.
- [ ] Preparar justificativa clara do escopo `drive` com base em funções já existentes.
- [ ] Enviar os dados solicitados no Google Auth Platform / **OAuth Verification Center**.
- [ ] Acompanhar mensagens no contato cadastrado, responder exigências e registrar decisões.
- [ ] Guardar evidências de aprovação de Branding, escopos e eventual parecer sobre CASA.
- [ ] Validar com **conta que não esteja na lista de testes**, somente quando a configuração aprovada permitir; confirmar ausência do aviso de não verificado.
- [ ] Documentar necessidade de nova verificação se escopos ou fluxos de dados mudarem.

## 8. Publicação experimental e acompanhamento

- [ ] Gerar pacote Production pelo Actions e registrar commit, versão, manifesto e SHA-256.
- [ ] Fazer teste funcional com conta Google descartável e arquivos substituíveis.
- [ ] Publicar instruções de instalação, limitações, reporte de bugs e reversão.
- [ ] Criar cadastro opcional de interessados em testar (coleta mínima e política de privacidade).
- [ ] Liberar artifact a testadores controlados; posteriormente criar prerelease pública se apropriado.
- [ ] Nunca anunciar aprovação OAuth/segurança geral sem evidência; a verificação Google **não** atesta que o sincronizador está livre de bugs.
- [ ] Acompanhar issues, falhas OAuth, retenção e renovações de consentimento.

## 9. Critérios de conclusão desta iniciativa

1. Site, política de privacidade, termos e domínio configurados e acessíveis.
2. Aplicativo OAuth de produção aprovado para os escopos necessários (e avaliação de segurança resolvida, se exigida).
3. Build Production reproduzível por GitHub Actions, testada, identificável e separada da instalação `.Devel`.
4. Pessoa externa consegue instalar o pacote e conectar uma conta Google elegível sem projeto Google Cloud/JSON próprio nem aviso de app não verificado.
5. Documentação pública consistente sobre riscos, estado experimental e canais de suporte.

## Referências oficiais

- [Google — visão geral do estado de publicação OAuth](https://developers.google.com/identity/protocols/oauth2/production-readiness/overview)
- [Google — requisitos de verificação da marca e do site](https://support.google.com/cloud/answer/13464321)
- [Google — verificação de escopos restritos](https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification)
- [Google Drive — classificação dos escopos e apps permitidos](https://developers.google.com/workspace/drive/api/guides/api-specific-auth)
- [Google — política de dados e Limited Use](https://developers.google.com/terms/api-services-user-data-policy)
- [GitHub — publicação com Actions no GitHub Pages](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages)
- Documentação interna: [OAuth Desktop](google-oauth.md) · [Perfis de build](build-profiles.md) · [Plano do site](website-plan.md)
