# Capturas da interface

O teste `account_only_layout_and_initial_name_editor` monta widgets reais com
contas fictícias. Verifica o dashboard vazio, a edição de contas em Preferências e
o Setup ocupando uma única janela. Não inicia OAuth real, keyring ou autostart.
Os testes ficam em `src/application/tests.rs` e `src/ui/setup/tests.rs`.

Os fixtures exigem perfil XDG e display isolados. Dentro do SDK/Flatpak definido
no manifesto, com sua extensão Rust disponível:

```sh
ui_profile=$(mktemp -d /tmp/drivesync-ui-test-XXXXXX)
export XDG_DATA_HOME="$ui_profile/data"
export XDG_CONFIG_HOME="$ui_profile/config"
export XDG_CACHE_HOME="$ui_profile/cache"
mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME"
export DRIVESYNC_UI_PREVIEW_DIR=/caminho/absoluto/para/capturas
export GTK_A11Y=none GDK_BACKEND=broadway BROADWAY_DISPLAY=:17 GSK_RENDERER=cairo
gtk4-broadwayd :17 >"$ui_profile/broadway.log" 2>&1 &
broadway_pid=$!
trap 'kill "$broadway_pid"' EXIT
dbus-run-session -- cargo test -p drivesync --bin drivesync \
  account_only_layout_and_initial_name_editor -- --ignored --test-threads=1
```

O diretório de saída deve estar acessível ao Flatpak. São geradas:

- `dashboard-empty.png`, `dashboard-completed.png`, `new-account.png` (editor em Preferências) e `preferences.png`;
- `setup-provider.png` e `setup-connection.png`;
- `setup-synchronization.png` e `setup-review.png`;
- `setup-review-small-dark.png`;
- `setup-single-window-review.png` e `setup-fresh-start.png`.

Sem a variável de saída, o teste verifica os widgets sem salvar as imagens.
A conclusão valida e salva as escolhas e abre o dashboard vazio. O fixture usa
contas e pastas fictícias; não inicia sincronização nem consentimento real. Não há captura de tela intermediária ou de contas pendentes: essas
telas foram removidas. As escolhas incompletas são mantidas em memória e descartadas
junto com a conexão provisória ao fechar sem concluir.

O teste cria arquivos legados somente no perfil temporário para verificar sua
limpeza. As capturas não comprovam integração Google, portais ou funcionamento do
aplicativo instalado. A arquitetura atual está em
[docs/GUI_ARCHITECTURE.md](../../docs/GUI_ARCHITECTURE.md).

O fixture `live_account_selection_and_detector_layout` usa duas sessões reais do detector com pastas temporárias e identidades fictícias. No mesmo ambiente isolado, gera `detector-local-personal.png` e `detector-local-narrow.png` quando DRIVESYNC_UI_PREVIEW_DIR estiver definido. Verifica criação de eventos, troca de conta sem mistura de histórico e parada isolada; não usa Google, keyring ou fila persistente.
