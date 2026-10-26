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

A restauração de contas em UI-001 executa em worker, uma vez por processo;
UI-003 só reapresenta a janela. Ver [ACCT-003](../engine/src/accounts/algorithms.md#acct-003--persistir-e-restaurar-perfis-na-abertura).
