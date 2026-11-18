# Orchestrator — orientações para agentes

## Escopo autorizado

O usuário solicitou fluxo real e contínuo da aplicação: iniciar os detectores,
aguardar Scanner, entregar itens ao Analyzer e continuar consumindo eventos.
O usuário ampliou o escopo: Analyzer deve chamar Operator para novas réplicas,
com reobservação, retomada e baseline comprovado; Orchestrator coordena o ciclo.
Ler [README](README.md), [INTERFACES](INTERFACES.md), [TESTS](TESTS.md) e
[AGENTS raiz](../../../AGENTS.md).

- Preservar interfaces de Detector, Scanner, Inventory e Analyzer. Usar somente
  métodos públicos; não acessar SQL, storages internos, Providers ou algoritmos privados.
- Iniciar captura com entrega pausada antes do scan. Somente scan completo e análise
  inicial bem-sucedida liberam o consumidor da sessão.
- A fila pertence ao Detector. O sinal de disponibilidade não é entrega nem ack;
  não criar backlog paralelo de File ou um consumidor por página GTK.
- Preservar File.id e payload ao registrar observações associáveis. Não inventar
  ausência confirmada, identidade local/remota por nome ou SyncItem sem dados.
- Usar Scanner somente na reconstrução local inicial; eventos contínuos devem usar
  as observações dos Detectores, sem iniciar varreduras completas para suprir dados
  ausentes. Avisos técnicos não equivalem a arquivos e permanecem no Detector.
- Confirmar o lote somente após ingestão e análise duráveis.
  Falha/Partial/cancelamento preserva o lote por release/expiração, sem Discarded.
- Testar reentrega entre os bancos, cuja gravação não é uma transação distribuída.
- As políticas e chamadas ao Operator pertencem ao Analyzer, nunca ao consumidor
  ou à GUI. Orchestrator compõe dependências, cancela por epoch e entrega itens.
- Manter GTK livre de I/O, preservar chamadores sem Tokio externo, epoch de sessão,
  wakeup sem perda de sinal, worker único, desligamento e substituição seguros.
- Expor estado real de ingestão, erros e contagem de eventos processados, sem
  apresentar ack de observação como sincronização concluída.

## Validação

Usar módulos reais, SQLite e contas/arquivos sintéticos para fluxo pós-abertura,
isolamento, falhas injetadas, reentrega, stop/restart e ausência de efeitos físicos.
Testar também o hook real de restauração da aplicação e atualização da UI sem
refresh manual, em perfil gráfico isolado. Rust somente no Flatpak do manifesto.
