# Providers — orientações para agentes

## Missão

Manter exclusivamente a camada de mapeamento direto das APIs externas. Cada provedor abstrai a comunicação com seu serviço e expõe as operações oficiais, sem implementar fluxos funcionais da aplicação.

## Regras

1. Não criar lógica de sincronização, download/upload como processos gerenciados, filas, conflitos ou decisões de negócio.
2. Mapear endpoints, parâmetros, requisições, respostas, erros e autenticação técnica da API.
3. Não guardar tokens, contas ou credenciais de forma persistente.
4. Não criar um contrato universal, `trait`, fábrica ou catálogo obrigatório sem requisito concreto de implementação.
5. Não referenciar consumidores nem descrever dependências funcionais de outros módulos.
6. Cada serviço tem seu próprio submódulo; `google/` é o mapeamento direto, sem `google/api/` redundante.
7. Preservar as operações oficiais e os contratos técnicos documentados, sem alegar implementação ou testes não verificados.
8. Definir a distribuição interna do código em arquivos somente durante a implementação.
9. Manter os quatro documentos `README.md`, `INTERFACES.md`, `TESTS.md` e `AGENTS.md` com seus papéis próprios.
10. Seguir também as regras gerais do [AGENTS.md](../../../AGENTS.md) da raiz.

[Google](google/README.md) · [Interfaces](INTERFACES.md) · [Testes](TESTS.md)
