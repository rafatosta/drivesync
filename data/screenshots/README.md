# DriveSync — telas demonstrativas

Capturas históricas com dados fictícios e contas de Marina para uso pessoal,
trabalho e estudos. Elas não representam necessariamente a interface atual.
Os endereços `example.com`, nomes de arquivos e caminhos de demonstração não
representam dados reais.

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

Estas imagens mostram componentes da implementação anterior, removidos da
interface atual. As larguras são 720 e 360 px.

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

A interface atual gerencia contas e pastas, mas não sincroniza arquivos nem
oferece upload, resolução de conflitos ou tratamento de exclusões. A escolha da
pasta local usa o seletor do sistema. A autorização Google ocorre no navegador.

## Gerar novamente

O teste visual `demonstration_screenshots` gera capturas da interface de contas
com dados fictícios, em tamanhos amplos e compactos. Ele não regenera as imagens
históricas de sincronização. Ele é ignorado na execução normal dos testes.

Requer uma sessão gráfica acessível e o SDK `org.gnome.Sdk//50` com a extensão `org.freedesktop.Sdk.Extension.rust-stable` correspondente, instalados pelo Flathub conforme o [manifesto de desenvolvimento](../../flatpak/com.rtosta.DriveSync.Devel.yml). O comando abaixo usa o Cargo e as bibliotecas GTK4/Libadwaita do SDK; não é necessário instalar Rust/Cargo no host.

Execute a partir da raiz do projeto:

```sh
demo_build_dir="$(mktemp -d /tmp/drivesync-screenshot-sdk-XXXXXX)" || exit 1
flatpak build-init "$demo_build_dir" \
  com.rtosta.DriveSync.Devel org.gnome.Sdk org.gnome.Platform 50 || exit 1

flatpak build \
  --filesystem="$PWD" \
  --share=network --share=ipc \
  --socket=wayland --socket=fallback-x11 \
  --own-name=com.rtosta.DriveSync.Screenshots \
  --env=PATH=/usr/lib/sdk/rust-stable/bin:/usr/bin \
  "$demo_build_dir" sh -c '
    cd "$1" || exit 1
    XDG_DATA_HOME="$(mktemp -d /tmp/drivesync-ui-demo-XXXXXX)" || exit 1
    export XDG_DATA_HOME
    export DRIVESYNC_UI_PREVIEW_DIR="$PWD/data/screenshots"
    export GSK_RENDERER=cairo
    cargo test -p drivesync demonstration_screenshots -- --ignored --test-threads=1
  ' sh "$PWD"
```

- `flatpak build-init` prepara um ambiente temporário com a identidade `com.rtosta.DriveSync.Devel`, usando o SDK do manifesto. Essa identidade permite ao Glycin carregar o SVG embutido em uma execução de desenvolvimento sem instalar o aplicativo. Executar diretamente `org.gnome.Sdk//50` pode falhar ao carregar o SVG e mostrar um ícone de download no lugar do ícone do DriveSync.
- `--filesystem="$PWD"` permite ler o projeto e gravar as capturas; os sockets e `--share=ipc` dão acesso à sessão gráfica. `--share=network` permite baixar dependências Cargo durante a compilação.
- `--own-name=com.rtosta.DriveSync.Screenshots` permite registrar o aplicativo de demonstração no D-Bus da sessão; sem essa permissão, `app.register()` pode falhar com `ServiceUnknown`.
- `--env=PATH=...` seleciona as ferramentas Rust da extensão do SDK.
- `XDG_DATA_HOME` cria um perfil temporário isolado. O prefixo `/tmp/drivesync-ui-demo-` é obrigatório e verificado pelo teste; não use o perfil real do aplicativo.
- `DRIVESYNC_UI_PREVIEW_DIR` define o destino dos PNGs. O diretório é criado se necessário e as imagens com os mesmos nomes são substituídas. O destino padrão desta documentação é `data/screenshots`; para uma saída descartável, use outro caminho absoluto.
- `GSK_RENDERER=cairo` seleciona o renderizador Cairo para as capturas.
- `--ignored` inclui o teste visual opt-in; `--test-threads=1` executa os testes selecionados em uma única thread, conforme necessário para o GTK.

Ao terminar, confira o resultado do teste no terminal e abra os PNGs ou os links deste índice. O perfil temporário é criado no `/tmp` interno do sandbox Flatpak. O diretório de desenvolvimento temporário fica no host, em `$demo_build_dir`, e pode ser removido após a geração.

O gerador não instala tarefas automáticas, não inicia OAuth e não executa as ações de sincronizar ou sair da conta. As capturas validam a apresentação dos componentes, sem comprovar uma sincronização real com o Google Drive.
