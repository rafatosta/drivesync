# Google — AGENTS

## Missão

Disponibilizar a **API técnica Google já definida** em [`api/INTERFACES.md`](api/INTERFACES.md) por meio do catálogo Providers. **Não duplicar seus métodos**.

## Regras obrigatórias

1. Esta pasta contém somente composição/inicialização da implementação Google e documentação desse limite.
2. Os contratos de Google Drive REST, OAuth/OIDC, transferências e tipos continuam exclusivamente em `api/`.
3. Não redesenhar os 59 + 9 + 10 métodos existentes e não criar wrappers funcionais equivalentes neste nível.
4. As funções técnicas podem receber sessões, tokens ou credenciais conforme a assinatura própria, mas o módulo não se torna proprietário de armazenamento de contas ou credenciais.
5. Não adicionar política, estado nem referências a decisões funcionais de outros módulos.
6. Não criar interfaces universais para outros serviços; cada API externa tem sua definição específica.
7. Falhas de configuração e inicialização devem ser explícitas. Não retornar cliente fictício.
8. Não imprimir segredos em logs, mensagens ou `Debug`.
9. Não criar compatibilidade legada ou refatorar os contratos da API existente sem solicitação explícita.
10. Não afirmar implementação ou testes executados quando somente a documentação foi alterada.

## Verificação

Conferir [INTERFACES](INTERFACES.md), executar futuramente [TESTS](TESTS.md) para obtenção e [api/TESTS](api/TESTS.md) para o protocolo. Reportar resultados verificados e pendências de representação Rust.

[README](README.md) · [API técnica](api/README.md)
