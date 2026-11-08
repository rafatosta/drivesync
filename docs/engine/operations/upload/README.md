# Upload — responsabilidade e organização futura

O módulo `operations/upload/` é dono da **execução funcional de operações de envio autorizadas** pelo Analisador, sem decidir conflitos. Quando houver implementações diferentes por serviço, mantê-las sob `operations/upload/<provedor>/`, com uma interface do próprio operador de Upload. A implementação `operations/upload/google/` poderá **obter por [Providers](../../providers/INTERFACES.md) a [API Google](../../providers/google/api/README.md)** e usar seus métodos específicos, sem duplicar cliente HTTP ou fluxo de credenciais. A interface comum é a do módulo Upload, não uma interface universal de transferência de Providers.

O [provedor fictício `example`](example/README.md) ilustra onde ficaria um especialista futuro.

**Estado:** apenas posição arquitetural; contratos de métodos e testes de Upload **não foram definidos aqui**. Nenhum código Rust foi modificado.
