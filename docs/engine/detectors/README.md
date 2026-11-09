# Detector

## Objetivo

Detectar alterações em arquivos locais e remotos e disponibilizar essas ocorrências por uma interface única, com entrega durável e confirmação explícita.

## Funcionamento

O módulo reúne observações locais e remotas como eventos individuais, preserva os dados de cada ocorrência e os organiza em uma fila persistente. Eventos podem ser reservados em lotes, confirmados após processamento ou liberados para nova entrega. A retomada mantém a identidade e a ordem de enfileiramento dos registros.

## Interface pública

- `new`: cria o detector.
- `start`: inicia a observação.
- `stop`: interrompe a observação.
- `request_scan`: solicita uma nova observação.
- `status`: informa o estado da detecção.
- `take_events`: reserva um lote de eventos disponíveis.
- `acknowledge`: confirma o processamento de um lote.
- `release_events`: devolve eventos reservados à fila.

Cada ocorrência é representada por `File`, incluindo identidade, contexto da alteração e dados observados disponíveis. `FileBatch` representa o lote entregue. Uma observação não equivale a uma alteração já aplicada.

## Componentes

- [Local](local/README.md): observação do sistema de arquivos.
- [Remoto](remote/README.md): observação de alterações em serviços remotos.
- [Storage](storage/README.md): persistência e entrega dos eventos detectados.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)

**Estado:** especificação documental; não comprova implementação Rust.

## Persistência recuperável

O banco privativo dos Detectores usa política de [Configurations](../configuration/README.md) por meio do [Detector Storage](storage/INTERFACES.md): corrupção comprovada gera aviso, restauração automática e, se esta falhar, novo aviso e reconstrução não destrutiva. A fila e checkpoints eventualmente perdidos não são considerados processados; a observação local/remota deve recuperar cobertura antes da liberação de consumo pelo Orquestrador. Os algoritmos e a API pública de eventos permanecem os mesmos.
