# Orientações para agentes — Detector Local

## Objetivo

Implementar observação de alterações do sistema de arquivos com notificações e varreduras de verificação.

## Regras de implementação

- Respeitar as operações e os tipos definidos em [INTERFACES.md](INTERFACES.md).
- Emitir uma ocorrência individual por evento observado, preservando identidade, localização e metadados comprovados.
- Distinguir varreduras completas de observações parciais ou inacessíveis.
- Evitar interpretações destrutivas de falhas, indisponibilidade e ausência de dados.
- Encapsular a leitura de metadados locais e manter o formato de `File` previsto no contrato.
- Definir detalhes internos de implementação conforme as necessidades reais do código.

## Validação

Seguir [TESTS.md](TESTS.md), com diretórios temporários isolados, alterações de arquivos, interrupções e recuperação. Aplicar também as instruções gerais do [AGENTS.md da raiz](../../../../AGENTS.md).

## Estado e controle

- Expor atividade e contadores da observação atual por `status()`, sem I/O da consulta.
- Não implementar pausa de entrega no Local: o pai suspende apenas a entrega externa; o Local segue emitindo ao sink.
- `stop` interrompe observação, preservando emissões já aceitas, sem executar recuperação/reconciliação por conta própria.
- Testar LOC-S-01 a LOC-S-05, sem depender de Orquestrador real.


## Preparação e implementação posterior

- Ler INTERFACES → README → TESTS e o AGENTS da raiz antes de codificar. A etapa atual autoriza implementar e testar o coletor físico; não criar stubs da fachada, File/RemoteFile, varredura ou dependências do watcher.
- Na etapa de implementação, seguir RED → GREEN → REFACTOR por contrato e registrar os IDs dos cenários exercitados. Não substituir testes do filesystem por fakes do próprio detector.
- Preservar a separação entre verificação de cobertura do watcher e reconciliação de períodos interrompidos. Não iniciar Scanner ou acessar Inventário.
- Seguir o perfil local selecionado em Models e não implementar tipos compartilhados alternativos para contornar a dependência remota do envelope. O coletor concentra o conhecimento do esquema.
- Usar a branch arquivada apenas como referência: adaptar segurança por descritores e seus testes; não importar filtros que descartam nomes não UTF-8, hashing ou políticas de upload.
- Aplicar as regras da seção 8 de INTERFACES para estados, cancelamento, retry e memória limitada; não declarar entrega durável com sink fake nem prometer recuperação de pendência volátil após stop/crash.
