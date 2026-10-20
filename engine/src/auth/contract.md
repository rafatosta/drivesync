# Contrato — auth

## Propósito
Autenticação OAuth 2.0 com PKCE e refresh.

## Consumidores
accounts,remote.

## Entrada
Objetos conceituais validados, identificadores estáveis, contexto de cancelamento e, quando pertinente, versões esperadas.

## Saída
Resultado tipado ou erro explícito e classificável: temporário, definitivo, conflito ou resultado ambíguo.

## Responsabilidade e limites
Autenticação OAuth 2.0 com PKCE e refresh. Não assumir responsabilidades de outros módulos nem depender da implementação gráfica.

## Invariante
Não registrar tokens; falha de rede não significa revogação.

## Qualidade de contrato
Toda operação deverá especificar efeitos colaterais, idempotência, concorrência permitida, pré/pós-condições e recuperação após interrupções. Nenhuma falha pode ser interpretada como sucesso.

## Credenciais por conta

Toda aquisição/renovação usa `AccountId`, sem token global. A porta de credenciais isola segredo por identidade de conta e retorna apenas capacidade autenticada escopada, sem expor refresh token a todos os módulos. Renovação simultânea de uma conta pode ser coordenada por essa conta, mas não impede renovação da outra.

Teste obrigatório: token revogado ou refresh falho na conta A não invalida B; nenhuma credencial cruza contas nem aparece em log.
