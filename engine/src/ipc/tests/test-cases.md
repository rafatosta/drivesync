# Casos de teste IPC — extensão Nautilus

| ID | Cenário | Esperado |
|---|---|---|
| IPC-T01 | Cliente consulta URI gerenciada atualizada | Estado e geração coerentes |
| IPC-T02 | URI fora das raízes | NOT_MANAGED |
| IPC-T03 | Estado obsoleto/degradado | UNKNOWN, nunca SYNCED |
| IPC-T04 | DriveSync encerrou | Cliente trata UNAVAILABLE sem travar Nautilus |
| IPC-T05 | DriveSync oculto | Consultas continuam funcionando |
| IPC-T06 | Sinal perdido | Cliente recompõe snapshot |
| IPC-T07 | API D-Bus indisponível no sandbox | Motor e janela continuam funcionando |
| IPC-T08 | Cliente consulta lote grande | Limite aplicado, erro tipado |
| IPC-T09 | Cliente lento ou malicioso | Motor não bloqueia nem revela dados sensíveis |

A implementação do conector Nautilus é opcional e requer teste real com Flatpak + cliente nativo.
