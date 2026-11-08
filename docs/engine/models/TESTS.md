# Models — TESTS (invariantes e contratos de dados)

## 1. Objetivo e isolamento

Testar **tipos de dados e funções puras**, sem Google API, HTTP, filesystem real, SQLite, Detector, Inventário, Analisador ou Operadores. O teste de `models/` é autônomo. Não criar mocks de banco para justificar um serviço que o módulo não possui.

A ausência de métodos operacionais não significa ausência de testes: verificar a identidade, construção válida, ausência de dados, serialização quando aplicável e preservação de informações específicas do provedor.

## 2. Matriz TDD

| Caso | Modelo/regra | Sucesso | Falha/limite |
| --- | --- | --- | --- |
| MOD-01 | `RemoteFileRef` | Mesma conta + mesmo ID identifica mesmo recurso | Duas contas com ID bruto igual não colidem |
| MOD-02 | `AccountId` | Reutilização do tipo proprietário existente | Não redefinir tipo incompatível em outro módulo |
| MOD-03 | `RemoteFile` | Pode ser construído com atributos opcionais ausentes | `None` não é exclusão nem ordem de apagar dados |
| MOD-04 | `RemoteFileKind` | Arquivo, diretório, nativo, outro e desconhecido | `Unknown` não é convertido em `File` por extensão |
| MOD-05 | `RemoteVersion` | Preserva versão opaca integralmente | Não comparar numericamente versões de provedores diferentes |
| MOD-06 | `Checksum` | Mesmo algoritmo + digest comparável | Algoritmos incompatíveis não são tratados como equivalentes |
| MOD-07 | `LocalFile` | Guarda dados recebidos do observador | Construção não faz acesso físico ao disco |
| MOD-08 | `LocalRelativePath` | Preserva componentes não UTF-8 válidos no Unix | Caminho inválido não é normalizado silenciosamente |
| MOD-09 | `ProviderFileData` | Preserva bytes, provedor, formato e versão sem interpretar | Formato/versão inesperado só pode ser interpretado pelo especialista |
| MOD-10 | Envelope opcional | `provider_data=None` permite arquivo genérico | Ausência de payload não apaga outros campos |
| MOD-11 | Conta e provedor | Conta seleciona seu provedor via vínculo externo | Modelo não lê sessões nem credenciais |
| MOD-12 | Campos parciais | Produtor pode distinguir não solicitado de ausência comprovada | Não sobrescrever valores confirmados com campos omitidos |
| MOD-13 | Serialização, se adotada | Round-trip conserva identidade, opções e dados opacos | Formatos inválidos geram erro, nunca perda silenciosa |
| MOD-14 | Isolamento de módulo | `models/` compila e testa sem módulos de serviços concretos | Não deve importar Google, SQLite, Auth, Detector ou Inventário |
| MOD-15 | Proteção do payload | Limites de tamanho definidos são respeitados | Rejeitar payload acima do limite sem travar a aplicação |

## 3. Testes das interfaces públicas de dados

### Identidade

Criar `RemoteFileRef` em duas contas diferentes com `file_id` idêntico; devem permanecer referências diferentes. Verificar igualdade/hashing consistente se esses traits forem publicados. Testar identidade inválida sem consultar o módulo Accounts.

### Metadados

Construir `RemoteFile` com todos os atributos disponíveis, depois outro somente com obrigatórios; verificar que atributos desconhecidos continuam `None` e que o objeto não representa exclusão. Verificar `RemoteFileKind::Unknown` e pais ainda não consultados.

### Dados específicos

Construir envelope `ProviderFileData` com bytes arbitrários não sensíveis e metadados de formato. Garantir preservação integral e round-trip conforme serialização adotada. **Não exigir que models saiba decodificar `GoogleFile`.** A conversão Google pertence aos testes do especialista Google.

### LocalFile

Validar caminho relativo, codificação do nome e comportamento de identificador local opcional. Verificar que criar/clonar/comparar valores não abre nem modifica arquivos.

### Fronteira de compilação

O módulo deve ser testável sem instanciar `InventoryManager`, `GoogleDrive`, `RemoteDetector` ou objetos de conta autenticada. Falhas por dependência circular ou importação concreta são regressões arquiteturais.

## 4. Técnica e aceite

RED → GREEN → REFACTOR por grupo de valores; testes unitários/property-based onde necessário. Executar Cargo/Rust **somente no ambiente Flatpak** conforme padrão do projeto. Nenhum teste de API/SQLite é requisito para concluir `models/`.

Relatório deve indicar PASS/FAIL/BLOCKED por ID. Os contratos de identidade, opções e dados opacos deverão ser resolvidos antes dos testes correspondentes. **Nesta etapa, foram criados apenas documentos; nenhum teste foi executado.**
