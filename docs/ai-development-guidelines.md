# Diretrizes para implementação assistida por IA

## Objetivo
Delegar tarefas pequenas e auditáveis, sem delegar ao modelo a definição implícita do comportamento. IA e pessoa implementam **o mesmo contrato aprovado**.

## Tamanho da tarefa
Uma solicitação = um algoritmo (ou refinamento atômico) + seus testes. Limitar mudanças ao diretório `engine/src/<modulo>/`; exceções para contratos compartilhados exigem justificativa e revisão. Não implementar um sistema completo num único prompt.

## Prompt modelo
```text
Projeto: DriveSync.
Implemente apenas <ALGORITHM_ID> no módulo <MODULE>.
Leia: docs/invariants.md, docs/development-methodology.md,
docs/algorithm-spec-template.md, engine/src/<MODULE>/README.md,
contract.md, algorithms.md e as referências associadas.

Requisitos:
1. Não alterar contratos públicos, estrutura do workspace ou outros módulos sem justificar e pedir revisão.
2. Não criar políticas implícitas de exclusão, substituição ou resolução de conflitos.
3. Usar interfaces de dependências; criar fake somente para os testes.
4. Implementar casos de teste da especificação e falhas indicadas.
5. Não registrar tokens ou conteúdo privado em logs.
6. Explicar opções técnicas não triviais e alternativas descartadas.
7. Relatar exatamente quais testes foram executados; não alegar execução sem evidência.
8. Entregar diff mínimo; caso falte uma regra, marcar BLOCKED e explicar.
```

## Checklist de revisão humana
- Consigo explicar, linha por linha, o caminho normal e o de falha?
- O código faz apenas o que o contrato promete?
- Existe leitura/gravação fora da raiz autorizada?
- O resultado da chamada remota pode ser ambíguo?
- Há operação destrutiva sem revalidação da versão?
- Uma alteração local durante I/O é reconhecida?
- Uma reinicialização preserva a capacidade de recuperação?
- O teste comprova o comportamento em vez de espelhar a implementação?
- As dependências Rust e o escopo OAuth foram justificados?
- O diff adiciona lógica não prevista? Se sim, voltar à especificação.

## Ferramentas
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`; testes de integração/rede controlada em etapas separadas. Execução do Cargo exige dependências nativas GTK/Libadwaita presentes para todo o workspace; os comandos variam conforme o objetivo.

## Critério para encerrar uma etapa
Especificação e testes revisados, código pequeno inspecionado, evidências do teste registradas, efeitos inesperados ausentes. Sem requisito satisfeito, etapa continua aberta.
