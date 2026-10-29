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

## COORD-004 — gate de disponibilidade implementado

Entradas: associação persistida, cliente OAuth e ID esperado de marcador no
registro privado externo. Sem vínculo externo, a associação fica não verificada;
somente configuração explícita pode estabelecer vínculo, com marcador compatível.

1. Abrir e validar registro externo; adquirir lock não bloqueante da associação.
2. Abrir e bloquear raiz física por descritor sem seguir symlinks. Comparar caminho,
   identidade atual, acesso efetivo e marcador com vínculo esperado.
3. Verificar raiz remota pelo ID, tipo, lixeira e capacidade de listar. Falha não
   produz inventário vazio. HTTP 404 permanece inconclusivo.
4. Preparar observações/plano completos. Revalidar antes de persistir e executar.
   Manter descritor/bloqueios através do download e upload opcional.
5. Antes de efeitos, revalidar associação/raiz e versões segundo contratos dos
   executores; upload exige capacidade de criar no pai efetivo. Durante streaming,
   perda local observada interrompe escrita antes do próximo bloco.
6. Revalidar ao encerrar. Persistir motivo tipado se a raiz falhou; limpar suspensão
   somente após ciclo concluído e revalidação. Liberar locks após gravar resultado.
7. Retomada começa novamente na verificação, sem reaplicar plano antigo. Fases
   interrompidas são reconciliadas pelos journals existentes antes de confirmação.

O registro externo não confirma transferência e não avança B. Falha de gravação
impede sucesso. Não existe transação distribuída com Google ou exclusão mútua com
processos externos; os limites de corrida e a bateria real constam no contrato e
nos [casos de teste](tests/test-cases.md).
