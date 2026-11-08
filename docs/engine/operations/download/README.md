# Download — responsabilidade e organização futura

O módulo `operations/download/` é dono da **execução funcional de operações de recebimento autorizadas** pelo Analisador, sem decidir conflitos. Quando houver implementações específicas por serviço, mantê-las sob `operations/download/<provedor>/`, com uma interface do próprio operador de Download. A implementação `operations/download/google/` poderá **obter por [Providers](../../providers/INTERFACES.md) a [API Google](../../providers/google/api/README.md)** e usar seus métodos específicos, sem assumir responsabilidade por protocolos ou criar cliente HTTP avulso. A interface comum é a do módulo Download, não uma API universal de Providers.

O [provedor fictício `example`](example/README.md) ilustra onde ficaria um especialista futuro.

**Estado:** apenas posição arquitetural; contratos e testes específicos de Download **não foram definidos nesta etapa**. Nenhum código Rust foi modificado.
