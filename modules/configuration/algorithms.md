# configuration — algoritmos

**Status:** rascunho v0.2.

## Identificadores
CONFIG-001 validar, CONFIG-002 migrar.

## Objetivo
Implementar a responsabilidade do módulo sem acoplar a interface ao motor.

## Pré-condições
Dados de entrada validados, dependências acessíveis ou erro tipado.

## Algoritmo
1. Validar pasta, conta e ID remoto.
2. recusar raízes sobrepostas.
3. validar seleção e política de exclusão.
4. persistir atomicamente.
5. migrar versão com backup.

## Pós-condições
Resposta ou erro explícito sem perda silenciosa de estado.

## Erros
Configuração inválida, falha temporária de comunicação, interrupção, cancelamento e incompatibilidade de versão.

## Verificações
Identidade e versão de contexto, efeitos confirmados e invariantes globais.

## Proibido
Mudar configuração não deve produzir deleção automática. Timeout não significa sucesso ou ausência remota.

## Complexidade
Por comando O(1) sem contar eventuais itens de lista; para n registros O(n).
