# Provedor fictício `example`

**Exemplo exclusivamente documental**, sem serviço real, autenticação ou implementação. Ilustra onde ficam as integrações específicas de um futuro provedor.

- [`api/`](api/README.md): comunicação com o serviço, modelos e protocolos próprios.
- [`detectors/remote/example/`](../../detectors/remote/example/README.md): lógica de detecção remota do provedor.
- [`operations/upload/example/`](../../operations/upload/example/README.md): estratégia de envio para o provedor.
- [`operations/download/example/`](../../operations/download/example/README.md): estratégia de recebimento do provedor.

Cada módulo funcional mantém sua própria responsabilidade e consome a API do serviço por contrato. Nenhum destes diretórios contém código ou testes executáveis.
