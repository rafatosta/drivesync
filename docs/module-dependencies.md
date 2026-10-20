# Dependências entre módulos

Módulos são independentes por responsabilidade, não por dados ou pré-condições funcionais. O aplicativo GNOME e o motor Rust rodam **no mesmo processo**. A GUI chama a fachada pública diretamente.

## Fluxo principal

GUI → `DriveSyncEngine` → Coordinator → Auth/Changes/Planner/Queue/Transfers/Conflicts/State.

## Integração opcional

Nautilus (extensão nativa) ↔ D-Bus de sessão ↔ `ipc` ↔ snapshots de `state` e eventos de `notifications`.

Este D-Bus **não** é exigido entre GUI e motor. Falha na extensão/IPC não interrompe a sincronização.

## Consumo dos módulos

- `auth` fornece autorização ao adaptador `remote`; não gerencia arquivos.
- `accounts` e `configuration` mantêm dados da conta e raízes.
- `changes` observa estado local e remoto usando `local`, `remote` e `state`.
- `planner` recebe observações e versão-base, devolve decisões puras.
- `conflicts` preserva/responde divergências sem sobrescrita silenciosa.
- `queue` controla operações duráveis de `upload` e `download`.
- `coordinator` verifica pré-condições e integra os resultados.
- `notifications` produz eventos, consumidos por GUI e adaptador `ipc`.
- `ipc` publica somente informação não sensível para clientes externos.
- `ui` apenas apresenta/solicita operações através da fachada.

## Falhas funcionais

Token revogado → `REAUTH_REQUIRED`; falha remota transitória → `RETRY_WAIT`; banco inconsistente → `DEGRADED`; divergência de versões → `CONFLICT`; cliente D-Bus ausente → extensão `UNAVAILABLE`, mas motor continua funcionando.

Para testes, provedores simulados podem satisfazer contratos sem autenticação real. Dependência funcional de autorização permanece nos testes de integração reais.
