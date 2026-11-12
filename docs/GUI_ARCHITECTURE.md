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
mostra o apelido e o e-mail. A seleção identifica a conta no cabeçalho; a área
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
