# Execução de novas réplicas

O fluxo é `Orchestrator → Analyzer → Operator`. `AnalyzerDependencies.execution`
é opcional: `None` preserva inspeção sem efeitos; a aplicação compõe Operator,
portas de cancelamento/progresso e observador por conta.

O Analyzer aceita execução automática somente sem baseline e com uma origem
observada, contraparte ainda não vinculada, um plano completo de criação e nenhum
conflito, escolha manual ou dependência pendente. `verify_creation` do Operator
reobserva origem, permissões, pai e destino livre. Somente requisitos de observação
da contraparte e de identidade inicial podem ser satisfeitos por essa prova. Isso
não persiste uma ausência fictícia e não associa recursos preexistentes.

Inventory revalida revisão, guardas de namespace, comando e origem antes de reservar.
A transação cria o checkpoint e vincula a operação original por chave reservada
`execution:<id>`; enqueue genérico rejeita esse prefixo. Não há alteração de esquema.
Claims e requeues genéricos continuam proibidos para decisões do Analyzer.

O Analyzer chama upload, download ou criação de diretório. O Operator preserva
staging, sessão, offset e ID remoto entre tentativas. Diretórios locais são publicados
por rename sem substituição de um diretório privado com identidade registrada;
diretórios remotos usam ID reservado antes da criação. Homônimos nunca são mesclados.

Após efeito verificado, Inventory vincula somente a identidade produzida pelo
checkpoint. Uma observação independente dessa mesma identidade física pode ser
retirada do índice, conservando seu registro histórico; isso não é pareamento por
nome/hash. O Analyzer reobserva origem e réplica pelo Operator, compara conteúdo com
o hash medido da transferência e confirma baseline, conclusão da operação e atividade
na mesma transação. Uma análise posterior sem mudanças preserva a confirmação
existente; se uma nova observação a invalidou, reobserva os dois lados sem transferir
novamente antes de confirmar. Retorno da análise, checkpoint e ack de eventos não são `Synced`.

Rede, integridade de resposta e falhas transitórias usam no máximo cinco tentativas,
com backoff exponencial a partir de 60 segundos e ID original. Origem alterada,
colisão e escopo inválido bloqueiam. Reinício recupera tentativas pelo Inventory;
worker de observação revisita as vencidas, inclusive sem novo evento. Falhas
transitórias na prova inicial, antes de criar uma operação, são verificadas novamente
a cada 30 segundos; reinício refaz essa prova pelo inventário. Cancelamento
de sessão é cooperativo e não desfaz um efeito já enviado.

Testes combinam fachada Analyzer, Operator/Provider reais, HTTP loopback com estado,
SQLite e filesystem temporários: upload/download, pastas, interrupção, confirmação
sem baseline prematura e manutenção do ID. Google real e Flatpak instalado exigem
aceitação separada. Atualizações, renomes, movimentos, lixeira, restauração e formatos
nativos Google continuam fora da execução automática.

## Verificação de réplica recém-criada

A origem continua exigindo versão exata. A réplica remota recém-criada pode ter
um avanço numérico de versão entre a conclusão do upload e a confirmação, mas
somente com SHA-256 conhecido e idêntico, mantendo todas as demais verificações
de identidade, tamanho, nome, MIME, pais e capacidade de leitura. Versão reduzida,
hash ausente ou conteúdo/localização divergentes continuam bloqueados. A baseline
usa os metadados efetivamente reobservados, nunca a versão antiga do resultado.
