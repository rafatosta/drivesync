# Ciclo de vida do aplicativo DriveSync

## Decisão
Um processo Flatpak com interface GTK4/Libadwaita e motor Rust. Fechar a janela não encerra a aplicação. Inicialização automática é opcional, com janela oculta. Sem daemon e sem system tray na v1.

## Estados
`STARTING → BACKGROUND | VISIBLE`, `VISIBLE ↔ BACKGROUND`, `VISIBLE|BACKGROUND → QUITTING → STOPPED`.

## Eventos
- Ativação pelo lançador sem instância: iniciar motor uma vez e exibir janela.
- Ativação pelo lançador com instância oculta: reexibir a mesma janela.
- Fechar janela: esconder, preservar processo e sincronização.
- Inicialização de sessão autorizada: iniciar sem apresentar janela.
- Pausar: suspender sincronização, preservar processo.
- Sair: encerrar trabalho com segurança e terminar processo; não confundir com fechar janela.

## Regras de implementação
- Gerir retenção explícita da aplicação (GApplication hold/release ou mecanismo equivalente) para manter vida sem janelas visíveis.
- Identificador estável de aplicação e proteção contra motores duplicados na mesma raiz.
- Na saída controlada, cancelar tarefas de forma recuperável, persistir journal antes de liberar recursos.
- Sem loop de eventos bloqueado por operações de rede, hashing ou disco.
- Inicialização automática via integração apropriada Flatpak/portal quando suportada, respeitando consentimento e preferências do ambiente.
- Se integração de background não for permitida, comunicar limitação ao usuário; não prometer execução garantida pelo simples ato de ocultar.

## Testes
Abrir → ocultar → sincronizar; abrir novamente → única janela/motor; iniciar oculto; pausar; sair; crash + recuperação; permissão de execução em background negada; Nautilus conectado/desconectado sem afetar o processo.

## Restauração de contas na abertura — UI-001 / ACCT-003

Após construir o motor e a janela, Welcome inicia ACCT-003 em worker. Connect fica
indisponível durante a verificação para evitar disputas com a restauração. O índice
local só descobre identidades: apenas AUTH-002 confirma autenticação. A interface
continua responsiva e oferece retry para falhas, inclusive offline/cofre bloqueado.
Uma única conta confirmada abre Account Information; várias são exibidas na lista.
Ativações posteriores só apresentam a mesma janela (UI-003). Sair cancela o fluxo
interativo existente; requisições de restauração em curso têm os timeouts de auth.
Nenhuma sincronização, autostart ou implementação de journal é acrescentada aqui.
