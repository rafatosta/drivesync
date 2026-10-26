import { useEffect, useState } from 'react'
import { ArrowLeft, ArrowRight } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Screenshot } from '@/components/screenshot'

interface Capture {
  title: string
  description: string
  wide?: string
  compact?: string
}
const full = (stem: string, title: string, description: string): Capture => ({ title, description, wide: `${stem}-1200.png`, compact: `${stem}-390.png` })
const detail = (stem: string, title: string, description: string): Capture => ({ title, description, wide: `${stem}-720.png`, compact: `${stem}-360.png` })
const groups = [
  { title: 'Primeiros passos', items: [
    full('01-boas-vindas', 'Boas-vindas', 'A apresentação inicial do DriveSync e o acesso à conexão de uma conta Google.'),
    full('02-conectar-conta', 'Conectar uma conta', 'O início da conexão com sua conta Google.'),
    full('03-autorizacao-navegador', 'Autorização no navegador', 'O DriveSync aguarda a autorização. O login e o consentimento acontecem no navegador.'),
    full('13-configuracao-inicial-pastas', 'Configurar as pastas', 'A configuração inicial das pastas remota e local para uma conta.'),
    full('10-selecionar-pasta-drive', 'Escolher a pasta do Drive', 'Navegue pelas pastas da conta e escolha a pasta que deseja acompanhar.'),
  ] },
  { title: 'Contas e navegação', items: [
    full('04-dashboard-pessoal', 'Visão da conta pessoal', 'Uma conta com suas pastas e configurações de sincronização.'),
    { title: 'Lista de contas', description: 'A navegação entre contas em uma janela compacta.', compact: '05-lista-multiplas-contas-390.png' },
    full('06-dashboard-trabalho', 'Alternar para o trabalho', 'Outra conta, com configurações e pastas independentes.'),
    full('07-acoes-conta', 'Ações da conta', 'O contexto das ações disponíveis para uma conta.'),
    full('07b-menu-acoes-conta', 'Menu da conta', 'Detalhe do menu de ações da conta, capturado separadamente da janela.'),
    full('08-confirmar-sair', 'Confirmar a desconexão', 'A confirmação antes de desconectar uma conta do aplicativo.'),
    full('09-conta-reconectar-pasta-indisponivel', 'Conta que precisa reconectar', 'A apresentação de uma conta que exige reconexão e de uma pasta indisponível.'),
    full('11-menu-principal', 'Menu principal', 'O menu do aplicativo no contexto da janela.'),
    full('11b-opcoes-menu-principal', 'Opções do aplicativo', 'Detalhe das opções do menu principal, capturado em sua própria superfície.'),
    { title: 'Sobre o DriveSync', description: 'A janela com informações sobre o aplicativo.', wide: '12-sobre-drivesync.png' },
  ] },
  { title: 'Transferências', items: [
    detail('download-analysis', 'Antes da transferência', 'A análise dos arquivos antes de iniciar uma transferência.'),
    detail('download-progress', 'Download em andamento', 'O progresso de arquivos sendo baixados para o computador.'),
    detail('upload-progress', 'Envio em andamento', 'O progresso do envio opcional de novos arquivos e pastas.'),
    detail('download-result', 'Resultado da operação', 'O resultado da transferência, com itens ignorados e arquivos preservados.'),
    detail('download-history-collapsed', 'Resumo da atividade', 'O histórico recolhido mantém o resumo da atividade à vista.'),
    detail('download-history-expanded', 'Histórico em detalhe', 'O histórico expandido mostra movimentações e exclusões remotas, com preservação de arquivos locais.'),
  ] },
  { title: 'Estados e alertas', items: [
    detail('automatic-current', 'Arquivos atualizados', 'O estado apresentado quando não há novas transferências necessárias.'),
    detail('automatic-offline', 'Sem conexão', 'A indicação de que o aplicativo está sem conexão.'),
    detail('automatic-retry', 'Nova tentativa', 'A apresentação de uma nova tentativa automática.'),
    detail('automatic-paused', 'Sincronização pausada', 'A pausa da sincronização e a opção de retomar.'),
    detail('automatic-blocked', 'Atenção necessária', 'Um problema que precisa de atenção. A captura mostra preservação de dados, sem uma interface completa de resolução de conflitos.'),
    detail('upload-mode-disabled', 'Envio desativado', 'A configuração de download sem envio de arquivos ao Drive.'),
    detail('upload-mode-enabled', 'Envio de novos arquivos', 'O envio opcional ativado para novos arquivos e pastas; não representa sincronização completa nos dois sentidos.'),
  ] },
]

export function ScreenshotGallery() {
  useEffect(() => {
    if (window.location.hash === '#galeria') document.getElementById('galeria')?.scrollIntoView()
  }, [])
  const [groupIndex, setGroupIndex] = useState(0)
  const [captureIndex, setCaptureIndex] = useState(0)
  const [format, setFormat] = useState<'wide' | 'compact'>('wide')
  const group = groups[groupIndex]
  const capture: Capture = group.items[captureIndex]
  const selectedFormat = capture[format] ? format : capture.wide ? 'wide' : 'compact'
  const src = capture[selectedFormat]!
  return <section className="section gallery" id="galeria" aria-labelledby="gallery-title">
    <div className="section-heading horizontal"><div><p className="eyebrow">Conheça as telas</p><h2 id="gallery-title">O aplicativo, de perto.</h2></div><p>Da primeira conexão<br />aos detalhes da sincronização.</p></div>
    <div className="gallery-categories" role="group" aria-label="Assunto das capturas">{groups.map((item, index) => <Button key={item.title} variant={groupIndex === index ? 'secondary' : 'ghost'} aria-pressed={groupIndex === index} onClick={() => { setGroupIndex(index); setCaptureIndex(0) }}>{item.title}</Button>)}</div>
    <div className="gallery-toolbar"><span>{group.title} · {captureIndex + 1} de {group.items.length}</span><div role="group" aria-label="Tamanho da janela"><Button size="sm" variant={selectedFormat === 'wide' ? 'secondary' : 'ghost'} disabled={!capture.wide} aria-pressed={selectedFormat === 'wide'} onClick={() => setFormat('wide')}>Ampla</Button><Button size="sm" variant={selectedFormat === 'compact' ? 'secondary' : 'ghost'} disabled={!capture.compact} aria-pressed={selectedFormat === 'compact'} onClick={() => setFormat('compact')}>Compacta</Button></div></div>
    <div className={`gallery-stage gallery-${selectedFormat}`}><Screenshot src={src} title={capture.title} description={capture.description} /></div>
    <div className="gallery-selection"><h3 aria-live="polite">{capture.title}</h3><div><Button size="icon-sm" variant="outline" disabled={captureIndex === 0} aria-label="Captura anterior" onClick={() => setCaptureIndex(index => index - 1)}><ArrowLeft /></Button><Button size="icon-sm" variant="outline" disabled={captureIndex === group.items.length - 1} aria-label="Próxima captura" onClick={() => setCaptureIndex(index => index + 1)}><ArrowRight /></Button></div></div>
    <div className="gallery-thumbnails" role="group" aria-label={`Capturas de ${group.title.toLowerCase()}`}>{group.items.map((item: Capture, index) => <button type="button" key={item.title} className="gallery-thumbnail" aria-pressed={captureIndex === index} onClick={() => setCaptureIndex(index)}><img src={`${import.meta.env.BASE_URL}screenshots/${item.wide ?? item.compact}`} alt="" loading="lazy" /><span>{item.title}</span></button>)}</div>
    <p className="gallery-note">Capturas da interface atual com contas fictícias e estados simulados. Não representam operações reais no Google Drive. As versões compactas mostram a adaptação da janela, não um aplicativo para celular. O seletor de pasta local é fornecido pelo sistema e varia conforme o desktop.</p>
  </section>
}
