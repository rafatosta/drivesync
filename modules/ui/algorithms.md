# ui — algoritmos

**Status:** rascunho v0.2.

## Identificadores
UI-001 iniciar, UI-002 mostrar status.

## Objetivo
Implementar a responsabilidade do módulo sem acoplar a interface ao motor.

## Pré-condições
Dados de entrada validados, dependências acessíveis ou erro tipado.

## Algoritmo
1. Abrir janela DriveSync GTK4/Libadwaita.
2. consultar snapshot.
3. exibir conta/pasta e progresso.
4. enviar comandos ao motor.
5. mostrar erros/conflitos.

## Pós-condições
Resposta ou erro explícito sem perda silenciosa de estado.

## Erros
Configuração inválida, falha temporária de comunicação, interrupção, cancelamento e incompatibilidade de versão.

## Verificações
Identidade e versão de contexto, efeitos confirmados e invariantes globais.

## Proibido
Interface não implementa algoritmos do motor. Timeout não significa sucesso ou ausência remota.

## Complexidade
Por comando O(1) sem contar eventuais itens de lista; para n registros O(n).
