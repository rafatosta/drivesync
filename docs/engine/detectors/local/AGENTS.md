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
