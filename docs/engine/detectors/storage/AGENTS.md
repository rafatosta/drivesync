# Orientações para agentes — Detector Storage

## Objetivo

Implementar armazenamento durável de eventos detectados, com reservas, confirmação e recuperação.

## Regras de implementação

- Seguir os contratos de [INTERFACES.md](INTERFACES.md).
- Persistir e devolver cada objeto `File` integralmente, preservando identidade e metadados.
- Garantir integridade, idempotência e ordenação das ocorrências.
- Implementar reservas temporárias, confirmação explícita e recuperação após interrupção.
- Tratar falhas de armazenamento sem eliminar ou recriar dados silenciosamente.
- Encapsular detalhes de banco, transações, esquema e localização dos arquivos.
- Registrar localização/identidade na infraestrutura SQLite de Configuration para backup/restauração física, sem expor conteúdo da fila.
- Fechar/isolar conexões antes de restaurar, preservar originais e revalidar identidade e schema após a troca.
- Não reconstruir automaticamente banco vazio na falha de restauração; isso exige decisão separada da coordenação.
- Proteger informações sensíveis presentes nos registros.

## Validação

Executar [TESTS.md](TESTS.md) usando armazenamento real temporário, com reinício, concorrência, falhas e recuperação. Seguir as instruções gerais do [AGENTS.md da raiz](../../../../AGENTS.md).
