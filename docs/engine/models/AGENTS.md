# Agentes — Models (tipos de domínio compartilhados)

## Escopo

Implementar **somente o módulo `models/`**, seguindo [README.md](README.md), [INTERFACES.md](INTERFACES.md), [TESTS.md](TESTS.md), [padrão](../MODULE_STANDARD.md) e [AGENTS raiz](../../../AGENTS.md).

Este é um módulo de **representação de dados**, não de operações. Não criar traits `ModelManager`, `Repository`, `RemoteProvider`, `RemoteDetector` ou métodos para executar chamadas de rede, consultas SQL, leitura de arquivo, upload ou download.

## Regras de ownership e desacoplamento

- `RemoteFile` e `LocalFile` são valores comuns, sem efeitos colaterais.
- `RemoteFile` possui **atributo opaco de dados específicos do provedor**, mas `models/` não importa `GoogleFile` nem tipos do OneDrive.
- `GoogleFile` permanece propriedade de `providers/google/api/`; codificação/decodificação específica pertence ao especialista que conhece o provedor.
- Não incluir token, sessão, cliente HTTP, conexão SQLite ou bytes de conteúdo de arquivo nos modelos.
- IDs de arquivo são relativos à conta; preservar identidade por `RemoteFileRef`.
- Reutilizar os tipos de identidade já existentes ou propor migração explícita: **não criar tipos duplicados incompatíveis**.
- `SyncItem`, baseline e estados de presença são propriedades do `inventory/` até decisão posterior. Não mover contratos ou modificar seu banco para concluir esta etapa.
- Campos opcionais não são prova de ausência física; informações não consultadas não causam limpeza automática.
- Funções de validação/conversão **puras** são permitidas; implementação de API externa ou I/O pertence aos módulos especialistas.
- Cada modelo pode ter arquivo Rust próprio por coesão; não criar divisões artificiais se uma implementação simples bastar.

## Desenvolvimento independente

1. Validar primeiro com os tipos existentes em `engine/src/models.rs` e no módulo de estado local.
2. Resolver pendências de owner, path e payload antes de implementar contratos dependentes, sem adivinhar.
3. Escrever testes RED para invariantes e casos de `TESTS.md`.
4. Implementar GREEN e REFACTOR sem implementar módulos consumidores ou provedores.
5. Reportar interfaces de dados concluídas, testes reais, comandos, limitações e commits.

Executar Rust/Cargo exclusivamente no Flatpak do projeto; comentários/commits em inglês; não fazer push nem PR sem autorização explícita. Criar código fora do módulo ou migrar o Inventário exige uma solicitação separada.

**Estado:** instrução de implementação futura; nenhum código foi alterado nesta documentação.
