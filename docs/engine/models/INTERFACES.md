# Models — INTERFACES (contratos conceituais)

## Estado deste documento

**Etapa 1 — papéis e encapsulamento: aprovada. Etapa 2 — atributos: pendente.**

Este documento **não** descreve código Rust existente e **não autoriza implementar estruturas definitivas**. A implementação deverá ser reflexo literal da descrição aprovada, não de exemplos ou decisões do agente.

`models/` não exige uma interface operacional (`ModelManager`, trait de armazenamento, `scan()`, `save()` etc.). Suas interfaces são os **próprios tipos de dados** e suas regras. As interfaces operacionais permanecem com Detector, Inventário, Provedores e Operações.

## 1. LocalFile

**Responsabilidade:** representar os dados de um arquivo/diretório **local** observados pelo componente apropriado. Não lê o disco, não calcula automaticamente hash, não decide exclusão e não conhece sincronização.

**Consumidor:** `FileData::Local`, após a observação do Detector Local.

**Atributos:** **a definir na etapa 2**, incluindo identidade, localização, tipo, metadados disponíveis e semântica da ausência de valores. Nenhum atributo apresentado em documentos anteriores deve ser tratado como decisão final.

## 2. RemoteFile

**Responsabilidade:** representar os dados de um arquivo/diretório **remoto** em formato consumível pelos módulos genéricos. Não consulta API, não autentica e não depende de `GoogleFile` como tipo obrigatório.

**Consumidor:** `FileData::Remote`, após adaptação no especialista do Detector Remoto.

**Dados específicos do provedor:** o objeto poderá encapsulá-los sem exigir que consumidores genéricos interpretem Google/OneDrive. O **formato, os campos e a estratégia de armazenamento desses dados ainda precisam ser definidos**.

**Atributos:** **a definir na etapa 2**; não presumir os campos de `GoogleFile` como lista completa, obrigatória ou universal.

## 3. FileData — discriminação de conteúdo

**Responsabilidade:** encapsular **exatamente uma** representação de arquivo — local **ou** remota — sem que o Orquestrador tenha de lidar com tipos separados.

A **única forma conceitual aprovada** é:

```rust
pub enum FileData {
    Local(LocalFile),
    Remote(RemoteFile),
}
```

O exemplo estabelece **variantes e relação de tipos**. Ownership Rust, `derive`, serialização e forma dos tipos internos ainda não estão definidos.

**Regras:**
- `Local` e `Remote` são alternativas, não dois campos preenchidos simultaneamente.
- `FileData` **não** indica por si só alteração, exclusão, instante ou confirmação.
- O Analisador interpreta a variante. O Orquestrador apenas encaminha o `File` que a encapsula.
- O Detector ou consumidor não deve converter indisponibilidade de uma observação em uma variante inventada.

## 4. File — observação autocontida

**Responsabilidade:** encapsular os **dados observados** (`FileData`, quando disponíveis) e **todo o contexto necessário para que o Analisador compreenda a ocorrência** sem pedir ao Detector que reconstrua a notificação.

`File` é o objeto unitário de comunicação. **Não é** necessariamente um arquivo existente no momento de consumo; pode representar uma ocorrência que exige reavaliação de um caminho/escopo, mesmo quando não há metadados completos.

**Não pertence ao File:** estado consolidado de sincronização (`SyncItem`), decisão de conflito, execução de operação, posse da fila ou autorização destrutiva.

**Atributos:** **a definir na etapa 2.** Ainda não está decidido se `FileData` será campo opcional, enum maior com casos sem arquivo ou outra modelagem explícita. Também estão em aberto identidade da notificação, motivo/tipo de ocorrência, instante, escopo, cobertura e associação com conta/sincronização. Não converter esses exemplos em campos obrigatórios sem revisão.

## 5. FileBatch — agregado de entrega (definido em detectors/)

**Responsabilidade:** transportar **uma coleção de `File`** para o Orquestrador e permitir que o Detector gerencie entrega e confirmação segura.

```text
FileBatch
└── files: coleção de File
```

**Proprietário:** [`detectors/`](../detectors/INTERFACES.md), **não `models/`**. Sua presença aqui documenta a relação entre os tipos, sem duplicar o contrato do Detector.

**Atributos:** lista de `File` é a relação **conceitualmente aprovada**. Identidade do lote, token/lease, tamanhos máximos, políticas de vazio, acknowledgements e assinaturas de métodos permanecem no contrato do Detector e serão ajustados quando a etapa 2 for concluída.

**Uso:** o Detector armazena/agrupa as observações e entrega `FileBatch`; o Orquestrador recebe a lista sem realizar `match` em `FileData`; o Analisador identifica as variantes e decide a análise.

## 6. Fluxo e ownership

```text
LocalFile  ─┐
            ├─> FileData ─> File ─> FileBatch ─> Orquestrador ─> Analisador
RemoteFile ─┘                 (models/)  (detectors/)
```

- `models/` não importa `detectors/`, `inventory/` ou `providers/`.
- Detectores podem depender dos contratos de `models/`.
- `FileBatch` utiliza `File`, mas `File` não conhece `FileBatch`.
- `SyncItem` só será tratado quando o planejamento do Inventário for revisado.
- Storage guarda **o objeto completo necessário para reentrega**, sem perder seu conteúdo; confirmações devem impedir perda após falha. A modelagem da fila é propriedade do Detector/Storage.

## 7. Questões reservadas para a etapa 2

1. Atributos e tipos exatos de `LocalFile`.
2. Atributos e tipos exatos de `RemoteFile`, inclusive dados específicos do provedor.
3. Atributos e invariantes de `File` (inclusive representação de evento sem arquivo observado).
4. Identidades compartilhadas, timestamps, caminhos, cobertura, completude e regras de opcionalidade.
5. Validação, conversões puras e eventual serialização de dados, sem implementar acesso externo.
6. Compatibilidade futura com o Inventário e evolução coordenada dos contratos do Detector/Storage.

**Regra de interpretação:** até aprovação desses pontos, **não há contrato literal de campos implementável**. Não reutilizar automaticamente exemplos anteriores de structs, métodos ou campos como se fossem decisões finais.
