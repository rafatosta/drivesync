# Download — responsabilidade e organização futura

O módulo `download/` é dono da **execução funcional de operações de recebimento autorizadas** pelo Analisador, sem decidir conflitos. Quando houver implementações específicas por serviço, mantê-las sob `download/<provedor>/`, com uma interface do próprio operador de Download. A implementação `download/google/` poderá consumir a [Google API](../providers/google/api/README.md) sem assumir responsabilidade por seus protocolos.

**Estado:** apenas posição arquitetural; contratos e testes específicos de Download **não foram definidos nesta etapa**. Nenhum código ou implementação de provedor foi criado.
