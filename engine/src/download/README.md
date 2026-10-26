# Módulo download

## Objetivo
Instalar conteúdo binário verificado e criar atalhos locais para documentos
Google Workspace, preservando os arquivos locais existentes.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
queue, coordinator.

## Saídas e compromissos
Arquivos binários instalados por estágio temporário ou atalhos `.desktop` com o
`webViewLink` validado do Drive. Os atalhos abrem Docs, Sheets e Slides no
navegador, inclusive atalhos do Drive para esses tipos; não exportam cópias nem
enviam os arquivos `.desktop` de volta ao Drive.
Um atalho divergente já existente gera conflito e é preservado.

## Limites: o que NÃO faz
Não planejar qual arquivo sincronizar.

## Algoritmos
Downloads binários usam transferência temporária e validação final. Atalhos são
escritos como `Type=Link`, sem `Exec`, com instalação exclusiva sem substituição.
No GNOME, o gerenciador de arquivos pode pedir que o usuário confie no atalho na
primeira abertura.

## Erros e cenários a verificar
Rede interrompida; disco cheio; remoto alterado; hash divergente.

## Contrato para implementação
Deverá documentar entradas, saídas, erros tipados, efeitos colaterais, idempotência e pré/pós-condições antes de gerar código. Não vincular a especificação a Rust ou Python.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** rascunho v0.1; não implementado.
