# Agentes — Models (núcleo de domínio)

## Estado da especificação

**Somente documentação conceitual nesta fase. NÃO implementar código.**

A implementação futura deve ser um **reflexo literal dos contratos aprovados** de [README.md](README.md), [INTERFACES.md](INTERFACES.md) e [TESTS.md](TESTS.md). Um campo apresentado em discussão ou em uma versão anterior não constitui requisito atual se não estiver explicitamente aprovado.

## Hierarquia e proprietários

- `LocalFile`, `RemoteFile`, `FileData` e `File`: **`models/`**.
- `FileData`: enum conceitualmente fechado em `Local(LocalFile)` ou `Remote(RemoteFile)`.
- `File`: observação autocontida que encapsula dados e contexto; **atributos ainda não definidos**.
- `FileBatch`: **`detectors/`**, agrega `File` para entrega; **não** implementar nem redefinir aqui.
- `SyncItem`: decisão **adiada até a revisão do planejamento de `inventory/`**; não adicionar ao Models agora.
- `GoogleFile`: pertence a `providers/google/api/`; não importar esse tipo para Models.

## Limites de responsabilidade

`models/` é apenas representação de domínio, com eventuais validações puras. Não incluir acesso à rede, filesystem, SQLite, autenticação, lógica do Analisador, fila de notificação, confirmação, upload ou download. Não criar `ModelManager` ou interfaces operacionais.

O Detector Local/Remoto constrói dados especializados e os encapsula na interface de observação; o Detector pai e Storage preservam e entregam `File`; o Orquestrador não precisa distinguir a variante e o Analisador a interpreta. Não duplicar modelos em submódulos.

## Proibições enquanto os atributos estiverem em aberto

- Não definir campos, valores padrão, variantes adicionais, tipos de IDs, timestamps, enums de eventos ou política de ausência por conta própria.
- Não transformar nomes ilustrativos de campos em contrato compilável.
- Não escolher implicitamente entre `Option<FileData>`, enum especial para ausência ou outro formato.
- Não duplicar identidades já existentes; compatibilidade de tipos será revisada na etapa de atributos.
- Não mover `SyncItem` ou alterar o Inventário, Detector ou Storage como parte desta fase.
- Não declarar testes aprovados por comparação com esquemas hipotéticos.

## Próxima etapa

Definir e aprovar **cada atributo** com nome, tipo, obrigatoriedade, origem, semântica de ausência e invariantes; depois atualizar `TESTS.md` para refletir a especificação literal. Somente então solicitar implementação independente em Rust, seguindo [MODULE_STANDARD.md](../MODULE_STANDARD.md) e [AGENTS raiz](../../../AGENTS.md).

**Estado:** documentos atualizados; nenhum código Rust ou teste foi executado.
