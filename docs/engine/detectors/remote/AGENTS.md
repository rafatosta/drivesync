# Agentes — Detector Remoto

## Escopo e fontes

Implementar **somente `detectors/remote/`**, independentemente do Detector pai, Local e Storage reais. Ler [INTERFACES.md](INTERFACES.md), [README.md](README.md), [TESTS.md](TESTS.md), [AGENTS raiz](../../../../AGENTS.md), [padrão](../../MODULE_STANDARD.md) e a documentação do especialista [Google](google/README.md), quando estiver implementando Google.

## Regras

- Remoto produz **eventos individuais** e os entrega por `RemoteEventSink` injetado; nunca constrói `File` público nem recebe API do Orquestrador.
- Não importar o Storage concreto nem assumir onde sua fila está armazenada. Sucesso do sink significa persistência durável confirmada.
- Estratégia própria por especialista: polling, consulta paginada e checkpoint quando disponíveis, sem impor comportamento ao Local.
- Checkpoint não pode avançar antes de aceitação durável dos eventos correspondentes; replay é necessário quando houver resultado incerto.
- Erro de rede, acesso, cursor ou leitura parcial não confirma exclusão. Não realizar sincronização física.
- O contrato Remoto é independente de provedor; cada especialista usa seu cliente em `providers/<provedor>/api/`, sem reimplementar HTTP ou OAuth fora do módulo proprietário.
- Remoto não administra `inventory.sqlite3` nem `detector.sqlite3`; qualquer backend de checkpoint será injetado por contrato tipado.
- A API é interna a `detectors/`; não criar reexportação externa de `RemoteDetector`.

## Testes e autonomia

TDD incremental (RED/GREEN/REFACTOR) com fake de porta remota/sessão/scheduler/sink/checkpoint; testar mecanismo real do Remoto, não mock do próprio objeto. “Implemente o Detector Remoto” abrange **cinco métodos** e cenários de `TESTS.md`, **não** implica implementação do pai/Local/Storage.

Executar Rust/Cargo somente no Flatpak do projeto; comentários/commits em inglês; commits locais coesos; sem push/PR sem autorização. Reportar casos executados, resultados, bloqueios e integrações ainda não validadas. **Somente documentação nesta etapa.**

## Regra de encapsulamento Models

O Remoto emite `File` completo por `RemoteEventSink.emit`, não `RemoteObservation` mínimo nem `DetectorEvent` duplicado. Metadados de provedor são adaptados de `GoogleFile` a `RemoteFile` exclusivamente no especialista e transmitidos como `FileData::Remote`. `FileEventKind` identifica sinais com evidência; `previous/current` guardam mudanças de localização. Perda de acesso, sem ID e falha de cursor geram reavaliação, não exclusão confirmada. Campos de RemoteFile podem evoluir sem alterar polling ou fila.
