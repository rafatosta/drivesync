# Configuration — TESTS

## Objetivo

Plano TDD **não executado** para preferências da primeira versão e para o mecanismo compartilhado de recuperação SQLite.

## Preferências

| ID | Cenário | Resultado |
| --- | --- | --- |
| CFG-01 | Chave sem override | `get` devolve `None`; consumidor usa seu default |
| CFG-02 | Override global válido | Valor tipado correto, sem alterar regra do módulo |
| CFG-03 | Override por SyncId válido | Somente aquela sincronização usa a substituição |
| CFG-04 | Override por sync ausente | Recupera override global permitido, se houver |
| CFG-05 | Valor inválido, tipo trocado, escopo proibido | Erro, sem gravar mudança parcial |
| CFG-06 | `clear` de preferência | Volta a não haver substituição no escopo |
| CFG-07 | Configuration ausente/indisponível | Módulos independentes continuam com defaults seguros |
| CFG-08 | Mais de uma conta/sync | Nenhuma leitura cruza identidades; sem limite de concorrência inventado |
| CFG-09 | Mudança durante operação ativa | Não altera parâmetros já capturados pela operação sem suporte expresso |

## Backup e recuperação SQLite

| ID | Cenário | Resultado |
| --- | --- | --- |
| CFG-DB-01 | Storage registra banco e identidade | Local restrito e identidade vinculados, sem ler dados de domínio |
| CFG-DB-02 | Banco não registrado | Backup/restauração recusados; não localizar arquivos por varredura |
| CFG-DB-03 | Backup de SQLite ativo em WAL | Snapshot consistente via Online Backup API / mecanismo validado; sem cópia ingênua de arquivo ativo |
| CFG-DB-04 | Falha durante criação de backup | Último snapshot válido preservado; arquivo parcial não fica elegível |
| CFG-DB-05 | Corrupção comprovada e snapshot válido | Storage torna banco exclusivo, backup é validado e restaurado com mesma identidade |
| CFG-DB-06 | Conexão/transação ainda ativa na restauração | Não substituir banco; erro seguro |
| CFG-DB-07 | Backup de conta, sync ou classe diferente | Rejeitar restauração, preservar banco atual |
| CFG-DB-08 | Snapshot ausente, danificado ou esquema incompatível | Falha explícita, sem aceitar backup inválido |
| CFG-DB-09 | SQLite BUSY, permissão negada ou disco cheio | Não classificar como corrupção nem restaurar |
| CFG-DB-10 | Falha após preservar banco antigo, antes da troca | Banco e WAL/journal preservados; recuperação segura |
| CFG-DB-11 | Troca física bem-sucedida, validação de domínio falha | Não declarar `Restored`; banco antigo preservado |
| CFG-DB-12 | RecoveryFailed, pedido de reconstrução vazia | Configuration não cria tabelas de domínio; decisão e inicialização são externas |
| CFG-DB-13 | Backup do Inventário e do Detector | Mesmo mecanismo SQLite, isolamento de identidades e resultados próprios |
| CFG-DB-14 | Credenciais do keyring | Nunca copiadas para backup SQLite |
| CFG-DB-15 | Banco restaurado defasado da realidade Drive/filesystem | Retorno não executa sync, não confirma ausências nem elimina histórico externo |

Os testes físicos devem utilizar bancos SQLite reais em diretórios temporários, WAL habilitado, concorrência controlada, reinício e falhas injetadas. Verificar a integridade com consulta independente e a identidade/schema pela interface proprietária. Mocks sozinhos não comprovam a consistência do backup. Executar testes Rust somente no Flatpak previsto pelo repositório; não declarar aprovação sem execução.
