# Providers — AGENTS

## Missão

Manter um catálogo de serviços externos e **fornecer suas interfaces específicas** conforme [README](README.md) e [INTERFACES](INTERFACES.md). Não incluir responsabilidades funcionais da aplicação.

## Regras obrigatórias

1. O módulo oferece descoberta e seleção de APIs; **não** decide como, quando ou por que as funções serão usadas.
2. Cada provedor mapeia seu próprio serviço externo em métodos, tipos e erros. **Não** exigir protocolo universal de arquivos, login ou transferência.
3. A API de cada serviço é a fonte exclusiva das suas funções; não duplicar contratos técnicos no catálogo.
4. A fábrica somente registra implementações disponíveis; documentação ou placeholder não significa disponibilidade.
5. O cliente de protocolo externo é criado/fornecido pela implementação correspondente, sem múltiplas implementações concorrentes do mesmo transporte.
6. É permitido expor funções técnicas de autenticação, tokens e sessões conforme exigidas pelo serviço; **não** criar armazenamento de contas, keyring ou gerenciamento de credenciais como responsabilidade de Providers.
7. Parâmetros e dados técnicos de autenticação podem transitar pelas funções da API quando necessários, com cuidado para não registrar segredos em logs ou erros.
8. Não adicionar referências de propriedade, dependências ou regras de funcionamento de outros módulos na documentação de Providers.
9. Não alegar implementação Rust, testes aprovados ou suporte a serviços que não foram verificados.
10. Não acrescentar adaptadores de compatibilidade legada nem alterar contratos técnicos existentes sem justificativa específica.

## Método de trabalho

- Conferir o contrato da API específica antes de adicionar funções.
- Implementar e testar catálogo/seleção independentemente dos métodos técnicos.
- Manter testes de protocolo dentro da pasta de cada API.
- Verificar tipo correto, isolamento da implementação, falhas de inicialização e preservação de erros.
- Documentar apenas comportamento observado, pendências reais e resultados executados.

[TESTS](TESTS.md) · [Google Provider](google/README.md) · [Google API](google/api/INTERFACES.md)
