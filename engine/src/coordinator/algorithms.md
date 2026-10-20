# Algoritmos coordinator

**Estado:** especificação proposta v0.2, independente de linguagem.

## IDs
COORD-001 ciclo, RECOVERY-002 recuperação.

## Pré-condições
Entradas validadas, estado conhecido, pré-condições registradas e recursos disponíveis.

## Algoritmo
1. Obter lock exclusivo por raiz.
2. validar estado.
3. recuperar in-flight.
4. receber alterações.
5. pedir plano puro.
6. persistir.
7. executar com versões esperadas.
8. confirmar efeitos e base.

## Pós-condições
Resultado confirmado e durável ou erro tipado com etapas parciais recuperáveis.

## Falhas
Rede, autorização, disco, versão concorrente, estado parcial, cancelamento e interrupção entre efeito e commit.

## Verificar
Identidade, versões, integridade e invariantes antes de ações destrutivas, e efeitos confirmados depois.

## Não inferir
Timeout não prova ausência; nome não prova identidade; erro parcial não confirma sucesso; estado UNKNOWN bloqueia exclusão.

## Invariante
Falha de estado bloqueia operações destrutivas.

## Complexidade
Metadados por item aproximadamente O(1); n operações O(n); B bytes transferidos O(B), excluídas latências externas.


## Fundamentação técnica por algoritmo

Separar **técnica consolidada**, **política própria**, **evidência de teste** e **hipótese não demonstrada**. Consulte [método](../../../docs/development-methodology.md), [fontes](../../../docs/reference-catalog.md) e [template](../../../docs/algorithm-spec-template.md).

### COORD-001

- **Técnica conhecida:** Orquestração por contratos com máquina de estados.
- **Referências primárias:** [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html), [Google Drive changes](https://developers.google.com/workspace/drive/api/guides/manage-changes).
- **Política do DriveSync:** Verificar credenciais, banco, roots e dependências, sem duplicar regras de outros módulos; somente um coordenador ativo por raiz.
- **Testes/oráculos a detalhar:** Motor iniciado duas vezes, token revogado no meio do ciclo, DB DEGRADED.
- **Limitações / questões abertas:** Distinguir autorização da sincronização real versus fake utilizado em teste.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.

### RECOVERY-002

- **Técnica conhecida:** Reconciliação de operações parciais em dois sistemas não transacionais.
- **Referências primárias:** [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html), [Google Drive upload guide](https://developers.google.com/workspace/drive/api/guides/manage-uploads).
- **Política do DriveSync:** Verificar efeitos externos antes de retry; não assumir transação distribuída entre SQLite, filesystem e Drive.
- **Testes/oráculos a detalhar:** Crash após upload antes do commit SQLite; arquivo local renomeado sem gravar base; restarts repetidos.
- **Limitações / questões abertas:** Especificar tabela de recuperação por operação e ponto de falha antes de ativar exclusões.
- **Situação:** DRAFT; revisar e concluir os oráculos antes de gerar Rust.
