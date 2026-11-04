# Capturas da interface

O teste visual `demonstration_screenshots` gera capturas da interface de contas
com dados fictícios, sem autenticação ou operações no Google Drive. Ele é
ignorado na execução normal e requer um display e um perfil isolado.

Use o SDK e a extensão Rust definidos em
[`flatpak/com.rtosta.DriveSync.Devel.yml`](../../flatpak/com.rtosta.DriveSync.Devel.yml).
No ambiente Flatpak de desenvolvimento, defina `XDG_DATA_HOME` como um diretório
temporário cujo caminho comece com `/tmp/drivesync-ui-demo-`, e
`DRIVESYNC_UI_PREVIEW_DIR` como um diretório de saída descartável. Execute:

```sh
cargo test -p drivesync demonstration_screenshots -- --ignored --test-threads=1
```

O comando deve ser executado **dentro do SDK Flatpak**, não no host. As
capturas são apenas evidência visual; elas não validam OAuth real, portais ou o
aplicativo instalado.
