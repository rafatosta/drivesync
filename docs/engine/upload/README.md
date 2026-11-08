# Upload — responsabilidade e organização futura

O módulo `upload/` é dono da **execução funcional de operações de envio autorizadas** pelo Analisador, sem decidir conflitos. Quando houver implementações diferentes por serviço, mantê-las sob `upload/<provedor>/`, com uma interface do próprio operador de Upload. A implementação `upload/google/` poderá consumir a [Google API](../providers/google/api/README.md), sem duplicar HTTP, autenticação ou modelos protocolários.

**Estado:** apenas posição arquitetural; contratos de métodos e testes de Upload **não foram definidos aqui**. Não criar código, diretórios de provedores não implementados ou testes fictícios por causa deste índice.
