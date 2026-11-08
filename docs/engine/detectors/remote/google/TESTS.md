# Detector Remoto Google — TESTS

## 1. Testabilidade independente

Implementar TDD do **especialista Google real** com Google API fake/servidor HTTP simulado, sink individual fake, checkpoints isolados e scheduler/relógio controláveis. Não usar conta Google real, Storage real, Local, Orquestrador nem o Detector pai real.

## 2. Casos por método da interface remota

| Caso | Método | Sucesso | Negativos |
| --- | --- | --- | --- |
| GRD-01 | `new` | Cria instância Google Stopped | Conta/raiz inválida, dependência ausente |
| GRD-02 | `start` | Agenda leitura inicial ou incremental pelo Google | Start repetido, checkpoint inválido |
| GRD-03 | `poll_once` | Processa páginas e emite eventos **individuais** ao sink | 403, 404, timeout, campos ausentes, paginação parcial, sink falhando |
| GRD-04 | `stop` | Interrompe polling e preserva checkpoint | Interrupção no meio de página, stop repetido |
| GRD-05 | `status` | Reflete saúde/última leitura Google | Estado nunca iniciado, erro sanitizado |

## 3. Casos específicos do Google

- **GRD-G-01:** Drive Changes paginado; cursor só confirmado após aceite durável de todos os eventos relevantes.
- **GRD-G-02:** `removed=true` indica reavaliação, não exclusão automática.
- **GRD-G-03:** arquivo movido/renomeado mantém identidade Google estável quando a API comprova.
- **GRD-G-04:** mudanças durante inventário inicial não são perdidas entre páginas/cursor.
- **GRD-G-05:** expiração do cursor aciona recuperação segura sem descartar notificações já duráveis.
- **GRD-G-06:** resposta de arquivo nativo Google sem tamanho/hash não gera metadados inventados.
- **GRD-G-07:** evento aceito no sink e falha antes do checkpoint gera replay seguro.
- **GRD-G-08:** múltiplas contas Google não misturam IDs, cursores ou sessões.
- **GRD-G-09:** intervalos de polling independentes do Detector Local.
- **GRD-G-10:** todos os caminhos emitem **um evento por chamada**, sem acessar fila Storage.

Cada caso testa estados, chamadas, argumentos, erros e resultados; nenhum teste do especialista pode simplesmente simular o próprio `GoogleRemoteDetector`.

## 4. Aceite

Cobrir os cinco métodos do contrato pai e os casos Google aplicáveis. Executar Rust exclusivamente no Flatpak. Relatar PASS/FAIL/BLOCKED e evidências reais; testes simulados não comprovam integração com Google real. **Nenhum teste foi executado pela criação deste documento.**
