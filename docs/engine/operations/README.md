# Operações — execução especializada por provedor

Este módulo reúne as responsabilidades de **execução de operações autorizadas de sincronização**, sem decidir conflitos e sem implementar protocolos HTTP específicos. Cada operação possui seu próprio contrato e especialistas independentes por provedor, seguindo a mesma organização adotada em `detectors/remote/`.

- [Upload](upload/README.md): execução de envios autorizados.
- [Download](download/README.md): execução de recebimentos autorizados.

O Analisador decide o que executar; o operador escolhe um especialista adequado à conta e ao provedor. **Esse especialista solicita a API específica por [Providers](../providers/INTERFACES.md)** e implementa o contrato funcional do módulo de operação. Não existe obrigação de padronizar os métodos das APIs dos diferentes serviços. Os contratos de métodos ainda não foram definidos nesta etapa; não presumir interfaces ou implementações existentes no código.
