# Detector Storage

## Objetivo

Armazenar de forma durável as ocorrências detectadas e disponibilizá-las para entrega, confirmação e recuperação.

## Funcionamento

Persiste cada evento `File` integralmente, preservando identidade, dados observados e ordem de enfileiramento. A entrega utiliza reservas temporárias (leases): eventos reservados podem ser confirmados ou devolvidos à fila; reservas não concluídas podem ser recuperadas após interrupções.

O armazenamento deve preservar a integridade dos registros, tratar falhas explicitamente e evitar perda silenciosa de eventos. A tecnologia e a localização física são detalhes de implementação do próprio componente.

## Interface

- `open`: abre o armazenamento.
- `enqueue`: registra uma ocorrência.
- `claim_batch`: reserva eventos disponíveis.
- `acknowledge`: confirma eventos reservados.
- `release_batch`: libera reservas.
- `status`: informa o estado do armazenamento.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Agentes](AGENTS.md)

**Estado:** especificação documental; não comprova implementação Rust.

## Proteção e recuperação

O Storage preserva a propriedade de sua fila persistente. Se detectar corrupção comprovada, informa erro ao usuário e solicita política de backup/restauração automática a [Configurations](../../configuration/README.md). Se a restauração falhar, informa novamente e reconstrói um banco vazio preservando o anterior. A fila vazia recém-criada **não comprova ausência de eventos externos**; o Detector precisa recuperar a cobertura e os checkpoints de observação antes de liberar seu consumo. Falhas transitórias, identidade divergente e esquema incompatível não autorizam reconstrução.
