# Arquitetura atual do DriveSync

Registro descritivo do código presente no repositório em 2026-11-07. A
implementação foi tratada como fonte principal; a documentação existente foi
confrontada com ela. Cada afirmação técnica remete a arquivos e símbolos
verificáveis.

## Documentos

| Documento | Conteúdo |
| --- | --- |
| [SYSTEM_ARCHITECTURE.md](SYSTEM_ARCHITECTURE.md) | Crates, tecnologias, fronteiras e diagrama geral. |
| [MODULES.md](MODULES.md) | Diretórios, módulos, APIs principais e integração observada. |
| [DATA_MODEL.md](DATA_MODEL.md) | Persistência, esquemas, restrições e DER. |
| [EXECUTION_FLOWS.md](EXECUTION_FLOWS.md) | Sequências de inicialização, conta, pastas, observação e downloads. |
| [GUI_ARCHITECTURE.md](GUI_ARCHITECTURE.md) | Páginas, componentes, handlers e conexão com a engine. |
| [FEATURE_STATUS.md](FEATURE_STATUS.md) | Funcionalidades e estado conforme implementação e testes executados. |
| [TEST_COVERAGE.md](TEST_COVERAGE.md) | Suítes, resultados, ambiente e limitações de validação. |

## Síntese observada

O workspace contém o binário GTK4/Libadwaita `drivesync` e a biblioteca
`drivesync-engine` ([Cargo.toml](../../Cargo.toml),
[engine/Cargo.toml](../../engine/Cargo.toml)). O aplicativo implementa
autorização Google, cadastro de contas, escolha de pastas, observação local,
download inicial e processamento de alterações remotas para a pasta local.
Esses fluxos não equivalem a sincronização bidirecional: não foi identificada
implementação de upload de alterações locais para o Drive.

Há uma divergência importante com [README.md](../../README.md), que afirma que o
aplicativo não sincroniza arquivos. O código atual conecta a tela de conta a
`initial_download_with_progress` e `poll_remote_downloads` em
`src/features/dashboard/components/initial_download.rs`; o segundo fluxo pode
ser iniciado periodicamente. A implementação e os testes locais não comprovam
funcionamento em produção contra Google Drive, Secret Service ou portal real.

## Limites deste registro

- Não foram consultadas issues, planos futuros ou propostas de arquitetura.
- “Integrado” descreve uma chamada identificável no código, não uma garantia de
  disponibilidade operacional em toda instalação.
- Testes com provedores falsos, fixtures ou diretórios temporários são
  identificados como tais; não são evidência de integração externa real.
- Quando a implementação ou validação não permite uma conclusão, o estado é
  indicado como **Não identificado** ou **Não verificado**.

