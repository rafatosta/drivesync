# Fontes técnicas de referência

- [OAuth para aplicativos desktop](https://developers.google.com/identity/protocols/oauth2/native-app),
  [PKCE RFC 7636](https://www.rfc-editor.org/rfc/rfc7636) e
  [aplicativos nativos RFC 8252](https://www.rfc-editor.org/rfc/rfc8252).
- [Escopos do Google Drive](https://developers.google.com/workspace/drive/api/guides/api-specific-auth)
  e [consulta de pastas](https://developers.google.com/workspace/drive/api/guides/search-files).
- [Rust `rename`](https://doc.rust-lang.org/std/fs/fn.rename.html), cujas
  garantias dependem do filesystem e do contexto.
- [GApplication hold](https://docs.gtk.org/gio/method.Application.hold.html) e
  [Background portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Background.html).

Uma referência não comprova o comportamento da implementação. Identificar os
limites de cada fonte e verificar integração e falhas no ambiente alvo.
