# Módulo download

## Objetivo
Instalar conteúdo binário verificado e criar atalhos locais para documentos
Google Workspace, preservando os arquivos locais existentes.

## Papel no sistema
Atua como caixa-preta especializada do DriveSync, sujeita aos invariantes globais em `docs/invariants.md`.

## Quem utiliza
queue, coordinator.

## Saídas e compromissos
Arquivos binários instalados por estágio temporário ou arquivos de dados
`.dslink` com o `webViewLink` validado do Drive. Esses arquivos abrem Docs,
Sheets e Slides no navegador padrão por meio do handler MIME do DriveSync,
inclusive atalhos do Drive para esses tipos; não exportam cópias nem enviam os
arquivos `.dslink` de volta ao Drive. O formato atual é `DRIVESYNC-LINK/1`,
seguido pela URL em uma linha própria.
Um atalho divergente já existente gera conflito e é preservado.

## Limites: o que NÃO faz
Não planejar qual arquivo sincronizar.

## Algoritmos
Downloads binários usam transferência temporária e validação final. Arquivos
`.dslink` contêm apenas um marcador de formato e uma URL validada; não são
executáveis. O tipo MIME associa esses dados ao launcher instalado do DriveSync,
que valida a URL novamente e a abre pelo handler HTTPS padrão do sistema. A
instalação é exclusiva e nunca substitui um arquivo local divergente.

## Erros e cenários a verificar
Rede interrompida; disco cheio; remoto alterado; hash divergente.

## Contrato para implementação
O executor valida o link antes de criar o arquivo de dados, instala-o sem
substituir caminhos existentes e reporta conflitos por item. A camada de
aplicativo valida novamente o formato e restringe a abertura ao host e aos
caminhos do Google Workspace esperados.

## Testes mínimos
Caminho normal, entrada inválida, falha temporária, interrupção após efeito parcial, repetição e recuperação. Casos completos pendentes de revisão humana.

**Estado:** implementação no engine e no aplicativo; validação do fluxo real
via Nautilus e Flatpak ainda requer uma sessão gráfica instalada.
