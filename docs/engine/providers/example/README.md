# Provedor fictício `example`

**Exemplo exclusivamente documental**, sem serviço real, autenticação ou implementação. Ilustra onde ficam as integrações específicas de um futuro provedor.

- [`api/`](api/README.md): comunicação com o serviço, modelos e protocolos próprios.
- [`detectors/remote/example/`](../../detectors/remote/example/README.md): lógica de detecção remota do provedor.
- [`upload/example/`](../../upload/example/README.md): estratégia de envio para o provedor.
- [`download/example/`](../../download/example/README.md): estratégia de recebimento do provedor.

Cada módulo funcional mantém sua própria responsabilidade e consome a API do serviço por contrato. Nenhum destes diretórios contém código ou testes executáveis.
