# Orchestrator — orientações para agentes

## Escopo

Coordenar por interfaces públicas, sem reimplementar Scanner, Detector, Inventory,
Analyzer ou seus algoritmos. Ler [README](README.md), [INTERFACES](INTERFACES.md),
[TESTS](TESTS.md) e o [AGENTS raiz](../../../AGENTS.md).

A integração autorizada é `Scanner → Inventory → Analyzer` durante `prepare`,
com Detector capturando e entrega pausada. O Analyzer **apenas produz/persiste
decisões**; execução real ficará para implementação posterior do Operator.

- Preservar interfaces existentes de Detector, Scanner e Inventory, conforme
  instrução expressa do usuário. Usar os métodos públicos atuais.
- Iniciar as duas fontes com entrega Paused antes do Scanner. Scan Partial não
  libera análises, não fabrica cobertura e não descarta eventos.
- Após scan completo, consultar itens atuais do Inventory e chamar a fachada
  Analyzer por item. Nunca chamar Reconciliação, Conflitos, Operator, Providers,
  Storage, SQL ou primitivas de filesystem diretamente para tomar/executar decisões.
- Não confirmar baseline nem duplicar a persistência já feita pelo Analyzer.
- Não inferir igualdade de conteúdo por tamanho/data/versão, ausência remota por
  falta de snapshot ou associação local/remota por nome. Manter evidência desconhecida.
- Visitar todos os status: observações podem invalidar avaliações sem mudar status.
- Manter trabalho bloqueante fora de GTK e permitir `prepare` sem runtime Tokio
  no chamador. Worker da análise é finito, não scheduler global ou fila privada.
- Preservar epoch e controle de parada/cancelamento. Uma chamada despachada pode
  terminar a transação; não publicar sucesso final após parada.
- Status reflete fases reais e snapshots locais; `Analyzing` não é transferência.
- Não ativar consumidor contínuo/ack parcial da fila nesta fase. Uma futura
  ingestão deve persistir todas as observações e necessidades de reanálise antes
  do reconhecimento; idempotência e gaps precisam ser testados com APIs públicas.
- Ações/opções de resolução, comandos completos e execução ficam fora do escopo.

## Validação

Usar módulos reais, SQLite temporário, contas/arquivos sintéticos e falhas
injetadas para testar durabilidade, retry, isolamento e ausência de efeitos reais.
Executar Rust somente no Flatpak do projeto. Registrar testes realmente executados
e limitações de GUI, contas reais, aplicativo instalado e ingestão futura.
