# Configuration — orientações para agentes

## Escopo

Documentar e implementar somente as preferências aprovadas da primeira versão e o mecanismo comum de **backup/restauração SQLite sob registro explícito dos Storages**. Nenhum cadastro de contas, agendamento de concorrência ou regra de sincronização pertence a Configuration.

## Regras

1. Ler [README.md](README.md), [INTERFACES.md](INTERFACES.md) e [TESTS.md](TESTS.md) antes de implementar.
2. O consumidor mantém os defaults; Configuration só valida e fornece substituições opcionais. Ausência de preferência não falha a engine.
3. Configurações da primeira versão: intervalo remoto, limite de páginas remotas, debounce local, tamanho de lote de eventos, timeout de rede e intervalo de retry. Não configurar concorrência de uploads/downloads/análises, workers, chunks ou política de exclusões ainda não decididos.
4. O escopo por sincronização usa `SyncId`, não caminhos; por conta fica adiado.
5. Não modificar parâmetros capturados durante operações em andamento sem suporte explícito do consumidor.
6. Receber localização física de bancos somente por registro autorizado de cada Storage; nunca descobrir bancos por varredura do sistema.
7. Centralizar o mecanismo físico de backup consistente e restauração SQLite. Preferir Online Backup API; não copiar arquivos SQLite ativos ignorando WAL/journal.
8. Restaurar somente com exclusividade real, preservação do original e validação posterior pelo Storage proprietário.
9. Não criar ou modificar tabelas do Inventário ou Detector, não administrar eventos/leases/baselines e não guardar segredos de Accounts.
10. Distinguir corrupção comprovada de indisponibilidade, disco cheio, lock, permissão e identidade/schema incompatível. Nunca reconstruir banco automaticamente como efeito de `restore_database`.
11. Reconstrução limpa, se autorizada posteriormente, utiliza inicialização do schema pelo Storage; perdas de histórico exigem reobservação decidida pela coordenação.
12. Não assumir número de instâncias, contas, threads ou workers. A independência modular deve sobreviver a essa decisão posterior.
13. Testar preferências e recuperação com SQLite real isolado, WAL, falhas e concorrência. Nenhum código Rust testado sem evidência deve ser declarado pronto.

## Critério de aceite

Contratos de preferência e de recuperação separados, ausência de acesso SQL entre módulos, backups consistentes, substituição segura, identidade validada e testes reais executados no ambiente Flatpak quando houver implementação.

**Estado:** documentação contratual proposta.
