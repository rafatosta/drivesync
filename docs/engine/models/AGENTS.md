# Agentes — Models: implementação futura fiel ao contrato

## Estado

**Apenas documentação nesta etapa. Não implementar código Rust.** A referência de atributos, tipos, presença e invariantes é [INTERFACES.md](INTERFACES.md); [README.md](README.md) apresenta responsabilidades e [TESTS.md](TESTS.md) define os testes correspondentes.

Os contratos foram escritos para revisão conceitual. Uma futura implementação deve reproduzir a versão **explicitamente aprovada**, não completar requisitos faltantes nem recuperar atributos de documentos anteriores.

## Tipos e propriedade

- `models/` define: `LocalFile`, `RemoteFile`, `FileData`, `File` e tipos auxiliares pertencentes a esse domínio.
- `FileData` contém **exatamente uma** variante `Local(LocalFile)` ou `Remote(RemoteFile)`.
- `File` contém contexto da ocorrência e `Option<FileData>`, com invariantes explícitos de escopo/evento/cobertura.
- `FileId` é **identidade de ocorrência**; IDs de recurso permanecem nos modelos específicos.
- `FileBatch` pertence **exclusivamente a `detectors/`** e não pode ser redefinido em Models.
- `SyncItem` não entra agora. Só será avaliado durante a revisão futura do planejamento do Inventário.
- `GoogleFile` continua em `providers/google/api/`. Models não importa nenhum tipo de serviço concreto.

## Restrições

1. Não criar trait operacional como `ModelManager`, repositório de dados ou métodos com I/O.
2. Não alterar nomes, tipos, obrigatoriedade ou variantes descritos em `INTERFACES.md` sem revisão explícita.
3. Reutilizar os tipos de identidade existentes ou planejar migração de proprietário, **nunca criar duplicatas incompatíveis**.
4. Não preencher `None` com zero, string vazia, tempo atual ou hash presumido.
5. `FieldValue::Unknown` e `Known([])` são distintos; não converter desconhecido em vazio.
6. `LocalRelativePath` preserva bytes Unix; rejeitar caminho absoluto, NUL e segmentos de travessia.
7. `ProviderFileData` guarda somente metadados e nunca credenciais, respostas não sanitizadas ou conteúdo de arquivos.
8. Não interpretar `File.data=None` como exclusão.
9. Não mudar implicitamente Detector Local, Detector pai, Storage, Inventário ou Analisador ao desenvolver Models.
10. Não inventar `SyncItem`, estado de baseline, fila ou decisões de conflito em `File`.

## Pontos que exigem resolução antes da implementação

- Ownership real de `AccountId`, `SyncId` e `LocalFileId` diante dos tipos existentes no Rust.
- Limite máximo, codificação concreta e política de retenção dos dados de `ProviderFileData.payload`.
- Estratégia de validação de `RemoteFile.provider` contra a conta configurada sem fazer Models depender de Accounts.
- Ajuste coordenado do Detector pai/Storage para transporte durável do objeto `File` e `FileBatch`.

Esses pontos **não autorizam** criar novos módulos ou esquemas por iniciativa do agente. Quando bloquearem implementação, relatar lacuna e solicitar revisão contratual.

## TDD futuro

Escrever testes puros para cada atributo e invariante enumerado em [TESTS.md](TESTS.md); depois implementar apenas os tipos aprovados. Testes de FileBatch, leases, retry e ack pertencem ao Detector/Storage, não a Models.

Execução Cargo/Rust somente em Flatpak, conforme [MODULE_STANDARD.md](../MODULE_STANDARD.md) e [AGENTS raiz](../../../AGENTS.md). A entrega atual é documental: sem compilação, commit de código, push de alterações Rust ou PR.
