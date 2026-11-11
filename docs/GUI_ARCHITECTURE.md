# Interface do DriveSync

O aplicativo tem um setup inicial, um dashboard vazio e Preferências. Os widgets
ficam em `src/ui`; a coordenação da aplicação fica em `src/application`, e os
serviços de desktop e persistência de escolhas em `src/services`. A comunicação
com Google e as credenciais passam exclusivamente pela API pública da engine.

## Setup

Uma única janela apresenta seleção de provedor, conexão, pastas e revisão. A
primeira página combina uma ilustração à esquerda com o catálogo de provedores à
direita. As outras compartilham o mesmo cabeçalho vertical: área de arte de 184
pixels lógicos, título centralizado, descrição e componentes da etapa. Os ícones
Google Drive e DriveSync usam 176 pixels; a revisão usa a ilustração maior.

Todas as etapas compartilham o mínimo de 760 × 800 pixels lógicos e podem ser
ampliadas. O mínimo pertence ao conteúdo do setup. A navegação não redefine o
tamanho escolhido pelo usuário e usa uma transição de 180 ms que respeita as
preferências de animação do GTK. Os botões Cancelar e Continuar ficam no mesmo
rodapé. A seleção remota usa um componente genérico com um adaptador da engine.

As escolhas incompletas ficam somente em memória. Concluir executa validações
frescas, salva as escolhas e abre o dashboard na mesma janela. Fechar ou cancelar
antes disso desconecta a conta provisória e descarta as escolhas. O fechamento
aguarda uma conclusão já em andamento para evitar corrida com a gravação.

## Configuração concluída

O contrato, a persistência privada por conta, a recuperação de falhas e os limites
da validação estão em [SETUP_COMPLETION.md](SETUP_COMPLETION.md). Uma configuração
concluída nunca é descartada automaticamente por falha de rede ou necessidade de
reautorização. Rascunhos legados não liberam acesso ao dashboard.

## Dashboard e Preferências

O dashboard apresenta somente um cabeçalho com o menu do aplicativo e uma área de
conteúdo vazia. Não permite voltar ao setup nem editar configurações. Em
Preferências, Configuração edita as pastas com Salvar/Cancelar; Contas reúne nome,
autorização, desconexão e adição. Aparência, segundo plano e Sobre continuam
separados desses fluxos. Não há processo de sincronização iniciado ao concluir.

## Recursos visuais e testes

Os ícones da interface ficam em `data/icons`, as fontes editáveis em
`data/icons/source` e as ilustrações em `data/illustrations`. O ícone do provedor
Google permanece em `engine/src/providers/google/assets/drive.svg`, fornecido em
`RegisteredProvider::icon_svg`; não depende de nomes no tema instalado.

Os fixtures usam conta fictícia, respostas HTTP sintéticas e perfis XDG isolados.
Conferem alinhamento, ausência de cortes no mínimo comum, limpeza de setup
incompleto, persistência da conclusão e dashboard vazio. As capturas desativam
animações apenas no processo de teste para registrar o estado final. Ver
[data/screenshots/README.md](../data/screenshots/README.md).
