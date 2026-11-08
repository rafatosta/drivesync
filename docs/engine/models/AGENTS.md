# Agentes — Models (contratos de dados propostos)

## Situação e fonte de verdade

**Documentação conceitual; NÃO implementar código Rust nesta etapa.**

As definições de atributos, tipos, variantes, ausência e invariantes estão em [INTERFACES.md](INTERFACES.md). O [README.md](README.md) apresenta responsabilidades e uma tabela resumida; [TESTS.md](TESTS.md) traduz essas regras em casos verificáveis. A implementação futura deverá ser **reflexo literal da especificação após aprovação do responsável**, sem acrescentar ou alterar campos por preferência do agente.

**Atributos documentados agora são PROPOSTOS, não aprovados para implementação automaticamente.**

## Proprietários

- `models/`: `LocalFile`, `RemoteFile`, `FileData`, `File` e auxiliares explicitamente definidos.
- `detectors/`: `FileBatch`, identificação de lote, lease, fila e confirmação. Models apenas explica a relação, não redefine o lote.
- `providers/google/api/`: `GoogleFile` e protocolo Google; não importar `GoogleFile` em Models.
- `inventory/`: propriedade atual de `SyncItem` até revisão específica. **Não introduzir nem migrar `SyncItem` nesta fase.**

## Restrições literais

1. `FileData` tem somente `Local(LocalFile)` e `Remote(RemoteFile)`. Não inventar `Both`, `Google`, `Unknown` ou lógica operacional.
2. `File` encapsula os **sete campos propostos**: `id`, `sync_id`, `event`, `observed_at`, `coverage`, `scope`, `data`; não misturar com `SyncItem`.
3. `File.id` identifica **a ocorrência**, não o arquivo físico; replay da mesma ocorrência preserva ID e payload.
4. `LocalFile` e `RemoteFile` só armazenam os valores efetivamente observados. `None` não significa zero, ausência física ou comando de exclusão.
5. `FieldValue::Unknown` e `FieldValue::Known([])` são semanticamente diferentes; não simplificar ambos para lista vazia.
6. `ProviderFileData` contém somente metadados opacos, nunca tokens, credenciais, respostas completas não filtradas ou conteúdo do arquivo.
7. Não construir I/O de arquivo, rede, SQLite, fila, autenticação, operações, detector ou analisador dentro de Models.
8. Não criar `ModelManager`, `Repository` ou trait operacional para justificar abstrações desnecessárias.
9. Não duplicar tipos já existentes (`AccountId`, `SyncId`, `LocalFileId` etc.). Propor migração/reexportação sob revisão explícita.
10. Não definir limite/codificação de `ProviderFileData.payload` enquanto estiver pendente; bloquear implementação afetada, em vez de adivinhar.
11. A construção de `File` deve respeitar as relações entre `event`, `scope`, `data` e `coverage` definidas no contrato; não fabricar dados para contornar validação.
12. Não alterar `detectors/`, `detectors/storage/`, `inventory/` ou `operations/` para fazer compilar um exemplo conceitual de Models.

## Execução futura

Após aprovação: validar tipo a tipo, criar testes unitários puros RED → GREEN → REFACTOR conforme [TESTS](TESTS.md), executar Cargo/Rust apenas no Flatpak do projeto e relatar PASS/FAIL/BLOCKED. Integração e testes de lote/ack pertencem ao Detector, não aos testes de Models.

Para regras não especificadas, registrar como pendência e **solicitar revisão do contrato antes de codificar**. Não converter comentários de exemplos anteriores em requisitos não aprovados.

[MODULE_STANDARD](../MODULE_STANDARD.md) · [AGENTS raiz](../../../AGENTS.md).
