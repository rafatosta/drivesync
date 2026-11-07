# Estado das funcionalidades

Os estados abaixo qualificam evidência no repositório, não disponibilidade em
produção. “Implementada e testada” significa que existe teste relevante no
workspace e que a suíte executada passou; quando o teste usa doubles ou
fixtures isso está explicitado. Não equivale a teste contra serviços/portais
reais. Comandos e resultado da execução estão em
[TEST_COVERAGE.md](TEST_COVERAGE.md).

| Funcionalidade | Módulo responsável | Evidência no código | Estado |
| --- | --- | --- | --- |
| Restauração e validação de contas salvas | `engine/src/accounts/`, `engine/src/facade.rs` | `accounts/tests.rs`; `facade/tests.rs`; `restore_accounts` | Implementada e testada |
| Autorização OAuth com PKCE, callback loopback e identificação | `engine/src/auth/` | `auth/tests.rs` usa `FakeGoogle` e `MemoryVault`; `provider.rs::authorize` | Implementada e testada — simulação; Google real não verificado |
| Persistência de credenciais em Secret Service | `engine/src/auth/storage.rs` | Adaptador `DesktopVault`; testes de integração estão ignorados e requerem Secret Service privado | Implementada, não validada — integração real não verificada |
| Configuração e validação de pasta local/remota | `engine/src/configuration/`, `engine/src/accounts/` | `configuration/tests.rs`, `accounts/tests.rs`; portal real é requisito de testes ignorados | Implementada e testada — validação local; integração portal não verificada |
| Consulta e paginação de pastas Drive | `engine/src/remote/folders.rs`, UI `folder_picker/` | `remote/tests.rs`; handler `folder_picker/controller.rs::load` | Implementada, não validada — chamadas reais ao Drive não executadas |
| Observação de alterações locais (inotify) | `engine/src/local_detector/watcher.rs` | `watcher/tests.rs` usa diretórios temporários e eventos locais; integração UI em `src/core/local_observation.rs` | Implementada e testada — condições locais simuladas/temporárias |
| Persistência de baseline e fila de investigação | `engine/src/local_state/` | `local_state/store/tests.rs` exercita transações, limites e recuperação | Implementada e testada |
| Histórico e timeline de atividades | `engine/src/activity_history.rs`, `src/features/dashboard/components/activities.rs` | `activity_history.rs` tem testes; componente UI consulta `activity_history`; testes de UI dependem de display | Implementada e testada — persistência; UI real não verificada |
| Download inicial de arquivos | `engine/src/download.rs`, `engine/src/facade.rs`; UI `initial_download.rs` | `download.rs` e testes de resume/reconciliação; validação por fixtures/ambiente de teste, não por conta real | Implementada e testada — teste simulado/local; Google Drive real não verificado |
| Detecção de mudanças remotas e aplicação no diretório local | `engine/src/remote_detector/`, `engine/src/download.rs`, `engine/src/facade.rs::poll_remote_downloads` | `remote_detector/tests.rs`, `facade/tests.rs` e testes do download; providers/servidores de teste | Implementada e testada — cenário automatizado não comprova integração real |
| Detecção e apresentação de conflito | `engine/src/download.rs`, `src/features/dashboard/components/transfers.rs` | engine retorna conflitos pendentes; UI persiste/mostra conflito e oferece abrir pasta remota | Parcial — revisão visual existe; resolução/renomeação pela UI não está implementada |
| Upload de alterações locais para Drive | Não identificado | O observador produz hints para persistência/rechecagem; não foi encontrado consumidor que envie conteúdo local pela API do Drive | Não identificada |
| Sincronização bidirecional | Não identificado | Há download inicial e aplicação de mudanças remotas; fluxo local→Drive não identificado | Parcial — direção remota→local observada; upload local não identificado |
| Preferências de tema, execução em background e intervalo remoto | `src/features/preferences/` | `model.rs` lê/grava `preferences.ini`; `window.rs` conecta handlers. A integração depende do desktop para background | Implementada, não validada — UI/portal real não verificados |
| Execução em background via portal desktop | `src/core/background/` | `background/tests.rs` está ignorado por depender de sandbox/portal; código conecta ações e startup | Implementada, não validada |
| Abrir pasta local/remota e sair da conta | `src/features/dashboard/components/folders.rs`, `account_actions.rs`, `src/features/accounts/session.rs` | Handlers estão conectados a GIO e `DriveSyncEngine::sign_out`; testes GTK relevantes estão ignorados | Implementada, não validada como interação de desktop |
| Diagnóstico local e remoto por CLI | `src/bin/drivesync-local-diagnostic.rs`, `drivesync-remote-diagnostic.rs` | Bins chamam APIs da engine para diagnóstico; testes unitários de watchers/detector cobrem partes, não toda execução empacotada | Implementada, não validada como operação empacotada |
| Sincronização em produção contra Google Drive | Não verificado | Não foi executado teste de integração com credencial/conta real durante a análise | Não verificada |

## Evidência e interpretação

- “Testada” é baseada nos testes do workspace concluídos com sucesso no
  ambiente Flatpak documentado em `TEST_COVERAGE.md`.
- Os testes de OAuth usam provedores e vaults falsos; os testes de download e
  detector exercitam fixtures, providers de teste e armazenamento local.
- Existem testes automatizados de portal, keyring e GTK marcados como
  ignorados, portanto não foram contabilizados como validação real.
- A existência de binário ou handler não demonstra que o fluxo foi exercitado
  num aplicativo empacotado nem que serviços externos estejam disponíveis.

