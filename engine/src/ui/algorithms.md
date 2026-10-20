# Algoritmos UI / ciclo de vida

## UI-001 inicialização
1. Detectar ativação normal versus autostart autorizado.
2. Construir/obter instância do motor uma única vez.
3. Em ativação normal, apresentar a janela única; no autostart, não apresentá-la.
4. Manter processo vivo com retenção apropriada enquanto motor ativo.

## UI-002 ocultação
1. Interceptar solicitação de fechar.
2. Ocultar janela; não emitir shutdown para motor.
3. Continuar processando eventos e atualizações sem bloquear GTK.

## UI-003 reabertura
1. Ao ativar o aplicativo já executando, localizar janela existente.
2. Apresentar janela, não criar segundo motor.

## UI-004 saída verdadeira
1. Receber comando explícito Sair.
2. Solicitar ao motor parada recuperável e confirmar journal.
3. Liberar retenção de aplicação e terminar.

## Testes
Fechar/reabrir; executar sem janela; comando Sair; autostart sem janela; duas ativações em sequência; falha do background portal; desempenho da UI sob sincronização.
