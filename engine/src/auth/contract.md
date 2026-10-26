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

## Escopo desta implementação (AUTH-001 / AUTH-002)

Entrada: configuração local de cliente Desktop Google, cancelamento cooperativo e
porta para abrir a URL no navegador. Saída: identidade confirmada por UserInfo via
HTTPS (`sub`, e-mail verificado opcional), sem tokens na API pública.

1. Validar configuração; adquirir exclusão do login no motor; criar listener
   somente em 127.0.0.1 com porta efêmera, state aleatório e PKCE S256.
2. Solicitar `openid email drive`, acesso offline e consentimento explícito no
   cadastro da conta. O escopo completo cobre leitura e criação de arquivos; o
   upload continua desativado por padrão e só é usado após opt-in. Ver
   `docs/google-oauth.md`.
3. Abrir navegador via porta da UI (GIO/portal); aguardar até cinco minutos.
   Cancelamento, erro do navegador e timeout nunca criam uma conta.
4. Aceitar GET apenas no host/porta/path exatos, sem parâmetros duplicados,
   com state correspondente. Retornos forjados são rejeitados sem encerrar o fluxo.
5. Consumir código uma vez; trocar com o verifier; verificar escopos e UserInfo.
6. Sob mutex por identidade, persistir tokens no Secret Service, indexados por
   client ID + subject. Só então publicar o perfil sanitizado no motor/UI.

Renovação: carregar credencial da conta, conferir identidade, renovar sob mutex
por conta se expirar em até 60 segundos, conservar refresh anterior se omitido,
confirmar a identidade em UserInfo e salvar antes de retornar o perfil sanitizado. `invalid_grant` exige novo login;
rede/5xx não removem credenciais nem afetam outra conta. Nenhum retry automático
na troca de código (resultado pode ser ambíguo). Cancelar durante HTTP produz
resposta em até o timeout de 30 segundos; antes da persistência, verificar de novo.

Interrupção após autorização remota e antes do cofre exige repetir login; não há
sucesso local parcial. Credenciais persistidas sobrevivem ao processo; a descoberta
automática de perfis ao iniciar fica para a persistência de accounts/state (F07/F08).
AUTH-003 (revogação remota/desvinculação) permanece fora desta integração.
