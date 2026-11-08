# Providers — AGENTS

## Missão

Implementar Providers **somente como catálogo e fábrica tipada das APIs específicas** por `ProviderId`/`AccountId`. Seguir [README.md](README.md), [INTERFACES.md](INTERFACES.md) e [TESTS.md](TESTS.md). O desenho é uma proposta documental, não implementação Rust confirmada.

## Regras obrigatórias

1. Providers mantém `list/get/open` como superfície conceitual mínima; não cria traits obrigatórias universais de autenticação, detecção, arquivos ou transferência.
2. Cada serviço fornece **sua própria API completa** por um handle tipado. [Google API](google/api/INTERFACES.md) preserva métodos, modelos e erros próprios de Google.
3. Apenas APIs obtidas pela fábrica Providers podem realizar comunicação externa com serviços remotos. Especialistas podem importar tipos Google, mas **não criar clientes HTTP/OAuth paralelos**.
4. O **contrato comum está no módulo consumidor**, não em Providers: especialista de Detector implementa `RemoteDetector`, especialista Upload implementa contrato Upload quando definido, e assim por diante.
5. O especialista transforma saída específica (`GoogleChange`, `GoogleFile`) para tipos do contrato de seu módulo (`File`, `RemoteFile`, estados) **sem alterar a interface pai**.
6. `accounts/auth/` possui gestão de sessão, consentimento e keyring; a API Google oferece protocolo OAuth/OIDC e sua comunicação externa. Nenhum token bruto no Inventário ou GUI.
7. `get(ProviderId)` não é login; `open(AccountId)` requer conta resolvida/autorizada e não pode cruzar identidades. Providers não lê banco privado de Accounts.
8. `example/` é placeholder não implementado: não anunciar como disponível, não criar stubs funcionais.
9. Não criar versões fictícias de interfaces OneDrive nem projetar um conjunto de endpoints que todas APIs precisem implementar.
10. Providers não controla `FileBatch`, checkpoint, `SyncItem`, fila de operações, reconciliação ou política de exclusão; consumidores especializados fazem seu trabalho conforme contrato.

## TDD e verificação

- Testar a fábrica com `ProviderId` válido/inválido, abertura de sessão por conta, erro de incompatibilidade e isolamento entre contas.
- Testar a integração com especialista fake usando API fornecida; garantir que output obedeça ao contrato do módulo pai.
- Colocar testes de protocolo no submódulo Google API; não duplicar seus 59 métodos em Providers.
- Inspecionar dependências para impedir construções diretas de HTTP externo fora de Providers.
- Registrar quais chamadas foram compiladas/executadas, quais são conceituais e quais estão bloqueadas por contrato ainda não aprovado.

## Pendências reais

Retorno heterogêneo em Rust (`enum` vs fábrica tipada), autorização da conta sem dependência circular, representação de sessão/credenciais, async/ownership e contratos completos de autenticação/operadores. Não preencher lacunas artificialmente, não migrar formatos legados e não alterar código Rust sem solicitação.
