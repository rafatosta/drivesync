# Operador — Example

## Objetivo

Ilustrar como uma nova implementação especializada seria organizada dentro de `operations/`, usando um provedor fictício.

## Contrato

Uma nova implementação deve satisfazer todos os métodos do [`OperationExecutor`](../INTERFACES.md#7-contrato-de-implementação-especializada):

`create_file`, `create_directory`, `upload`, `download`, `update_content`, `rename`, `move_item`, `delete`, `trash` e `restore`.

Cada método recebe os tipos definidos pelo Operador e retorna um resultado tipado. Capacidades indisponíveis retornam erro apropriado. As funções privadas de filesystem do Operador podem ser reutilizadas, e os acessos a APIs externas são disponibilizados pelo Provider correspondente.

## Validação

Toda implementação deve executar a [suíte de conformidade do Operador](../TESTS.md#conformidade-dos-executores-especializados), complementada por testes próprios.

**Estado:** exemplo exclusivamente documental, sem provedor real nem código implementado.
