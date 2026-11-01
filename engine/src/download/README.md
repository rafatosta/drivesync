# Módulo download

## Objetivo
Instalar conteúdo binário verificado e criar atalhos locais para documentos
Google Workspace, preservando os arquivos locais existentes.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
queue, coordinator.

## Saídas e compromissos
Arquivos binários instalados por estágio temporário ou atalhos `.html` com o
`webViewLink` validado do Drive. Os atalhos abrem Docs, Sheets e Slides no
navegador, inclusive atalhos do Drive para esses tipos; não exportam cópias nem
enviam os arquivos `.html` de volta ao Drive.
Um atalho divergente já existente gera conflito e é preservado.

## Limites: o que NÃO faz
Não planejar qual arquivo sincronizar.

## Algoritmos
Downloads binários usam transferência temporária e validação final. Atalhos são
escritos como HTML com redirecionamento e link de fallback, escaping e modo
`0444`, com journal e instalação exclusiva. No GNOME, a abertura depende da
associação de HTML ao navegador. O contrato vigente está em [workspace.md](workspace.md).

## Erros e cenários a verificar
Rede interrompida; disco cheio; remoto alterado; hash divergente.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** instalação local de binários e atualização gerenciada implementadas
no recorte de [execution.md](execution.md) e [updates.md](updates.md).
O contrato geral acima inclui intenções além desse recorte. Evidências e limites
na [matriz do eixo B/L/R](../state/preparation.md#rastreabilidade-das-regras-do-eixo-1);
a bateria A01–A12 com Google Drive real permanece pendente.
