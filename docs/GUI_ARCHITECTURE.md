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

A página Detector tem seleção por AccountId e abas Local/Remote. Ao abrir a página, a aplicação carrega as contas com setup concluído e cria sessões reais de observação local pelo Orchestrator. Cada conta possui seu próprio escopo, status, contagem e histórico; trocar a seleção troca imediatamente a origem exibida. As sessões pertencem ao Context da aplicação e continuam ativas quando a console é fechada, sem iniciar sincronização de arquivos.

O botão de recarregar relê contas/pastas, encerra escopos removidos ou alterados e cria os novos. Uma falha de leitura preserva as sessões existentes. A desconexão encerra a observação da conta antes de remover sua configuração; gerações impedem que uma inicialização concorrente antiga a reinsira. Se a desconexão falhar, recarregar permite iniciar novamente o diagnóstico. Mudanças de pasta em Preferências são aplicadas à inspeção ao recarregar/reabrir a página. O encerramento da aplicação libera as sessões fora da thread GTK.

A engine fornece o status, o total de ocorrências desde o início da sessão, o horário da última e até 100 resumos de evento. Não há números de demonstração. A interface preserva os tipos reais, incluindo RecheckRequired: não transforma pistas nativas em CREATE/MODIFY/RENAME comprovados. Pending events aparece como indisponível enquanto não existe telemetria de fila persistente; o histórico volátil não é fila de trabalho. Resumos não incluem conteúdo nem payload de metadados. Reiniciar/trocar a sessão zera seu diagnóstico.

A atualização visual a cada 500 ms lê snapshots em memória e só redesenha dados alterados; não agenda observação nem consulta remota. Remote informa que não está implementado. Quando existir, intervalo, última/próxima consulta, duração, alterações retornadas, cursor, eventos e recuperação deverão vir da engine por conta. Um eventual temporizador apenas apresenta o próximo ciclo informado pela engine, sem manter um agendador independente na interface.

Validação desta entrega: fmt, Clippy com warnings negados, testes do workspace (587 aprovados, 7 ignorados por ambiente), rustdoc e 4 doctests passaram no org.gnome.Sdk//50. O fixture gráfico foi depois executado separadamente via Broadway/D-Bus, com perfil XDG isolado: 1 aprovado, incluindo seleção entre duas contas, eventos reais, parada isolada e layout de 360 pixels; restam 6 testes ambientais não exercitados nesta entrega. Capturas foram inspecionadas em `_build/detector-preview/`. Não houve teste com contas reais Google, aplicação instalada ou fila persistente.

### Acesso nativo às pastas no desenvolvimento Flatpak

O caminho do Document Portal permite leitura/escrita, mas não retransmite notificações inotify de mudanças feitas no caminho original por outros processos. Isso foi reproduzido com exportação temporária: o mesmo arquivo ficou legível pelo portal, mas somente o watch do caminho nativo recebeu a criação externa.

O manifesto de desenvolvimento concede acesso somente a `~/GoogleDrive`, onde estão as pastas atualmente configuradas, por `--filesystem=~/GoogleDrive`. O Builder precisa recriar o ambiente de execução após mudar finish-args. Não foi concedido acesso global a home/host.

O serviço local_folder consulta o atributo documentado `user.document-portal.host-path` (via GIO xattr) somente para caminhos do portal. Exibe o caminho original em Setup/Preferências e escolhe a raiz nativa para o detector apenas se ela estiver acessível dentro do sandbox. A URI persistida e as concessões do portal são preservadas. Sem permissão nativa, a conta fica indisponível para monitoramento, com diagnóstico próprio, em vez de alegar captura funcional. Pastas em outros locais exigem concessão específica equivalente no manifesto de desenvolvimento; essa resolução não cria permissões nem fornece solução de distribuição para pastas arbitrárias.

Validação da correção do portal: 589 testes e 4 doctests passaram no SDK Flatpak, além de fmt, Clippy e rustdoc. O teste opt-in `portal_display_and_native_watch_permission` foi executado em duas instâncias isoladas do SDK com uma exportação temporária: sem acesso direto, confirmou erro e caminho legível; com `--filesystem=~/GoogleDrive`, confirmou que uma criação pelo host chegou ao detector real. A pasta temporária e sua exportação foram removidas, sem alterar arquivos das contas. O teste gráfico por conta também passou novamente via Broadway. O Builder real ainda precisa recriar o ambiente para incorporar os novos finish-args.

Referência para resolver o caminho sem modificar a URI: [Document Portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Documents.html).

### Ações classificadas no Detector

O produtor local agora fornece Created/Modified/Deleted/MetadataChanged e movimentos correlacionados. A página os apresenta como CREATE (verde), MODIFY (cor de destaque), DELETE (erro), METADATA, RENAME e MOVE (neutros); movimentos preservam a origem e o destino com seta. RECHECK continua visível para evidência insuficiente, com o escopo geral descrito como “Verify monitored folder”, em vez de apresentá-lo como arquivo. A página não infere o tipo da ação nem elimina eventos temporários pelo nome.

Alterações de arquivos comuns não reconstruem todos os watches e não geram recheck geral sistemático. A coleta inconclusiva de um temporário que já desapareceu também não sinaliza, sozinha, falha da cobertura do watcher. Problemas reais de cobertura, permissão e recursos continuam sendo relatados pela engine. Os testes gráficos usam criação, edição e rename reais para conferir os rótulos, a seta e o isolamento entre contas.

Validação da classificação: fmt, Clippy com warnings negados, testes do workspace (595 aprovados, 8 ambientais ignorados), rustdoc e 4 doctests passaram no SDK Flatpak. O fixture gráfico foi executado separadamente via Broadway com duas contas, criação/edição/rename reais e janela de 360 pixels; passou e as capturas foram inspecionadas em `_build/detector-classified-preview/`. Não foram testados overflow real do kernel, todos os editores, todos os interleavings nem sincronização com Google.

### Normalização anterior ao Orchestrator

A engine normaliza notificações por ciclos de escrita antes do sink e do histórico do Orchestrator. O padrão usa 80 ms de quietude, limitado a 500 ms desde a primeira pista. Uma origem criada e ainda não entregue pode ser agrupada à movimentação comprovada para o arquivo final; não se filtra pelo nome. Gravações com fechamentos distintos continuam distintas. Escrita aberta no teto produz recheck com motivo e, quando fecha, pode produzir sua ocorrência concluída. Não se promete inferir todas as transações de todos os editores ou agrupar pausas arbitrariamente longas.

FileEvent.uncertainty acompanha cada RECHECK, e a página apresenta a explicação ao lado do caminho: captura inicial, reconstrução, cobertura, notificações perdidas, limites, movimento sem par, leitura instável, escrita em andamento ou sequência conflitante. A UI não calcula essas causas. O fixture gráfico verifica também uma substituição atômica como apenas mais um MODIFY no histórico da conta, sem mostrar a origem intermediária.

Validação integrada desta normalização no org.gnome.Sdk//50: fmt, Clippy com warnings negados, 606 testes, rustdoc e 4 doctests passaram; 8 testes ambientais ficaram ignorados na suíte padrão. O fixture gráfico foi executado separadamente via Broadway, passou e teve as capturas inspecionadas em `_build/detector-normalized-preview/`. Incluiu substituição com origem de nome comum, salvamento real por GIO replace_contents, motivos explícitos e isolamento entre duas contas. Ambos os salvamentos acrescentaram apenas um Modified cada. O teste de largura usa uma janela nova de 360 pixels, pois alterar o tamanho padrão de uma janela já mapeada não garante redimensionamento pelo compositor. Não foram exercitados todos os editores, overflow real do kernel, fila durável ou Google real.

O Event stream diferencia entradas por ícones com tooltip e nome acessível: arquivo, pasta, link simbólico, entrada especial e tipo desconhecido. O tipo vem dos metadados conhecidos do File, exposto por DetectorEventSummary.entry_kind; não se infere por extensão ou nome. Avisos de WholeSync usam um ícone de escopo. Eventos sem metadados, inclusive remoções sem snapshot, mantêm tipo desconhecido. Validação no SDK: fmt, Clippy, 606 testes, 4 doctests e geração de documentação passaram. O fixture gráfico também passou com arquivo e pasta reais, duas contas e largura de 360 pixels; a captura foi inspecionada.

### Fila persistente e interrupção de entrega no Developer

As sessões reais do Developer agora usam Orchestrator::observe_persistent e SQLite na área privada `data/drivesync/detector-queues` do aplicativo, separada por conta/SyncId/raiz. Reabrir recupera o histórico recente, as contagens e a política de entrega. Configurar outra pasta cria outro escopo; a fila anterior não é apagada ou atribuída à nova pasta. Desconectar encerra a captura, mas não limpa silenciosamente registros persistidos.

O botão de pause/play ao lado de recarregar (tooltips “Interrupt delivery” / “Resume delivery”) atua somente na conta selecionada, com I/O fora da thread GTK. Durante a interrupção, captura e gravação continuam e Pending events cresce. O Event stream mostra observações capturadas, inclusive durante pause; ele não é uma lista de operações sincronizadas. A página mostra entrega, reservas, confirmações diagnósticas e último erro da fila. Ao retomar, o consumidor terminal diagnóstico reserva e confirma os lotes; não há Inventory, upload/download ou sucesso de sincronização implícito. O modo PersistentDetector público oferece consumo explícito e não confirma sozinho.

A interrupção também é persistida: reiniciar a aplicação não reabilita uma fila que estava pausada. O teste gráfico exercita o botão, grava durante pause, verifica o contador, troca de conta e retoma. A integração da engine testa reabertura com pause/histórico preservados e saída abrupta de subprocesso sem depender de destrutores.

Validação da fila nesta interface: 618 testes do workspace e 4 doctests passaram no org.gnome.Sdk//50, além de fmt, Clippy e rustdoc. O fixture gráfico foi executado separadamente via Broadway/XDG isolados: pausou a conta pessoal, criou um evento, observou Pending=1 e a outra conta habilitada, retomou e observou Pending=0. O layout de 360 pixels passou. Capturas inspecionadas em `_build/detector-queue-preview/`, incluindo `detector-delivery-paused.png`. As confirmações são diagnósticas; backup Configuration, sincronização com Google e o aplicativo instalado não foram exercitados.

Cada conta na sidebar oferece apenas **Abrir pasta local**, **Abrir pasta no Google Drive** e **Sair da conta**. As ações pertencem à conta da linha, independentemente da seleção. A abertura local usa a URI autorizada pelo portal; a remota usa o identificador da pasta configurada. Sair exige confirmação, interrompe o detector da conta e reutiliza o fluxo de desconexão, preservando os arquivos locais e remotos.

A página Detector prioriza os eventos: seletor de conta, recarregar e pause/play compartilham uma linha, seguidos imediatamente por Local/Remote. Os quatro indicadores ficam em uma linha nas janelas largas e se distribuem em mais linhas conforme a largura disponível. Contas longas são abreviadas no seletor, com o texto completo no tooltip. Os controles e indicadores permanecem visíveis enquanto apenas a região dos eventos rola.

A aba **Remote** agora usa observação Google real por conta/raiz, iniciada junto da sessão local, com fila e entrega diagnóstica independentes. A UI apresenta intervalo, última consulta, próxima consulta informada pela engine, duração, atividade/cobertura, fingerprint do cursor, alterações retornadas/eventos produzidos, falhas e recuperação. A interface não agenda nem calcula um ciclo próprio. O play/pause atua na origem selecionada: pausa de entrega remota mantém polling e gravação durável, preservando a origem local. Encerrar/remover uma sessão cancela ambas as capturas; erros remotos aparecem separadamente do estado local. O consumidor permanece terminal diagnóstico; validação da coordenação completa e sincronização ficam para etapa posterior.

Na aba Remote, intervalo, última/próxima consulta e falha atual permanecem visíveis. Os detalhes de polling podem ser expandidos; iniciam recolhidos para preservar a área dos eventos também a 360 pixels.
