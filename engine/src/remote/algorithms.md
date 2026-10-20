# Algoritmos — remote

**Status:** proposta v0.2, independente da linguagem.

## Identificadores
REMOTE-001 paginação; REMOTE-002 operações; REMOTE-003 retry.

## Objetivo
Adaptador REST da Drive API v3.

## Pré-condições
Configuração validada, dependências disponíveis ou erro explícito, estado prévio recuperável.

## Algoritmo conceitual
1. Obter token.
2. enviar request com timeout.
3. paginar.
4. converter IDs/revisões para modelo interno.
5. classificar status.
6. backoff com jitter.

## Pós-condições
Estado confirmado e persistido, ou erro tipado com efeitos parciais identificáveis.

## Possíveis erros
Entrada inválida; perda de conectividade; autorização negada; falta de espaço; versão alterada; erro de I/O; estado desconhecido; falha entre efeito externo e commit.

## O que verificar
IDs e revisões; pré-condições imediatamente antes de ações destrutivas; retorno e efeitos persistidos; invariantes globais e limites desta caixa-preta.

## O que não verificar nem inferir
Não inferir ausência por timeout/erro HTTP, nem identidade apenas pelo nome, nem sucesso pela ausência de exceções. Não inspecionar conteúdo do usuário sem necessidade da operação.

## Invariantes
403/404/429/5xx são erros distintos. Aplicam-se também os invariantes globais de `docs/invariants.md`.

## Complexidade
Consultas pontuais tipicamente O(1) em metadados; inventário e planejamento O(n); transferências de B bytes O(B). Latência de rede, disco e consistência entre operações dominam o custo real.

## Critérios de aceitação
Caso nominal, cancelamento, repetição sem duplicação indevida, erro transitório, erro definitivo, falha parcial e reinicialização. Revisar as decisões pendentes antes de codificar.


## Fundamentação técnica por algoritmo

Esta seção distingue **solução consolidada**, **decisão do DriveSync**, **teste verificável** e **hipótese ainda aberta**. Consulte [método de desenvolvimento](../../../docs/development-methodology.md) e [catálogo de fontes](../../../docs/reference-catalog.md). Os links são referências de projeto, não provas de que a implementação esteja pronta.

### REMOTE-001

- **Técnica conhecida:** Paginação e identidade de recurso remoto.
- **Referências primárias:** [files.list v3](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/list), [changes.list](https://developers.google.com/workspace/drive/api/reference/rest/v3/changes/list).
- **Política do DriveSync:** Percorrer todas as páginas, tratar IDs como identidade; não derivar unicidade do nome.
- **Testes/oráculos a detalhar:** Múltiplas páginas, IDs repetidos, nomes iguais em pastas diferentes, resposta parcial.
- **Limitações / questões abertas:** Validação de corpus com pastas compartilhadas e restrições de scopes.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.

### REMOTE-002

- **Técnica conhecida:** Operações tipadas Drive API.
- **Referências primárias:** [files resource v3](https://developers.google.com/workspace/drive/api/reference/rest/v3/files), [Google Drive scope guide](https://developers.google.com/workspace/drive/api/guides/api-specific-auth).
- **Política do DriveSync:** Converter capacidades remotas em resultados tipados; 404 não comprova exclusão, pode ser perda de acesso.
- **Testes/oráculos a detalhar:** Não encontrado, proibido, arquivo Workspace, mover e renomear.
- **Limitações / questões abertas:** Semântica de atualização condicional/versão remota não presumida: prova separada obrigatória.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.

### REMOTE-003

- **Técnica conhecida:** Retentativas exponenciais e classificação de erros.
- **Referências primárias:** [Google Drive errors](https://developers.google.com/workspace/drive/api/guides/handle-errors), [HTTP RFC 9110](https://www.rfc-editor.org/rfc/rfc9110).
- **Política do DriveSync:** Retry limitado, jitter e Retry-After; diferenciar 401, 403 por reason, 404, 429 e 5xx.
- **Testes/oráculos a detalhar:** 429 em rajadas, erro 403 rateLimitExceeded, 5xx, erro definitivo 403.
- **Limitações / questões abertas:** Limites reais de quota por projeto e por usuário precisam de medição.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.
