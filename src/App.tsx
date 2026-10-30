import { useI18n } from '@/i18n/useI18n';
import { LanguageSwitcher } from '@/components/LanguageSwitcher';
import { useEffect, useState } from 'react';
import { ArrowDown, ArrowRight, ArrowUpRight, Check, Code2, Heart, Menu, Moon, Sun } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Screenshot } from '@/components/screenshot';
import { ScreenshotGallery } from '@/components/screenshot-gallery';
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from '@/components/ui/accordion';
import { Sheet, SheetContent, SheetDescription, SheetTitle, SheetTrigger } from '@/components/ui/sheet';
const base = import.meta.env.BASE_URL;
const repo = 'https://github.com/rafatosta/drivesync';
const pages = [{
  id: 'app',
  label: 'Aplicativo',
  path: ''
}, {
  id: 'development',
  label: 'Desenvolvimento',
  path: 'desenvolvimento/'
}, {
  id: 'donations',
  label: 'Doações',
  path: 'doacoes/'
}, {
  id: 'privacy',
  label: 'Privacidade',
  path: 'privacidade/'
}, {
  id: 'terms',
  label: 'Termos de Serviço',
  path: 'termos/'
}];
const page = pages.find(item => item.path && window.location.pathname.replace(base, '').startsWith(item.path)) ?? pages[0];
function RepositoryLink({
  children
}: {
  children?: React.ReactNode;
}) {
  const {
    t
  } = useI18n();
  return <a className="text-link" href={repo}>{children ?? t("Ver no GitHub")}<ArrowUpRight size={16} /></a>;
}
function PageHeading({
  eyebrow,
  title,
  children
}: {
  eyebrow: string;
  title: string;
  children: React.ReactNode;
}) {
  return <div className="page-heading"><p className="eyebrow">{eyebrow}</p><h1>{title}</h1><p className="lede">{children}</p></div>;
}
function Application() {
  const {
    t
  } = useI18n();
  return <>
    <section className="hero"><div className="hero-copy"><p className="eyebrow"><span className="status-dot" /> {t("Pr\xE9via t\xE9cnica \xB7 Linux")}</p><h1>{t("Google Drive,")}<br /><span>{t("no seu Linux.")}</span></h1><p className="lede">{t("Escolha suas pastas. Acompanhe os arquivos.")}<br className="desktop-break" /> {t("Mantenha uma c\xF3pia por perto.")}</p><div className="hero-actions"><Button nativeButton={false} size="lg" render={<a href="#disponibilidade" />}>{t("Conhecer a pr\xE9via")} <ArrowDown /></Button><RepositoryLink /></div><p className="hero-footnote">{t("Um cliente independente, com c\xF3digo aberto.")} <a className="gallery-jump" href="#galeria">{t("Explorar as telas \u2193")}</a></p></div><div className="hero-aside"><img src={`${base}icon.svg`} alt="" width="128" height="128" /><p>{t("Feito para Linux.")}<br />{t("Com aten\xE7\xE3o ao seu desktop.")}</p><span>{t("GTK4 / Libadwaita")}</span></div></section>
    <div className="hero-image"><div className="image-heading"><span>{t("DriveSync em uso")}</span><span>{t("Interface atual / dados fict\xEDcios")}</span></div><Screenshot src="dashboard.png" title={t("Painel do DriveSync com m\xFAltiplas contas")} description={t("Contas e pastas em um s\xF3 lugar. Captura com contas fict\xEDcias e estados demonstrativos.")} /></div>
    <section className="section feature-section"><div className="section-heading"><p className="eyebrow">{t("O aplicativo")}</p><h2>{t("Menos dist\xE2ncia entre")}<br />{t("o Drive e seus arquivos.")}</h2></div><div className="feature-list">{[['Mais de uma conta', 'Conecte contas Google com configurações independentes. Escolha uma pasta do Drive e uma pasta local para cada conta.'], ['Cópias no computador', 'Baixe arquivos e subpastas e atualize cópias gerenciadas quando o conteúdo remoto mudar. Documentos Google são representados por links para o navegador.'], ['Atividade à vista', 'Acompanhe o progresso e o histórico. Pause ou retome uma sincronização e escolha entre verificações periódicas ou manuais.']].map(([title, text]) => <article key={title}><Check size={18} /><div><h3>{t(title)}</h3><p>{t(text)}</p></div></article>)}</div></section>
    <ScreenshotGallery />
    <section id="disponibilidade" className="section availability"><p className="eyebrow">{t("Antes de come\xE7ar")}</p><div className="availability-grid"><h2>{t("Ainda \xE9 uma pr\xE9via.")}<br />{t("Vale conhecer os limites.")}</h2><div><p>{t("O DriveSync est\xE1 em desenvolvimento e ainda n\xE3o oferece sincroniza\xE7\xE3o completa nos dois sentidos. O envio opcional cobre novos arquivos e pastas; altera\xE7\xF5es em arquivos existentes e exclus\xF5es locais n\xE3o s\xE3o enviadas ao Drive.")}</p><p>{t("A distribui\xE7\xE3o p\xFAblica depende de valida\xE7\xE3o, empacotamento e aprova\xE7\xE3o do login Google. Builds de teste podem estar restritas a contas autorizadas.")}</p><p><strong>{t("Teste com arquivos que voc\xEA possa recriar. O DriveSync n\xE3o \xE9 um servi\xE7o de backup.")}</strong></p><a className="text-link" href={`${base}desenvolvimento/`}>{t("Ver o estado do desenvolvimento")} <ArrowRight size={16} /></a></div></div></section>
    <section className="section closing"><div><p className="eyebrow">{t("Projeto independente")}</p><h2>{t("Voc\xEA pode fazer parte.")}</h2><p>{t("Um relato bem descrito, um teste cuidadoso ou uma contribui\xE7\xE3o ajudam o projeto a avan\xE7ar.")}</p></div><div className="closing-links"><a href={`${base}desenvolvimento/`}>{t("Contribuir com o projeto")} <Code2 size={20} /></a><a href={`${base}doacoes/`}>{t("Apoiar com uma doa\xE7\xE3o")} <Heart size={20} /></a></div></section>
  </>;
}
function Development() {
  const {
    t
  } = useI18n();
  return <><PageHeading eyebrow={t("Desenvolvimento aberto")} title={t("O projeto, por dentro.")}>{t("O DriveSync nasce para aproximar o Google Drive do desktop Linux. Aqui voc\xEA acompanha o que j\xE1 existe, as prioridades e como o trabalho \xE9 feito.")}</PageHeading>
    <section className="section editorial"><h2>{t("O que funciona hoje")}</h2><div><p>{t("A pr\xE9via permite conectar m\xFAltiplas contas, selecionar pastas, baixar arquivos e atualizar c\xF3pias locais. O envio opcional \xE9 limitado a novos arquivos e pastas.")}</p><p>{t("A sincroniza\xE7\xE3o completa nos dois sentidos, o suporte a Drives compartilhados e uma interface completa para resolver conflitos ainda n\xE3o est\xE3o dispon\xEDveis.")}</p><RepositoryLink>{t("Consultar o estado atual no reposit\xF3rio")}</RepositoryLink></div></section>
    <section className="section"><p className="eyebrow">{t("Prioridades")}</p><h2>{t("Confiabilidade antes de ampliar o escopo.")}</h2><ol className="roadmap"><li><span>{t("01 / Foco atual")}</span><div><h3>{t("Configura\xE7\xE3o e recupera\xE7\xE3o")}</h3><p>{t("Tornar os primeiros passos mais claros, melhorar a recupera\xE7\xE3o de interrup\xE7\xF5es e a apresenta\xE7\xE3o dos conflitos.")}</p></div></li><li><span>{t("02 / Valida\xE7\xE3o")}</span><div><h3>{t("O aplicativo no ambiente real")}</h3><p>{t("Validar login Google, instala\xE7\xE3o, opera\xE7\xE3o em segundo plano e abertura de documentos, considerando as diferen\xE7as entre desktops Linux.")}</p></div></li><li><span>{t("03 / Evolu\xE7\xE3o")}</span><div><h3>{t("Sincroniza\xE7\xE3o mais completa")}</h3><p>{t("Ampliar o tratamento de atualiza\xE7\xF5es e exclus\xF5es com salvaguardas. Novas fun\xE7\xF5es dependem de valida\xE7\xE3o; n\xE3o h\xE1 prazo de lan\xE7amento definido.")}</p></div></li></ol></section>
    <section className="section editorial"><h2>{t("Como desenvolvemos")}</h2><div><p>{t("A dire\xE7\xE3o do produto, as prioridades e a aceita\xE7\xE3o das entregas s\xE3o conduzidas pelo mantenedor. Agentes de IA participam da pesquisa, discuss\xE3o e implementa\xE7\xE3o do c\xF3digo.")}</p><p>{t("O objetivo \xE9 trabalhar a partir de requisitos, revis\xE3o e testes. C\xF3digo gerado e revis\xF5es automatizadas precisam de verifica\xE7\xE3o: n\xE3o s\xE3o garantia de qualidade nem substituem valida\xE7\xE3o real.")}</p><p>{t("A aplica\xE7\xE3o usa Rust, GTK4 e Libadwaita. Sua interface segue as diretrizes de design do GNOME, com o Linux como plataforma de destino.")}</p></div></section>
    <section className="section"><h2>{t("Tr\xEAs maneiras de participar")}</h2><div className="participation"><article><h3>{t("Relatar um problema")}</h3><p>{t("Conte o que esperava, o que aconteceu e como reproduzir. Remova dados pessoais dos registros.")}</p><a className="text-link" href={`${repo}/issues`}>{t("Abrir uma issue")} <ArrowUpRight size={16} /></a></article><article><h3>{t("Testar os fluxos")}</h3><p>{t("Consulte as instru\xE7\xF5es de desenvolvimento e use contas e arquivos de teste substitu\xEDveis.")}</p><RepositoryLink>{t("Consultar instru\xE7\xF5es")}</RepositoryLink></article><article><h3>{t("Contribuir com c\xF3digo")}</h3><p>{t("Converse sobre o escopo e consulte as orienta\xE7\xF5es do reposit\xF3rio antes de iniciar mudan\xE7as maiores.")}</p><RepositoryLink>{t("Explorar o c\xF3digo")}</RepositoryLink></article></div></section>
    <section className="section faq"><h2>{t("Perguntas frequentes")}</h2><Accordion>{[['O aplicativo exige GNOME?', 'Não. O DriveSync é um cliente para Linux. GTK4 e Libadwaita orientam a interface; integrações como segundo plano e início de sessão precisam ser validadas nos ambientes de destino.'], ['Já existe uma versão estável para instalar?', 'Ainda não há uma versão estável anunciada. Consulte o repositório para instruções de desenvolvimento e disponibilidade da prévia.'], ['Quando haverá sincronização completa?', 'Não há prazo definido. O foco é validar os fluxos e as salvaguardas antes de ampliar as operações sobre os arquivos.']].map(([question, answer]) => <AccordionItem value={question} key={question}><AccordionTrigger>{t(question)}</AccordionTrigger><AccordionContent>{t(answer)}</AccordionContent></AccordionItem>)}</Accordion></section></>;
}
function Donations() {
  const { t } = useI18n();
  const methods = [
    { name: 'GitHub Sponsors', url: 'https://github.com/sponsors/rafatosta' },
    { name: 'Ko-fi', url: 'https://ko-fi.com/rafaeltosta' },
    { name: 'PayPal', url: 'https://www.paypal.com/donate/?business=E7R4BVR45GRC2' },
    { name: 'Wise', url: 'https://wise.com/pay/me/rafaelt2487' },
    { name: 'Pix', url: 'https://nubank.com.br/pagar/3c3r2/LS2hiJJKzv' },
  ];
  return <><PageHeading eyebrow={t("Apoie o DriveSync")} title={t("Ajude o projeto a continuar.")}>{t("O DriveSync é um projeto independente. Contribuições voluntárias podem apoiar o tempo dedicado ao desenvolvimento, aos testes e à manutenção.")}</PageHeading>
    <section className="donation-panel"><Heart size={28} strokeWidth={1.5} /><div><h2>{t("Escolha como apoiar o DriveSync")}</h2><p>{t("As doações são realizadas em plataformas externas. Escolha um dos meios abaixo para contribuir voluntariamente.")}</p><div className="donation-methods">{methods.map(method => <a className="donation-method" key={method.name} href={method.url} target="_blank" rel="noopener noreferrer"><span>{method.name}</span><ArrowUpRight size={18} aria-hidden="true" /><span className="sr-only">{t("Abre em uma nova aba")}</span></a>)}</div><p className="donation-note">{t("Ao escolher um meio de doação, você será direcionado ao respectivo serviço, sujeito à política de privacidade dele. O DriveSync não processa pagamentos.")}</p></div></section>
    <section className="section editorial"><h2>{t("O que o apoio viabiliza")}</h2><div><h3>{t("Tempo para desenvolver")}</h3><p>{t("Investigar problemas, corrigir falhas e evoluir o aplicativo.")}</p><h3>{t("Testes e documentação")}</h3><p>{t("Validar fluxos reais, melhorar as instruções e acompanhar a experiência em diferentes desktops Linux.")}</p><h3>{t("Manutenção contínua")}</h3><p>{t("Acompanhar dependências e relatos de problemas ao longo do tempo.")}</p></div></section><section className="section editorial"><h2>{t("Uma contribuição voluntária")}</h2><div><p>{t("Doar não compra funcionalidades, suporte prioritário ou prazos de entrega. O apoio financeiro é opcional e não condiciona o acesso ao código.")}</p><p>{t("Você também pode ajudar com testes, relatos de problemas e contribuições ao código.")} <a className="text-link" href={`${base}desenvolvimento/`}>{t("Conhecer outras formas de apoiar")} <ArrowRight size={16} /></a></p></div></section></>;
}
function Privacy() {
  const {
    t
  } = useI18n();
  return <><PageHeading eyebrow={t("Privacidade \xB7 Atualizada em 26 de outubro de 2026")} title={t("Seus dados e o DriveSync.")}>{t("Esta pol\xEDtica descreve o tratamento de informa\xE7\xF5es pelo site e pela pr\xE9via t\xE9cnica do aplicativo DriveSync, projeto independente mantido por rafatosta.")}</PageHeading><div className="policy">
    <section><h2>{t("1. Este site")}</h2><p>{t("O site apresenta o aplicativo, o desenvolvimento e as formas de apoio. Ele n\xE3o solicita login Google, n\xE3o acessa seu Drive e n\xE3o cont\xE9m formul\xE1rios, processamento de pagamentos ou ferramentas de an\xE1lise de audi\xEAncia.")}</p><p>{t("O site não define cookies de rastreamento. As preferências de tema e idioma são salvas no armazenamento local do navegador; você pode removê-las nas configurações do navegador. Fontes, ícones e imagens são servidos junto com o site.")}</p><p>{t("A hospedagem \xE9 feita no GitHub Pages. O GitHub pode tratar dados t\xE9cnicos das requisi\xE7\xF5es, como endere\xE7o IP, conforme sua")} <a href="https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement">{t("pol\xEDtica de privacidade")}</a>{t(". Links externos levam a servi\xE7os com suas pr\xF3prias pol\xEDticas.")}</p></section>
    <section><h2>{t("2. Conex\xE3o com o Google")}</h2><p>{t("A conex\xE3o no aplicativo acontece pelo navegador, usando OAuth. Sua senha Google n\xE3o \xE9 solicitada pelo DriveSync. O aplicativo recebe tokens de autoriza\xE7\xE3o e informa\xE7\xF5es da conta, como identificador, e-mail e nome quando fornecidos pelo Google, para identificar a conta conectada.")}</p><p>{t("A configura\xE7\xE3o atual solicita os escopos")} <code>{t("openid")}</code>, <code>{t("email")}</code> {t("e")} <code>{t("https://www.googleapis.com/auth/drive")}</code>{t(". O \xFAltimo permite acesso amplo ao Google Drive. Escolher uma pasta ou desativar uploads limita o comportamento do aplicativo, mas n\xE3o reduz o alcance da autoriza\xE7\xE3o concedida ao token.")}</p></section>
    <section><h2>{t("3. Dados utilizados pelo aplicativo")}</h2><p>{t("Para listar pastas e transferir arquivos, o aplicativo utiliza metadados como nomes, identificadores, tamanhos, tipos, vers\xF5es e estrutura de pastas, al\xE9m do conte\xFAdo dos arquivos envolvidos nas transfer\xEAncias configuradas. Documentos nativos do Google podem ser representados localmente por links.")}</p><p>{t("Tokens s\xE3o armazenados no cofre de credenciais do sistema. Configura\xE7\xF5es de conta, pastas selecionadas e estado de sincroniza\xE7\xE3o s\xE3o mantidos localmente, assim como os arquivos baixados e as vers\xF5es preservadas. Esses dados s\xE3o usados para autentica\xE7\xE3o, execu\xE7\xE3o, acompanhamento e recupera\xE7\xE3o das opera\xE7\xF5es.")}</p></section>
    <section><h2>{t("4. Comunica\xE7\xE3o e compartilhamento")}</h2><p>{t("O aplicativo se comunica com os servi\xE7os de autentica\xE7\xE3o e APIs do Google para executar as fun\xE7\xF5es solicitadas. O projeto n\xE3o opera um servidor intermedi\xE1rio para receber o conte\xFAdo sincronizado e n\xE3o inclui envio autom\xE1tico de arquivos ou registros ao mantenedor.")}</p><p>{t("Os dados acessados por meio das APIs do Google s\xE3o usados para as fun\xE7\xF5es do aplicativo, n\xE3o para publicidade, venda de dados ou treinamento de modelos de IA. O uso de agentes de IA no desenvolvimento do c\xF3digo n\xE3o implica envio dos arquivos dos usu\xE1rios a esses agentes.")}</p></section>
    <section><h2>{t("5. Desconex\xE3o, reten\xE7\xE3o e remo\xE7\xE3o")}</h2><p>{t("A desconex\xE3o no aplicativo remove as credenciais locais e as configura\xE7\xF5es da conta, sem apagar seus arquivos pessoais. Ela n\xE3o equivale a revogar a autoriza\xE7\xE3o no Google. Para revogar o acesso, visite as")} <a href="https://myaccount.google.com/connections">{t("conex\xF5es de terceiros da sua Conta Google")}</a>.</p><p>{t("C\xF3pias baixadas, vers\xF5es preservadas e arquivos locais de estado podem permanecer no computador ap\xF3s a desconex\xE3o. Sua remo\xE7\xE3o deve ser feita por voc\xEA, considerando os dados que deseja manter. O site n\xE3o mant\xE9m um cadastro de visitantes.")}</p></section>
    <section><h2>{t("6. Relatos, contato e atualiza\xE7\xF5es")}</h2><p>{t("Issues e coment\xE1rios no reposit\xF3rio s\xE3o p\xFAblicos. N\xE3o publique tokens, senhas, documentos pessoais ou registros sem remover informa\xE7\xF5es sens\xEDveis. O conte\xFAdo que voc\xEA compartilhar no GitHub estar\xE1 sujeito \xE0s regras e \xE0 reten\xE7\xE3o desse servi\xE7o.")}</p><p>{t("Para d\xFAvidas gerais sobre esta pol\xEDtica, entre em contato com o mantenedor")} <a href="https://github.com/rafatosta">{t("rafatosta pelo GitHub")}</a>{t(". N\xE3o envie dados pessoais em uma issue p\xFAblica; use esse canal apenas para solicitar uma forma adequada de contato quando necess\xE1rio.")}</p><p>{t("Esta pol\xEDtica ser\xE1 atualizada quando as pr\xE1ticas do aplicativo ou os servi\xE7os utilizados mudarem. A data da vers\xE3o vigente aparece no in\xEDcio desta p\xE1gina.")}</p></section>
  </div></>;
}
function Terms() {
  const { t } = useI18n();
  return <><PageHeading eyebrow={t("Termos de Serviço · Atualizados em 30 de outubro de 2026")} title={t("Condições de uso do DriveSync.")}>{t("Estes termos descrevem as condições de utilização do DriveSync, um projeto independente e de código aberto desenvolvido e mantido por Rafael Tosta Santos.")}</PageHeading><div className="policy">
    <section><h2>{t("1. Sobre o aplicativo")}</h2><p>{t("O DriveSync é um cliente de Google Drive para Linux, em fase experimental. Permite conectar contas, selecionar pastas, baixar arquivos e opcionalmente enviar novos arquivos e pastas. Não é um serviço de backup nem oferece, neste momento, sincronização bidirecional completa.")}</p></section>
    <section><h2>{t("2. Uso e autorização")}</h2><p>{t("Ao utilizar o DriveSync, você concorda em respeitar estes termos e as regras aplicáveis ao seu uso do Google Drive. A conexão à Conta Google é voluntária e utiliza OAuth no navegador. Você pode desconectar sua conta no aplicativo e revogar a autorização nas configurações da Conta Google.")}</p></section>
    <section><h2>{t("3. Arquivos, sincronização e backups")}</h2><p>{t("A sincronização pode criar, atualizar ou preservar cópias locais e, quando o envio estiver ativado, carregar novos arquivos para o Google Drive. Falhas, interrupções, conflitos ou alterações futuras de comportamento podem afetar seus dados. Antes de testar, mantenha backups independentes e utilize arquivos que possa recuperar. Verifique as operações antes de autorizar alterações importantes.")}</p></section>
    <section><h2>{t("4. Privacidade e serviços externos")}</h2><p>{t("O funcionamento depende dos serviços e APIs do Google. O tratamento dos dados, as permissões solicitadas, a retenção local e a revogação de acesso são explicados na")} <a href={`${base}privacidade/`}>{t("Política de Privacidade")}</a>{t(". O DriveSync é independente e não possui afiliação, patrocínio ou endosso do Google.")}</p></section>
    <section><h2>{t("5. Software livre e direitos autorais")}</h2><p>{t("Copyright (C) 2026 Rafael Tosta Santos. O DriveSync é distribuído sob a GNU General Public License, versão 3.0 apenas (GPL-3.0-only). O uso, a modificação e a redistribuição do código obedecem ao texto integral da licença disponível no")} <a href={`${repo}/blob/main/LICENSE`}>{t("repositório do projeto")}</a>.</p></section>
    <section><h2>{t("6. Disponibilidade e limitações")}</h2><p>{t("O aplicativo é oferecido como prévia técnica, sem promessa de disponibilidade contínua, funcionamento sem erros ou compatibilidade com todas as contas e ambientes Linux. Na extensão permitida pela legislação aplicável, o software é fornecido sem garantias adicionais. Nada nestes termos afasta direitos ou responsabilidades que não possam ser excluídos por lei.")}</p></section>
    <section><h2>{t("7. Atualizações dos termos")}</h2><p>{t("Os termos poderão ser revisados para refletir alterações no aplicativo ou em suas condições de distribuição. A versão vigente será publicada nesta página com a data de atualização.")}</p></section>
    <section><h2>{t("8. Contato")}</h2><p>{t("Para dúvidas sobre estes termos, entre em contato com o mantenedor pelo")} <a href="https://github.com/rafatosta">{t("perfil do GitHub")}</a>{t(". Não publique dados pessoais ou credenciais em issues públicas.")}</p></section>
  </div></>;
}
export default function App() {
  const {
    t
  } = useI18n();
  const [dark, setDark] = useState(() => {
    try {
      const saved = localStorage.getItem('drivesync-theme');
      return saved ? saved === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches;
    } catch {
      return false;
    }
  });
  const [menuOpen, setMenuOpen] = useState(false);
  useEffect(() => {
    document.documentElement.classList.toggle('dark', dark);
    try {
      localStorage.setItem('drivesync-theme', dark ? 'dark' : 'light');
    } catch {/* Theme remains usable when storage is unavailable. */}
  }, [dark]);
  const navigation = <>{pages.map(item => <a key={item.id} aria-current={page.id === item.id ? 'page' : undefined} href={`${base}${item.path}`}>{t(item.label)}</a>)}</>;
  return <><a className="skip-link" href="#conteudo">{t("Pular para o conte\xFAdo")}</a><div className="site-shell"><header className="site-header"><a className="brand" href={base}><img src={`${base}icon.svg`} width="36" height="36" alt="" /><span>{t("DriveSync")}</span></a><nav className="desktop-nav" aria-label={t("Principal")}>{navigation}</nav><div className="header-actions"><LanguageSwitcher /><Button variant="ghost" size="icon" aria-label={t(dark ? 'Usar tema claro' : 'Usar tema escuro')} onClick={() => setDark(!dark)}>{dark ? <Sun /> : <Moon />}</Button><Sheet open={menuOpen} onOpenChange={setMenuOpen}><SheetTrigger className="mobile-menu" render={<Button variant="ghost" size="icon" aria-label={t("Abrir navega\xE7\xE3o")} />}><Menu /></SheetTrigger><SheetContent><SheetTitle>{t("DriveSync")}</SheetTitle><SheetDescription>{t("Cliente do Google Drive para Linux.")}</SheetDescription><nav className="mobile-nav" aria-label={t("Navega\xE7\xE3o m\xF3vel")}>{navigation}</nav></SheetContent></Sheet></div></header><main id="conteudo">{page.id === 'development' ? <Development /> : page.id === 'donations' ? <Donations /> : page.id === 'privacy' ? <Privacy /> : page.id === 'terms' ? <Terms /> : <Application />}</main><footer className="site-footer"><div><a className="footer-brand" href={base}>{t("DriveSync")}</a><p>{t("Cliente do Google Drive para Linux.")}<br />{t("Projeto independente, sem afilia\xE7\xE3o com o Google.")}</p></div><nav aria-label={t("Rodap\xE9")}><a href={`${base}privacidade/`}>{t("Privacidade")}</a><a href={`${base}termos/`}>{t("Termos de Serviço")}</a><a href={`${base}doacoes/`}>{t("Doa\xE7\xF5es")}</a><a href={repo}><Code2 size={15} /> {t("GitHub")}</a></nav><span className="footer-note">{t("Em desenvolvimento. Com espa\xE7o para voc\xEA.")}</span></footer></div></>;
}
