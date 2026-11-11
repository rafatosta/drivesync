# Capturas da interface de contas

O teste `account_only_layout_and_initial_name_editor` monta os widgets reais com contas fictícias e verifica boas-vindas, dashboard, edição de nome e preferências independentes da engine. Não inicia OAuth, keyring ou autostart.

Ele exige display e perfil isolados. Dentro do SDK/Flatpak definido no manifesto:

```sh
ui_profile=$(mktemp -d /tmp/drivesync-ui-test-XXXXXX)
export XDG_DATA_HOME="$ui_profile/data"
export XDG_CONFIG_HOME="$ui_profile/config"
export XDG_CACHE_HOME="$ui_profile/cache"
mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME"
export DRIVESYNC_UI_PREVIEW_DIR=/caminho/absoluto/para/capturas
export GTK_A11Y=none
dbus-run-session -- cargo test -p drivesync --bin drivesync \
  account_only_layout_and_initial_name_editor -- --ignored --test-threads=1
```

O diretório de saída precisa estar acessível ao Flatpak. São geradas `welcome.png`, `new-account.png` e `preferences.png` quando `DRIVESYNC_UI_PREVIEW_DIR` está definido. Sem essa variável, o teste verifica apenas os widgets. As capturas são evidência visual, não validação de conta Google real ou aplicativo instalado.

`data/icons/welcome.png` é uma exportação de 224×224 do SVG existente, usada para a apresentação inicial sem depender de um processo externo de carregamento SVG. A fonte vetorial permanece em `data/icons/hicolor/scalable/apps/com.rtosta.DriveSync.svg`.
