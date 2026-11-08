# Google — TESTS

> Testes de **obtenção da API Google**; não substituem [os testes técnicos da API](api/TESTS.md). Cenários documentados, ainda não executados.

| ID | Situação | Resultado esperado |
| --- | --- | --- |
| GPR-01 | Catálogo seleciona `google` | Retorna interface Google correta |
| GPR-02 | Inicialização com configuração válida | Cliente Google utilizável |
| GPR-03 | Configuração incompleta ou inválida | Erro de inicialização, sem handle fictício |
| GPR-04 | Referência aos métodos mapeados | Interface entregue contém o contrato técnico já documentado |
| GPR-05 | Obtenção repetida da API | Sem divergência de tipo/configuração entre instâncias |
| GPR-06 | API exige `GoogleSession` em uma função | Parâmetro pertence à função técnica, não a um login automático da fábrica |
| GPR-07 | Operação OAuth/OIDC disponível | Exposta como função da mesma API Google, sem outro serviço paralelo |
| GPR-08 | Erro técnico da API | Preserva `GoogleApiFailure` correspondente |
| GPR-09 | Inspeção da implementação | Não há segunda cópia de Files/Changes/OAuth no módulo pai |
| GPR-10 | Inspeção de dados e logs | Sem tokens ou valores secretos registrados pela infraestrutura de inicialização |

## Separação de cobertura

- `google/TESTS.md` verifica obtenção, configuração e disponibilidade da API.
- [`google/api/TESTS.md`](api/TESTS.md) especifica a cobertura dos **78 métodos técnicos**, seus tipos, erros e chamadas.

Não documentar cenários de uso, estado ou política funcional de outros módulos. Não declarar `PASS` sem execução; detalhes de handle Rust permanecem `BLOCKED` até decisão técnica.
