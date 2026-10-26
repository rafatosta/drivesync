# DriveSync — telas demonstrativas

Capturas dos componentes GTK/Libadwaita atuais, com dados fictícios e contas de Marina para uso pessoal, trabalho e estudos. Os endereços `example.com`, nomes de arquivos e caminhos de demonstração não representam dados reais.

As imagens mostram apresentações simuladas: nenhuma autenticação, transferência ou operação no Google Drive é executada. Não são evidência de sincronização real.

## Telas completas

As versões de 1200 px mostram o aplicativo em uma janela ampla; as de 390 px mostram a navegação compacta.

| Tela | Janela ampla | Compacta |
| --- | --- | --- |
| Boas-vindas | [PNG](01-boas-vindas-1200.png) | [PNG](01-boas-vindas-390.png) |
| Conectar conta Google | [PNG](02-conectar-conta-1200.png) | [PNG](02-conectar-conta-390.png) |
| Aguardar autorização no navegador | [PNG](03-autorizacao-navegador-1200.png) | [PNG](03-autorizacao-navegador-390.png) |
| Dashboard com três contas — pessoal | [PNG](04-dashboard-pessoal-1200.png) | [PNG](04-dashboard-pessoal-390.png) |
| Dashboard — trabalho | [PNG](06-dashboard-trabalho-1200.png) | [PNG](06-dashboard-trabalho-390.png) |
| Ações da conta — contexto | [PNG](07-acoes-conta-1200.png) | [PNG](07-acoes-conta-390.png) |
| Menu de ações da conta | [PNG](07b-menu-acoes-conta-1200.png) | [PNG](07b-menu-acoes-conta-390.png) |
| Confirmação de saída | [PNG](08-confirmar-sair-1200.png) | [PNG](08-confirmar-sair-390.png) |
| Reconectar conta e pasta indisponível | [PNG](09-conta-reconectar-pasta-indisponivel-1200.png) | [PNG](09-conta-reconectar-pasta-indisponivel-390.png) |
| Escolher pasta do Google Drive | [PNG](10-selecionar-pasta-drive-1200.png) | [PNG](10-selecionar-pasta-drive-390.png) |
| Menu principal — contexto | [PNG](11-menu-principal-1200.png) | [PNG](11-menu-principal-390.png) |
| Opções do menu principal | [PNG](11b-opcoes-menu-principal-1200.png) | [PNG](11b-opcoes-menu-principal-390.png) |
| Configuração inicial das pastas | [PNG](13-configuracao-inicial-pastas-1200.png) | [PNG](13-configuracao-inicial-pastas-390.png) |

[Lista de múltiplas contas na navegação compacta](05-lista-multiplas-contas-390.png) · [Sobre o DriveSync](12-sobre-drivesync.png)

Menus são capturados também separadamente porque o GTK os desenha em uma superfície própria.

## Sincronização e transferências

Estas imagens isolam os componentes reais de sincronização para mostrar seus estados e mensagens. As larguras são 720 e 360 px.

| Recurso / estado | Amplo | Compacto |
| --- | --- | --- |
| Análise antes da transferência | [PNG](download-analysis-720.png) | [PNG](download-analysis-360.png) |
| Download em andamento | [PNG](download-progress-720.png) | [PNG](download-progress-360.png) |
| Upload em andamento | [PNG](upload-progress-720.png) | [PNG](upload-progress-360.png) |
| Resultado, itens ignorados e arquivos preservados | [PNG](download-result-720.png) | [PNG](download-result-360.png) |
| Histórico recolhido | [PNG](download-history-collapsed-720.png) | [PNG](download-history-collapsed-360.png) |
| Histórico expandido: movimentação e exclusão remota | [PNG](download-history-expanded-720.png) | [PNG](download-history-expanded-360.png) |
| Arquivos atualizados | [PNG](automatic-current-720.png) | [PNG](automatic-current-360.png) |
| Sem conexão | [PNG](automatic-offline-720.png) | [PNG](automatic-offline-360.png) |
| Nova tentativa automática | [PNG](automatic-retry-720.png) | [PNG](automatic-retry-360.png) |
| Sincronização pausada / retomar | [PNG](automatic-paused-720.png) | [PNG](automatic-paused-360.png) |
| Atenção necessária | [PNG](automatic-blocked-720.png) | [PNG](automatic-blocked-360.png) |
| Upload desativado | [PNG](upload-mode-disabled-720.png) | [PNG](upload-mode-disabled-360.png) |
| Upload de novos arquivos ativado | [PNG](upload-mode-enabled-720.png) | [PNG](upload-mode-enabled-360.png) |

## Escopo atual

O upload é opcional e cria somente novos arquivos e pastas. As imagens de atenção e histórico mostram preservação de dados, sem uma tela completa de resolução de conflitos. Não há telas atuais de preferências, integração com o Nautilus ou seleção individual de subpastas. A escolha da pasta local usa o seletor do sistema, que varia conforme o desktop e não está incluído. A autorização Google ocorre no navegador; aqui é exibida a janela do DriveSync que aguarda essa autorização.

## Gerar novamente

Em um ambiente com Rust, GTK4, Libadwaita e uma sessão gráfica disponíveis, execute a partir da raiz do projeto:

```sh
XDG_DATA_HOME="$(mktemp -d /tmp/drivesync-ui-demo-XXXXXX)" \
DRIVESYNC_UI_PREVIEW_DIR="$PWD/screenshots" \
GSK_RENDERER=cairo \
cargo test -p drivesync demonstration_screenshots -- --ignored --test-threads=1
```

O perfil temporário é obrigatório. O gerador não instala tarefas automáticas, não inicia OAuth e não executa as ações de sincronizar ou sair da conta.
