# Instruções para agentes — Google API (DriveSync)

## Escopo

Implementar e testar **exclusivamente** o módulo independente `google_api/` da engine Rust, que abstrai a integração com **Google Drive REST API v3, Google OAuth 2.0 e OpenID Connect**. Os consumidores devem enxergar apenas tipos, operações e erros do contrato; não devem construir URLs, interpretar JSON do Google, conhecer endpoints nem repetir constantes específicas do provedor.

**Referências obrigatórias, nesta ordem:**

| Referência | Uso |
| --- | --- |
| [INTERFACES.md](INTERFACES.md) | Assinaturas Rust, tipos públicos, entradas, saídas e erros previstos |
| [README.md](README.md) | Endpoints, operações, recursos e comportamento oficial |
| [Drive API v3](https://developers.google.com/workspace/drive/api/reference/rest/v3) e [Discovery](https://www.googleapis.com/discovery/v1/apis/drive/v3/rest) | Contrato normativo dos métodos e campos do Google |
| [OAuth desktop](https://developers.google.com/identity/protocols/oauth2/native-app) e [OpenID Connect](https://developers.google.com/identity/openid-connect/reference) | Autorização, tokens, identidade e validação |
| [AGENTS.md da raiz](../../../AGENTS.md) | Regras gerais de commits, idioma, segurança e ambiente Flatpak |

**Não usar a implementação antiga da engine como especificação.** A documentação descreve o módulo que será construído; o código existente não restringe sua cobertura.

## Método de trabalho: Contract-First + TDD

| Etapa | Ação obrigatória | Critério de conclusão |
| --- | --- | --- |
| 1. Contrato | Selecionar uma operação pública de `INTERFACES.md`; identificar tipos, parâmetros obrigatórios/opcionais, retorno e erro | A operação é especificável sem olhar outros módulos |
| 2. Checagem oficial | Confirmar no método oficial: verbo, endpoint, query, body, campos retornados, paginação, limites e erros | Divergências e ambiguidades identificadas e corrigidas **na documentação antes do código** |
| 3. Testes primeiro | Escrever testes de contrato e de tradução HTTP usando respostas sintéticas do Google | Os testes inicialmente falham por ausência de comportamento, não por um teste incorreto |
| 4. Implementação mínima | Implementar só o necessário para satisfazer o contrato e os testes | Testes do grupo passam dentro do Flatpak |
| 5. Regressão | Validar erros, casos negativos, segurança e operações relacionadas | Nenhuma funcionalidade anterior do módulo regrediu |
| 6. Commit | Fazer commit pequeno por unidade lógica completa, com testes executados e limitações documentadas | Unidade independente e verificável |

**Não implementar todos os métodos em um único lote.** Organizar entregas por capacidade coesa (ex.: tipos/erros; OAuth/OIDC; metadados; alterações; conteúdo; uploads; demais recursos REST), preservando todas as interfaces previstas para desenvolvimento posterior.

## Autonomia e conclusão da implementação

- **“Implemente o módulo Google API”**, sem restrição adicional, significa executar **todo o escopo do módulo**: os 59 métodos REST, 9 métodos OAuth/OIDC e 10 de transferências/conveniência previstos em [INTERFACES.md](INTERFACES.md), com seus testes aplicáveis em [TESTS.md](TESTS.md). Não encerrar após criar estruturas ou finalizar somente a primeira capacidade.
- O agente deve avançar autonomamente por grupos coesos, com **testes antes do código, implementação mínima, regressão e commit local** por unidade validada. Não necessita pedir confirmação para cada método ou commit local.
- Quando o pedido limitar o trabalho a OAuth, arquivos, alterações, transferências ou outro grupo, executar **somente o escopo solicitado** e registrar os demais como pendentes; não anunciar o módulo como integralmente concluído.
- **Aceite da implementação integral:** contratos públicos implementados, casos TDD aplicáveis executados com servidor HTTP local simulado ou testes locais adequados, regressão sem falhas conhecidas e relatório dos resultados/limitações. Teste simulado não comprova integração real com Google.
- Se contrato, dependência, ambiente Flatpak ou verificação necessária estiver bloqueado, registrar exatamente a pendência e o impacto; **não declarar sucesso fictício nem alterar contrato validado unilateralmente**. As orientações para avaliar divergências oficiais permanecem válidas.
- Não executar `git push`, publicar PR, nem realizar integração com conta real sem autorização específica. Exemplo reutilizável em [IMPLEMENTATION_REQUEST.md](../IMPLEMENTATION_REQUEST.md).

## Regras de arquitetura e fronteiras

| Regra | Exigência |
| --- | --- |
| Isolamento | O módulo compila e tem testes independentes, sem exigir Orquestrador, Analisador, Inventário ou GUI |
| API pública | **Manter todos os métodos inicialmente públicos** conforme `INTERFACES.md`; não reduzir visibilidade sem decisão explícita |
| Implementação interna | Livre para criar submódulos e auxiliares privados; não alterar contratos públicos silenciosamente |
| Google centralizado | URLs, endpoints, scopes, parâmetros, modelos JSON, MIME types, limites e mapeamento de erros ficam somente aqui |
| Contas e credenciais | `accounts/auth/` coordena contas, login, callback e armazenamento seguro; `google_api/` abstrai protocolos e requisições Google |
| Transportes | Interfaces injetáveis/fakes ou servidor HTTP local simulado; evitar dependência de rede real nos testes automáticos |
| Tipos | Dados retornados são tipados; não expor JSON opaco como substituto de tipos públicos definidos |
| Estado | Não criar um segundo inventário, não persistir `SyncItem` e não implementar decisão de sincronização |
| I/O | Operações sobre bytes devem ser explícitas quanto a transferência parcial, retomada, cancelamento e confirmação |
| Segurança | Nunca colocar credenciais, tokens, URLs de sessão secretas ou corpos privados em logs ou mensagens de erro |

## Verificações por operação

| Assunto | Teste mínimo |
| --- | --- |
| Entrada | Parâmetros necessários, inválidos, opcionais e valores-limite |
| URL e HTTP | Método, caminho, query, cabeçalhos, body e `fields` produzidos corretamente |
| Saída | Desserialização, valores opcionais, modelos aninhados, tipagem de int64 e datas |
| Campos omitidos | Distinguir atributo não recebido de ausência comprovada; não inferir conteúdo completo com `fields` parcial |
| Paginação | Cursores, página final, resposta incompleta e falhas intermediárias |
| Erros | Status HTTP e razões do Google mapeados para `GoogleApiFailure` sem vazar informações privadas |
| Retry | Distinguir falhas transitórias de permanentes; não repetir escrita sem considerar idempotência |
| Arquivos | Renomear/mover via metadados sem reenviar conteúdo |
| Transferências | `alt=media`, exportação e `files.download` mantidos distintos; upload `308` não significa conclusão |
| Autenticação | Escopos, PKCE, state, troca e renovação de token, validação OIDC, isolamento entre contas |
| Recuperação | Cancelamento e interrupção não podem declarar operação concluída indevidamente |

## Controle de qualidade da especificação

`INTERFACES.md` é a **primeira versão do contrato**, não prova automática de que todos os campos e assinaturas já reproduzem fielmente o Discovery. Antes de implementar cada operação, validar sua correspondência com a referência oficial. Em caso de divergência:

1. Registrar claramente método, campo ou erro afetado e a documentação oficial correspondente.
2. Corrigir primeiro o contrato e seus testes, preservando as capacidades públicas pretendidas.
3. Só então implementar; **não adaptar testes para mascarar defeitos ou aceitar resultados incorretos**.
4. Não interpretar um campo `Option<T>` não retornado como valor ausente no Drive.
5. Para atualizações parciais, distinguir `Unchanged`, `Set` e `Clear` quando suportado; nunca limpar um campo por omissão acidental.

Se uma operação da referência oficial ainda não for implementada, mantê-la no contrato e identificá-la como pendente; não inserir implementação fictícia que retorne sucesso.

## Ambiente e alterações permitidas

- **Executar Cargo/Rust exclusivamente no ambiente Flatpak** definido pelo manifesto `flatpak/com.rtosta.DriveSync.Devel.yml`, conforme `AGENTS.md` da raiz.
- Testes automáticos devem usar servidor HTTP simulado ou provider fake, arquivos temporários isolados e dados sintéticos. Não exigir conta Google, OAuth real, rede externa nem dados pessoais para concluir os testes.
- Validar com `cargo fmt`, `clippy`, `test`, `doc` e doctests quando disponíveis **dentro do Flatpak**; registrar o que foi efetivamente executado.
- Não alterar GUI, módulos consumidores ou a arquitetura de sincronização por conveniência. Integração com a engine existente pertence a outra etapa.
- Comentários e mensagens de commit **em inglês**, conforme regras globais.
- Não realizar `git push` ou publicar PR sem autorização explícita; commits locais seguem `AGENTS.md` da raiz.

## Entrega de cada etapa

Informar: **contratos implementados**, **testes escritos primeiro**, **resultado dos testes**, **referências oficiais verificadas**, **erros/limitações conhecidos** e **contratos ainda pendentes**. Não declarar integração real com o Google sem executá-la e verificá-la explicitamente.
