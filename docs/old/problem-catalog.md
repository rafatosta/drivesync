# Mapa de problemas menores e suas soluções de referência

A classificação não equivale a afirmar que as integrações estão prontas. **Consolidado** refere-se à técnica de base; **DriveSync** indica uma decisão do produto; **Validar** significa possível bloqueio de viabilidade.

| Módulo | Problema de menor escala | Solução de referência | Questão própria do DriveSync |
|---|---|---|---|
| auth | autorizar e renovar credencial | Google OAuth Desktop/PKCE (R-OAUTH/R-PKCE) | escopos Drive e publicação |
| accounts | identificar e vincular conta | OIDC/identidades estáveis | desvinculação sem exclusão |
| remote | converter chamadas remotas e erros | Drive API/HTTP (R-ERRORS) | semântica de 404 e update condicional |
| local | detectar mudanças locais | GIO FileMonitor + rescan (R-MONITOR) | estabilidade de arquivos editados |
| state | guardar progresso durável | SQLite transactions/WAL (R-SQLITE/R-WAL) | reconciliação com API remota não transacional |
| changes | reconhecer mudanças entre ciclos | Drive changes e inventário local (R-CHANGES) | fechar janela entre scan inicial e cursor |
| planner | decidir ação por arquivo | reconciliação B/L/R + referência Unison | tratamento de UNKNOWN e renames |
| conflicts | preservar versões distintas | cópias de conflito | decisão pendente com janela oculta |
| download | recuperar conteúdo sem arquivo parcial | staging/rename (R-DOWNLOAD/R-RENAME) | durabilidade e mudança de destino |
| upload | recuperar transferência interrompida | sessão resumable (R-UPLOAD) | update condicional e resultado ambíguo |
| queue | retry e recuperação | journal persistente + backoff | agrupar eventos sem perder causalidade |
| coordinator | executar workflow com pré-condições | máquina de estados e recuperação | aplicar invariantes globais |
| notifications | estado assíncrono | eventos, snapshots e notificações | não vazar informação sensível |
| ipc | integrar cliente externo | D-Bus tipado (R-DBUS) | consulta Nautilus sem bloquear UI |
| configuration | validar preferências | validação/versionamento (R-SQLITE/R-FLATPAK) | permissão de background e root overlap |
| ui | ocultar, reabrir, encerrar | Gio Application hold/release (R-HOLD) | comportamento efetivo do Flatpak |

## Como usar
Escolher uma linha → abrir `engine/src/<módulo>/algorithms.md` → localizar seu ID → revisar técnica/fonte, regra própria, teste e lacuna → detalhar com [template](algorithm-spec-template.md) → só então implementar.

## Três desafios transversais ainda não resolvidos
1. Autorização e escopos para acessar arquivos existentes livremente.
2. Atualização remota com controle de concorrência sem perda de modificações.
3. Prova de recuperação segura quando uma operação remota já ocorreu mas SQLite ainda não a confirmou.

Estes desafios **não** devem ser escondidos sob afirmações de que são problemas totalmente resolvidos.
