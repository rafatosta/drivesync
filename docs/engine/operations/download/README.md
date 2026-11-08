# Download — responsabilidade e organização futura

O módulo `operations/download/` é dono da **execução funcional de operações de recebimento autorizadas** pelo Analisador, sem decidir conflitos. Quando houver implementações específicas por serviço, mantê-las sob `operations/download/<provedor>/`, com uma interface do próprio operador de Download. A implementação `operations/download/google/` poderá consumir a [Google API](../../providers/google/api/README.md), sem assumir responsabilidade por seus protocolos.

O [provedor fictício `example`](example/README.md) ilustra onde ficaria um especialista futuro.

**Estado:** apenas posição arquitetural; contratos e testes específicos de Download **não foram definidos nesta etapa**. Nenhum código Rust foi modificado.
