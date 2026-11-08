# Provider Google — organização por responsabilidade

## Propriedade

O agrupamento `providers/google/` reúne a documentação do **cliente e protocolos Google**, não todas as rotinas do DriveSync relacionadas ao Google. A interface HTTP/REST e a autenticação protocolar permanecem no módulo [Google API](api/README.md).

| Capacidade | Onde pertence | Responsabilidade |
| --- | --- | --- |
| API Google Drive, OAuth/OIDC, tipos e respostas Google | [`providers/google/api/`](api/README.md) | Comunicação de baixo nível e conversões de protocolo |
| Consulta de alterações por Google Changes | [`detectors/remote/google/`](../../detectors/remote/google/README.md) | Estratégia de detecção e checkpoint seguro |
| Gerenciamento de contas | `accounts/` | Associação de conta, serviço e sessão |
| Upload e download na engine | `upload/google/`, `download/google/` quando documentados | Estratégia funcional e execução sob decisão da engine |

## Fronteira

O especialista de detecção **usa** Google API, mas Google API não conhece Detector. Upload/Download também podem usar Google API, mas não devem importar o Detector Remoto. Os componentes de detecção/transferência são implementáveis e testáveis separadamente por seus contratos; cliente HTTP fake é adequado para testes de integração da Google API.

O módulo Google mantém os modelos específicos do serviço. O modelo de domínio `RemoteFile`, quando definido, pertence a modelos compartilhados da engine, sem exigir que a documentação atual da Google API seja reescrita agora.

**Estado:** organização documental, sem mudança de código ou implementação de novos provedores.
