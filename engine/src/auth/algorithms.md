# Algoritmos — auth

**Status:** proposta v0.2, independente da linguagem.

## Identificadores
AUTH-001 login; AUTH-002 refresh; AUTH-003 revogação.

## Objetivo
Autenticação OAuth 2.0 com PKCE e refresh.

## Pré-condições
Configuração validada, dependências disponíveis ou erro explícito, estado prévio recuperável.

## Algoritmo conceitual
1. Gerar state e PKCE.
2. abrir navegador e validar callback.
3. trocar código por tokens.
4. armazenar refresh em cofre.
5. renovar com exclusão mútua.

## Pós-condições
Estado confirmado e persistido, ou erro tipado com efeitos parciais identificáveis.

## Possíveis erros
Entrada inválida; perda de conectividade; autorização negada; falta de espaço; versão alterada; erro de I/O; estado desconhecido; falha entre efeito externo e commit.

## O que verificar
IDs e revisões; pré-condições imediatamente antes de ações destrutivas; retorno e efeitos persistidos; invariantes globais e limites desta caixa-preta.

## O que não verificar nem inferir
Não inferir ausência por timeout/erro HTTP, nem identidade apenas pelo nome, nem sucesso pela ausência de exceções. Não inspecionar conteúdo do usuário sem necessidade da operação.

## Invariantes
Não registrar tokens; falha de rede não significa revogação. Aplicam-se também os invariantes globais de `docs/invariants.md`.

## Complexidade
Consultas pontuais tipicamente O(1) em metadados; inventário e planejamento O(n); transferências de B bytes O(B). Latência de rede, disco e consistência entre operações dominam o custo real.

## Critérios de aceitação
Caso nominal, cancelamento, repetição sem duplicação indevida, erro transitório, erro definitivo, falha parcial e reinicialização. Revisar as decisões pendentes antes de codificar.


## Fundamentação técnica por algoritmo

Esta seção distingue **solução consolidada**, **decisão do DriveSync**, **teste verificável** e **hipótese ainda aberta**. Consulte [método de desenvolvimento](../../../docs/development-methodology.md) e [catálogo de fontes](../../../docs/reference-catalog.md). Os links são referências de projeto, não provas de que a implementação esteja pronta.

### AUTH-001

- **Técnica conhecida:** Authorization Code + PKCE em aplicativo nativo.
- **Referências primárias:** [Google OAuth Desktop](https://developers.google.com/identity/protocols/oauth2/native-app), [RFC 7636](https://www.rfc-editor.org/rfc/rfc7636), [RFC 8252](https://www.rfc-editor.org/rfc/rfc8252).
- **Política do DriveSync:** Abrir navegador padrão, conferir state, PKCE, callback/redirect e consentimento; nunca solicitar senha Google na interface DriveSync.
- **Testes/oráculos a detalhar:** Callback forjado, state diferente, code reutilizado, cancelamento e falta de browser.
- **Limitações / questões abertas:** Escolha de scopes e comportamento dentro do Flatpak precisam de prova real.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.

### AUTH-002

- **Técnica conhecida:** Renovação de token OAuth 2.0.
- **Referências primárias:** [Google OAuth Desktop](https://developers.google.com/identity/protocols/oauth2/native-app), [OAuth 2.0 RFC 6749](https://www.rfc-editor.org/rfc/rfc6749).
- **Política do DriveSync:** Refresh token fica em armazenamento seguro; renovar uma vez por conta mesmo diante de requisições concorrentes; falha temporária não revoga credenciais.
- **Testes/oráculos a detalhar:** Token expirado, refresh simultâneo, offline, invalid_grant e storage indisponível.
- **Limitações / questões abertas:** Política de armazenamento de segredo, reautorização e expiração do ambiente Testing devem ser validadas.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.

### AUTH-003

- **Técnica conhecida:** Revogação de autorização OAuth.
- **Referências primárias:** [Google OAuth Desktop](https://developers.google.com/identity/protocols/oauth2/native-app).
- **Política do DriveSync:** Desassociar credencial com opção explícita; nunca apagar conteúdo do usuário por revogação.
- **Testes/oráculos a detalhar:** Revogação remota bem-sucedida, rede indisponível, já revogado, conta ativa em outro perfil.
- **Limitações / questões abertas:** Precisar semântica de revogação local versus remota.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.
