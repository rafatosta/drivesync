# Estratégia de testes: evidência antes da implementação

## Princípio
O resultado esperado é definido a partir do contrato e da bibliografia/política aprovada, **antes de ler a implementação**. Testes que repetem a lógica do código não são oráculos independentes.

## Níveis
1. **Unitário puro:** decisões do Planner, nomenclatura de cópias, classificação de erros, coalescência de eventos, sem rede real.
2. **Contrato:** mocks/fakes das portas (Auth, Remote, Local, State), garantindo semântica idêntica de status e erros.
3. **Integração isolada:** SQLite real e árvore temporária, servidor HTTP simulado, interrupções injetadas.
4. **Ponta a ponta:** conta Google de teste e pasta exclusiva, sem arquivos pessoais.
5. **Operacional:** Flatpak GNOME com janela aberta/oculta, autostart autorizado e API D-Bus opcional.

## Matriz obrigatória
Para cada algoritmo ID:
- Caso normal e caso limite.
- Estado `UNKNOWN` vs `ABSENT` e alteração concorrente quando aplicável.
- Cancelamento e repetição.
- Falha antes de efeito, após efeito e antes do registro persistente.
- Persistência, recuperação e ausência de operação destrutiva indevida.
- Resultado observável e fixture/expectativa explicada em `tests/test-cases.md` do módulo.

## Cenários prioritários
- Idempotência: novo ciclo sem alterações = nenhuma transferência.
- Conteúdo: arquivo temporário jamais aparece como download concluído.
- Upload: resposta perdida exige reconciliação.
- Conflitos: modificações simultâneas mantêm ambas as versões.
- Monitor: eventos perdidos/duplicados não causam exclusão.
- Estado: DB corrompido não é apagado automaticamente.
- OAuth: credencial inválida não apaga dados.
- API: 404 não basta para comprovar exclusão; 403 quota difere de 403 permissão.
- CLI/GUI: fechar janela não encerra motor.
- Flatpak: queda de permissões de background/IPC não torna estado falsamente sincronizado.

## Critérios de aceite
Registrar comandos de teste, ambiente, resultados, evidência e limitações. Só assinalar testes como executados após execução real. Não afirmar confiabilidade em produção até a matriz de viabilidade e falhas críticas estar concluída.

Ligações: [método](development-methodology.md), [template de especificação](algorithm-spec-template.md), [estudo de viabilidade](feasibility-validation.md).
