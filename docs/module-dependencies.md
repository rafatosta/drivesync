# Dependências e colaboração dos módulos

## Regra central
Módulos são independentes em **responsabilidade e implementação**, mas cooperam por contratos e dados do domínio. Uma dependência funcional real não deve ser escondida; uma dependência de implementação deve ser invertida por uma interface.

## Fluxo principal
GUI/CLI → API pública do DriveSyncEngine → Coordinator → Auth/Changes/Planner/Queue/Transfers/Conflicts/State. O Coordinator organiza, não reproduz os algoritmos internos.

## Mapa de relações
| Módulo | Consumido principalmente por | Dados/serviços exigidos |
|---|---|---|
| auth | remote, accounts | AccountId, armazenamento seguro |
| accounts | coordinator, ui | perfil, AuthProvider |
| configuration | coordinator, ui | SyncRoot, SyncPolicy |
| remote | changes, upload, download | credencial via AuthProvider; RemoteFile |
| local | changes, planner, transfers | LocalObservation, SyncRoot |
| state | changes, planner, queue, coordinator | BaseVersion, Cursor, Operation |
| changes | coordinator, planner | observações locais/remotas |
| planner | coordinator | base/local/remoto; retorna SyncPlan |
| conflicts | coordinator | Conflict, versões e política |
| queue | coordinator | SyncOperation, dependências |
| upload/download | coordinator via queue | transfer request, versões esperadas |
| notifications | coordinator, ui | EngineEvent tipado |
| ipc | ui/cli e motor | comandos/eventos públicos versionados |
| ui | usuário | somente fachada pública |
| coordinator | fachada DriveSyncEngine | portas e modelos compartilhados |

## Dependências funcionais versus implementação
- Sincronização **real** com Drive exige autorização válida. Simulação de sync usa RemoteProvider falso, dispensando login real.
- Detecção de conflito exige versões de referência e observações, não necessariamente uma sincronização real prévia.
- Downloads exigem acesso remoto autorizado, mas seu algoritmo pode ser testado com streams simulados.
- A fachada é a única entrada pública para GUI; módulos não expõem estruturas internas à interface.
- Evitar ciclos entre módulos e chamadas que contornem os contratos.

## Falhas propagadas
Credencial revogada → REAUTH_REQUIRED; falha temporária remota → RETRY_WAIT; estado-base desconhecido/corrompido → DEGRADED; versões divergentes → CONFLICT. Nenhuma falha de rede pode se transformar em exclusão.
