# ipc — algoritmos

**Status:** rascunho v0.2.

## Identificadores
IPC-001 comandos, IPC-002 eventos.

## Objetivo
Implementar a responsabilidade do módulo sem acoplar a interface ao motor.

## Pré-condições
Dados de entrada validados, dependências acessíveis ou erro tipado.

## Algoritmo
1. Negociar versão do protocolo.
2. validar comando/conta.
3. despachar ao coordinator.
4. devolver ID.
5. expor snapshot e eventos.
6. lidar com desconexão.

## Pós-condições
Resposta ou erro explícito sem perda silenciosa de estado.

## Erros
Configuração inválida, falha temporária de comunicação, interrupção, cancelamento e incompatibilidade de versão.

## Verificações
Identidade e versão de contexto, efeitos confirmados e invariantes globais.

## Proibido
Sem decisões de sincronização no IPC. Timeout não significa sucesso ou ausência remota.

## Complexidade
Por comando O(1) sem contar eventuais itens de lista; para n registros O(n).
