# Detector Remoto Google — INTERFACES

## 1. Contrato adotado

A implementação `GoogleRemoteDetector` deve satisfazer **exatamente a interface interna** [RemoteDetector](../INTERFACES.md) quanto a ciclo de vida, observação, estado e emissão individual. Ela não define nova API pública para o Orquestrador.

## 2. Dependências injetadas

- Cliente de operações Google Drive conforme [Google API](../../../providers/google/api/INTERFACES.md).
- Contexto de conta e sessão **tipados**, sem manipular o armazenamento de credenciais.
- `RemoteEventSink` e `RemoteCheckpoint` definidos no módulo pai.
- Relógio, scheduler e estratégia de retry injetáveis quando necessários ao teste.

A tradução entre respostas `GoogleFile` e o evento remoto é **responsabilidade deste especialista**, não da fachada genérica. A representação de `RemoteFile` será definida separadamente; não se exige formato novo para construir esta documentação.

## 3. Mapeamento de operações

| Interface pai | Google especialista |
| --- | --- |
| `new(RemoteCreateRequest)` | Valida contexto de conta, raiz Google e dependências Google |
| `start(RemoteStartRequest)` | Configura estratégia de polling Google e retomada do cursor |
| `poll_once(RemotePollRequest)` | Consulta Google Drive por páginas e emite um evento por chamada ao sink |
| `stop(RemoteStopRequest)` | Interrompe consultas sem apagar estado confirmado |
| `status(RemoteStatusRequest)` | Informa estado e falhas sem expor token/JSON |

## 4. Semântica e falhas

- Resposta parcial não prova ausência; `removed=true` não é necessariamente exclusão.
- Cursor só avança após o sink confirmar aceite durável de todos os eventos correspondentes à página.
- Erros da Google API são **traduzidos** em erros do contrato remoto, sem vazar detalhes HTTP desnecessários.
- Replay da mesma página pode reemitir evento com identidade estável quando houver resultado incerto.
- A estratégia Google pode ser diferente das demais, desde que o contrato de saída seja preservado.

**Não redefinir** os cinco métodos ou os modelos base de `RemoteDetector`. Mudanças no contrato pai exigem revisão separada.
