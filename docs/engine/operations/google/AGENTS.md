# Orientações para agentes — Operador Google

## Objetivo

Implementar e testar a execução das operações sobre recursos do Google Drive conforme o contrato comum do Operador.

## Contratos de referência

[README](README.md) · [INTERFACES](INTERFACES.md) · [TESTS](TESTS.md) · [Operador](../AGENTS.md)

## Implementação

- Implementar os dez métodos da interface `OperationExecutor`, utilizando os tipos comuns definidos pelo Operador.
- Selecionar e validar a sessão Google associada à requisição e preservar identidades remotas.
- Utilizar exclusivamente a [API do Provider Google](../../providers/google/INTERFACES.md) nas operações remotas.
- Usar as funções privadas do Operador para operações de filesystem aplicáveis.
- Respeitar precondições, integridade, progressos comprovados e segurança dos dados em interrupções.
- Retornar resultados tipados e evidências de execução; representar capacidades indisponíveis e resultados indeterminados com erros apropriados.

## Validação

Implementar os cenários de [TESTS.md](TESTS.md), executar a [suíte comum](../TESTS.md#conformidade-dos-executores-especializados) e utilizar Provider Google simulado e filesystem temporário real. Registrar os testes executados e os resultados. Seguir as instruções gerais do repositório e executar verificações Rust no ambiente Flatpak estabelecido.

**Estado:** orientação documental; a implementação e os testes Rust dependem de trabalho futuro.
