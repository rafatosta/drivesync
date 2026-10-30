# Contrato — state

## Propósito
Persistência transacional e journal SQLite.

## Consumidores
coordinator,queue.

## Entrada
Objetos conceituais validados, identificadores estáveis, contexto de cancelamento e, quando pertinente, versões esperadas.

## Saída
Resultado tipado ou erro explícito e classificável: temporário, definitivo, conflito ou resultado ambíguo.

## Responsabilidade e limites
Persistência transacional e journal SQLite. Não assumir responsabilidades de outros módulos nem depender da implementação gráfica.

## Invariante
Banco corrompido não é banco vazio.

## Qualidade de contrato
Toda operação deverá especificar efeitos colaterais, idempotência, concorrência permitida, pré/pós-condições e recuperação após interrupções. Nenhuma falha pode ser interpretada como sucesso.

## Escopo de persistência multi-conta

Cada linha de raiz pertence a `AccountId`; cada arquivo mapeado, observação, versão-base, operação, conflito e cursor referencia o escopo proprietário. Invariantes: chaves estrangeiras e relações impedem operação da conta A apontar para root da conta B; identificador remoto nunca é chave única global sem conta/Drive.

Cursor de changes deve ser armazenado por **conta e escopo do feed de alterações** (não necessariamente por raiz). SQLite pode ser único, mas um erro de dados de uma conta não autoriza exclusões em outra; corrupção do banco compartilhado suspende ações arriscadas no motor todo. Testar cross-account references, migração, crash e replay por conta.

## Reconstrução explícita

[STATE-003](algorithms.md#state-003--reconstrução-explícita-sync-005) define seleção
atômica de substitutos, preservação de bancos/sidecars, reuso de observações
íntegras, isolamento, cancelamento e bloqueio por perda de intents de upload.
A fachada retém locks até o fim. Recuperar não significa instalar nem reconstruir B.


## Preservação de conflitos — SYNC-010

O formato principal exato é v3. A tabela `conflict_copies` mantém intenção,
staging verificado e publicação de versões remotas divergentes, isolados por
cliente/conta/raiz remota/local e vinculados ao caminho original. Não participa
de `confirmed_bases`, nem invalida ou avança B. A leitura valida nome derivado,
chave, fase e metadados; o executor revalida identidade e conteúdo no filesystem.
Formato anterior é rejeitado sem migração, reset ou remoção de dados.
Contrato e recuperação: [preservation.md](../conflicts/preservation.md).

## Histórico de conflitos — SYNC-011

`conflict-history.sqlite3` é um journal separado, com formato exato 1, aplicação
e chave compostas próprias por cliente/conta/raiz remota/raiz local. Registros
mantêm o último diagnóstico por identidade e caminho, incluindo se ainda exige
atenção; eventos append-only registram diagnósticos distintos, cópias verificadas
e falhas classificadas sem texto de provedor. Chaves determinísticas tornam a
repetição idempotente; consulta após reabertura não acessa a rede e não concede
autorização de escrita.

Atualizações de registro e seus eventos são transacionais dentro desse banco.
Ele não participa do journal de instalação, da base confirmada nem da
reconciliação; ausência, corrupção ou formato incompatível falham sem reset ou
migração. A reconstrução explícita preserva os originais e cria um journal vazio,
mantendo operações de sincronização bloqueadas até a recuperação autorizada.
Decisões assistidas e encerramento de pendências ainda não são executados pelo
motor e, portanto, não são registradas como se tivessem ocorrido.
