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
