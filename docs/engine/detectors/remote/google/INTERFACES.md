# Detector Remoto Google — INTERFACES

## 1. Contrato adotado

A implementação `GoogleRemoteDetector` deve satisfazer **exatamente a interface interna** [RemoteDetector](../INTERFACES.md) quanto a ciclo de vida, observação, estado e emissão individual. Sua interface abrange apenas observação, estado e emissão das ocorrências.

## 2. Dependências injetadas

- Interface de operações Google Drive **disponibilizada por [`providers/`](../../../providers/INTERFACES.md)**, cuja interface específica está em [Google API](../../../providers/google/INTERFACES.md).
- Contexto de conta e sessão **tipados**, sem manipular o armazenamento de credenciais.
- `RemoteEventSink` e `RemoteCheckpoint` definidos no módulo pai.
- Relógio, scheduler e estratégia de retry injetáveis quando necessários ao teste.

A tradução entre respostas `GoogleFile` e o evento remoto é **responsabilidade deste especialista**, não de Providers. O especialista pode usar os tipos e as chamadas específicas Google; a interface padronizada é o contrato interno do Detector Remoto. A representação de `RemoteFile` está em [Models](../../../models/INTERFACES.md); o especialista preserva os metadados normalizados e específicos sem novas APIs públicas.

## 3. Mapeamento de operações

| Interface pai | Google especialista |
| --- | --- |
| `new(RemoteCreateRequest)` | Valida contexto de conta, raiz Google e dependências Google |
| `start(RemoteStartRequest)` | Configura estratégia de polling Google e retomada do cursor |
| `poll_once(RemotePollRequest)` | Consulta Google Drive por páginas e emite um evento por chamada ao sink |
| `stop(RemoteStopRequest)` | Interrompe consultas sem apagar estado confirmado |
| `status(RemoteStatusRequest)` | Informa estado e falhas sem expor token/JSON |

## Associação GoogleFile → RemoteFile

O especialista Google é o **único componente que conhece simultaneamente `GoogleFile` (API) e `RemoteFile` (Models)**. A adaptação consiste em:

1. Verificar a conta Google configurada e o ID real do recurso; formar `RemoteFileRef(account_id, file_id)` sem inventar ID ou usar nome como identidade.
2. Normalizar **somente campos comprovadamente fornecidos** pela resposta Google, distinguindo `false`, `0`, `[]` de campo não solicitado por `fields`.
3. Preservar metadados especializados aprovados para retenção em `ProviderFileData` opaco, identificado por provedor/formato/versão, **sem tokens, chaves de criptografia, conteúdo de arquivo ou material sensível desnecessário**.
4. Entregar o `RemoteFile` completo à porta genérica; o Detector não inspeciona campos nem precisa mudar quando o esquema evoluir.
5. Preservar `GoogleChange.file_id` em File.scope mesmo quando File está ausente. Sem identidade válida, falhar a página sem avançar cursor; não fabricar um RemoteFile nem inferir exclusão.

Rotinas auxiliares do especialista podem consultar detalhes adicionais (permissões, labels, atalhos, capacidades etc.) sem alterar polling, fila ou fachada. **Antes da implementação**, revisar a necessidade de cada campo e documentar `manter / sob demanda / descartar`, conforme [Models — INTERFACES](../../../models/INTERFACES.md#3-remotefile--metadados-remotos-e-associação-ao-provedor).

## 4. Semântica e falhas

- Resposta parcial não prova ausência; `removed=true` não é necessariamente exclusão.
- Cursor só avança após o sink confirmar aceite durável de todos os eventos correspondentes à página.
- Erros da Google API são **traduzidos** em erros do contrato remoto, sem vazar detalhes HTTP desnecessários.
- Replay da mesma página pode reemitir evento com identidade estável quando houver resultado incerto.
- A estratégia Google pode ser diferente das demais, desde que o contrato de saída seja preservado.

**Não redefinir** os cinco métodos ou os modelos base de `RemoteDetector`. Mudanças no contrato pai exigem revisão separada.

## 5. Unidade de ocorrência emitida

`RemoteEventSink.emit(file: File)` recebe uma observação completa com kind
`Observed`, sem interpretação de ação. `File.event.remote` contém a origem
Capture/Changes e os sinais opcionais removed/time/changeType/driveId recebidos.
Os metadados disponíveis viram FileData::Remote; sua ausência mantém data=None
e a identidade qualificada em File.scope. `previous` permanece None.

A captura inicial percorre a raiz configurada. Changes repassa as alterações da
conta, sem consultar pais ou filtrar por pertencimento. Não existe comparação
com histórico, deduplicação por igualdade de metadados nem classificação de
rename/move/edit/delete. A idempotência se aplica à reentrega da mesma ocorrência,
não à supressão de ocorrências diferentes com os mesmos metadados.

O checkpoint contém somente controle de coleta e página pendente para replay.
O histórico de observações pertence à fila; o estado conhecido dos arquivos e
a interpretação pertencem a Inventory/Analisador. Falhas de coleta continuam
sendo avisos técnicos RecheckRequired/ScopeUnavailable.


## Organização compilada

O registro em `remote/mod.rs` seleciona a factory Google a partir da conta validada. `google/mod.rs` implementa `RemoteObserver` em `GoogleRemoteDetector`; a fachada comum oferece o ciclo de vida e o scheduler. `api.rs` usa apenas Providers Google e sessões obtidas por Connection/Accounts. `metadata.rs` e `tests.rs` mantêm os detalhes específicos do Google dentro deste diretório. Os métodos públicos de ciclo de vida são os da fachada `RemoteDetector`, compartilhados por todos os especialistas; a implementação concreta não é exposta ao consumidor externo.
