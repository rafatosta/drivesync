# Interface do DriveSync

O aplicativo tem um setup inicial, um dashboard com navegação de contas e Preferências. Os widgets
ficam em `src/ui`; a coordenação da aplicação fica em `src/application`, e os
serviços de desktop e persistência de escolhas em `src/services`. A comunicação
com Google e as credenciais passam exclusivamente pela API pública da engine.

## Setup

Sem contas configuradas, a janela principal apresenta seleção de provedor,
conexão, pastas e revisão. Com uma conta configurada, adicionar outra abre uma
janela de setup separada, sem ocultar ou substituir o dashboard. Fechar essa
janela encerra somente o setup. A primeira página combina uma ilustração à esquerda com o catálogo de provedores à
direita. As outras compartilham o mesmo cabeçalho vertical: área de arte de 120
pixels lógicos, título centralizado, descrição e componentes da etapa. Os ícones
Google Drive e DriveSync usam 112 pixels; a revisão usa uma ilustração horizontal de 224 × 120 pixels.

O setup abre em 1000 × 720 pixels lógicos, em formato horizontal. Todas as etapas
compartilham o mínimo de 900 × 720 pixels lógicos e podem ser ampliadas. O mínimo pertence ao conteúdo do setup. A navegação não redefine o
tamanho escolhido pelo usuário e usa uma transição de 180 ms que respeita as
preferências de animação do GTK. Os botões Cancelar e Continuar ficam no mesmo
rodapé. A seleção remota usa um componente genérico com um adaptador da engine.

As escolhas incompletas ficam somente em memória. Concluir executa validações
frescas e salva as escolhas. No primeiro cadastro, abre o dashboard na mesma
janela; nos seguintes, fecha somente a janela de setup. Fechar ou cancelar
antes disso desconecta a conta provisória e descarta as escolhas. O fechamento
aguarda uma conclusão já em andamento para evitar corrida com a gravação.

## Configuração concluída

O contrato, a persistência privada por conta, a recuperação de falhas e os limites
da validação estão em [SETUP_COMPLETION.md](SETUP_COMPLETION.md). Uma configuração
concluída nunca é descartada automaticamente por falha de rede ou necessidade de
reautorização. Rascunhos legados não liberam acesso ao dashboard.

## Dashboard e Preferências

O dashboard usa `AdwNavigationSplitView` com uma sidebar nativa de contas
(`GtkListBox` com `navigation-sidebar`, `AdwActionRow` e `AdwAvatar`). Cada linha
mostra o apelido e o e-mail. O rodapé fixo exibe a versão e o commit abreviado
do mesmo identificador usado em Sobre; o tooltip contém a revisão completa.
A seleção identifica a conta no cabeçalho; a área
principal continua vazia. A seleção é preservada por ID ao atualizar a lista.
Abaixo de 660 pixels lógicos, sidebar e conteúdo usam navegação adaptável com
botão de retorno. A lista inclui apenas contas com configuração concluída,
preservando também as que precisam de reautorização. Ela é recarregada ao
aparecer e após concluir a adição de outra conta. Falhas de leitura são exibidas
sem apagar a lista anterior. O cabeçalho permite adicionar outra conta. Em
Preferências, Contas reúne apelido, pastas com Salvar/Cancelar,
autorização, desconexão e adição. Aparência, segundo plano e Sobre continuam
separados desses fluxos. Não há processo de sincronização iniciado ao concluir.

## Recursos visuais e testes

Os ícones da interface ficam em `data/icons`, as fontes editáveis em
`data/icons/source` e as ilustrações em `data/illustrations`. O ícone do provedor
Google permanece em `engine/src/providers/google/assets/drive.svg`, fornecido em
`RegisteredProvider::icon_svg`; não depende de nomes no tema instalado.

Os fixtures usam conta fictícia, respostas HTTP sintéticas e perfis XDG isolados.
Conferem alinhamento, ausência de cortes no mínimo comum, limpeza de setup
incompleto, persistência da conclusão, seleção de contas e navegação compacta. As capturas desativam
animações apenas no processo de teste para registrar o estado final. Ver
[data/screenshots/README.md](../data/screenshots/README.md).

## Developer / Detector

A aplicação inicia as sessões das contas configuradas durante a restauração,
independentemente de abrir o console. Local e Remote capturam em fila persistente
compartilhada com entrega suspensa. O Orchestrator aguarda a captura local e
executa o Scanner no inventário. A página Detector apenas acompanha essas mesmas
sessões por conta, sem consumir ou confirmar ocorrências.

Recarregar relê contas/pastas e reutiliza sessões existentes; mudança de escopo
ou desconexão cancela a preparação e para as fontes antigas. Falha de leitura
preserva dados. O encerramento libera tarefas/recursos fora da thread GTK.

Eventos são observações brutas, sem inferir CREATE/MODIFY/RENAME. Indicadores e
últimos 100 eventos por origem vêm da engine. Snapshot visual não agenda polling
remoto nem faz SQL. O botão pause/play do consumidor diagnóstico foi removido.
Até conectar Analyzer/ingestão, a entrega permanece Paused mesmo após scan
completo; fila e observação continuam ativas.

### Acesso nativo às pastas no desenvolvimento Flatpak

O caminho do Document Portal permite leitura/escrita, mas não retransmite notificações inotify de mudanças feitas no caminho original por outros processos. Isso foi reproduzido com exportação temporária: o mesmo arquivo ficou legível pelo portal, mas somente o watch do caminho nativo recebeu a criação externa.

O manifesto de desenvolvimento concede acesso somente a `~/GoogleDrive`, onde estão as pastas atualmente configuradas, por `--filesystem=~/GoogleDrive`. O Builder precisa recriar o ambiente de execução após mudar finish-args. Não foi concedido acesso global a home/host.

O serviço local_folder consulta o atributo documentado `user.document-portal.host-path` (via GIO xattr) somente para caminhos do portal. Exibe o caminho original em Setup/Preferências e escolhe a raiz nativa para o detector apenas se ela estiver acessível dentro do sandbox. A URI persistida e as concessões do portal são preservadas. Sem permissão nativa, a conta fica indisponível para monitoramento, com diagnóstico próprio, em vez de alegar captura funcional. Pastas em outros locais exigem concessão específica equivalente no manifesto de desenvolvimento; essa resolução não cria permissões nem fornece solução de distribuição para pastas arbitrárias.

Validação da correção do portal: 589 testes e 4 doctests passaram no SDK Flatpak, além de fmt, Clippy e rustdoc. O teste opt-in `portal_display_and_native_watch_permission` foi executado em duas instâncias isoladas do SDK com uma exportação temporária: sem acesso direto, confirmou erro e caminho legível; com `--filesystem=~/GoogleDrive`, confirmou que uma criação pelo host chegou ao detector real. A pasta temporária e sua exportação foram removidas, sem alterar arquivos das contas. O teste gráfico por conta também passou novamente via Broadway. O Builder real ainda precisa recriar o ambiente para incorporar os novos finish-args.

Referência para resolver o caminho sem modificar a URI: [Document Portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Documents.html).

## Developer / Inventory

F12 → Inventory abre uma consulta somente leitura do banco da pasta configurada.
A página oferece seletor de conta, seletor das 11 tabelas do inventário, botão de
recarregar e paginação de até 100 registros. As colunas acompanham a tabela
selecionada, com rolagem horizontal e largura ajustável. A seção “Selected record”
mostra valores, tipos e tamanho dos campos da linha selecionada; ativar a linha
abre essa seção. Textos longos e BLOBs têm prévia limitada, com truncamento
explícito. Bytes não UTF-8 aparecem em hexadecimal; NULL é distinto de texto vazio.

A aplicação lê as configurações concluídas sem iniciar o Detector. Consultas
SQLite são executadas fora da thread GTK, pela API pública
`inventory::inspection`, sem SQL nos widgets. Cada mudança de seleção invalida
respostas antigas para não misturar contas/tabelas. Recarregar lê novamente as
configurações e a primeira página. Não há polling de tabelas em segundo plano.

A inspeção não cria inventário, não gera exemplos em contas reais e não alimenta
itens com eventos. Banco inexistente, tabela vazia, identidade incompatível,
esquema não suportado e indisponibilidade são apresentados explicitamente.
O arquivo existente é preservado. A alimentação pelo Analisador permanece uma
integração separada.

O teste gráfico `inventory_table_selection_pagination_and_account_isolation`
usa duas pastas sintéticas, SQLite real e uma sessão Wayland/perfil XDG isolados.
Valida paginação, troca de tabela/conta, resposta atrasada, detalhe, ausência sem
criação e janela estreita; não acessa credenciais nem contas reais. A inspeção
SQLite possui testes próprios no engine, incluindo leitura de arquivo 0400 e
comparação byte a byte antes/depois.

## Developer / Scanner

F12 → Scanner acompanha a preparação iniciada na abertura, por conta. Mostra
estado, raiz, arquivos/pastas observados, observações aceitas no inventário,
problemas, horários e diretório corrente. Não inventa percentual/ETA. Scan again
repete a preparação da conta selecionada, mantendo captura e fila pausadas.

Partial mantém análise suspensa e expõe os problemas. Scan complete significa
apenas inventário local apto à próxima etapa: a página explica que Analyzer ainda
não está conectado e que a entrega continua suspensa. Até 100 problemas do último
relatório desta execução são mostrados; não é histórico durável de ScanIssue.

A atualização a cada 250 ms lê somente snapshots de memória. Trocar conta muda
a fonte exibida; abrir/fechar o console não inicia/cancela sessões de backend.
Colunas de métricas se reorganizam em janelas estreitas; mensagens vazias não
ocupam linhas. A desconexão e o shutdown pertencem à aplicação.

Teste gráfico em perfil XDG e Wayland isolados cobre abertura real antes de
existir Developer Console, duas contas, parcial/completo, reuso sem rescan,
contagens, problemas, captura com entrega suspensa, rescan, 440 pixels e remoção.
O teste de Detector também foi reexecutado sem o antigo consumidor diagnóstico.
