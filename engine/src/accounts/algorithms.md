# Algoritmos — accounts

**Status:** proposta v0.2, independente da linguagem.

## Identificadores
ACCT-001 vincular; ACCT-002 desvincular.

## Objetivo
Identidade de contas e vínculos com pastas.

## Pré-condições
Configuração validada, dependências disponíveis ou erro explícito, estado prévio recuperável.

## Algoritmo conceitual
1. Identificar conta.
2. validar que raiz local não se sobrepõe a outra.
3. associar remote folder ID.
4. preservar todos os dados ao desvincular.

## Pós-condições
Estado confirmado e persistido, ou erro tipado com efeitos parciais identificáveis.

## Possíveis erros
Entrada inválida; perda de conectividade; autorização negada; falta de espaço; versão alterada; erro de I/O; estado desconhecido; falha entre efeito externo e commit.

## O que verificar
IDs e revisões; pré-condições imediatamente antes de ações destrutivas; retorno e efeitos persistidos; invariantes globais e limites desta caixa-preta.

## O que não verificar nem inferir
Não inferir ausência por timeout/erro HTTP, nem identidade apenas pelo nome, nem sucesso pela ausência de exceções. Não inspecionar conteúdo do usuário sem necessidade da operação.

## Invariantes
Desvincular nunca apaga conteúdo local ou remoto. Aplicam-se também os invariantes globais de `docs/invariants.md`.

## Complexidade
Consultas pontuais tipicamente O(1) em metadados; inventário e planejamento O(n); transferências de B bytes O(B). Latência de rede, disco e consistência entre operações dominam o custo real.

## Critérios de aceitação
Caso nominal, cancelamento, repetição sem duplicação indevida, erro transitório, erro definitivo, falha parcial e reinicialização. Revisar as decisões pendentes antes de codificar.


## Fundamentação técnica por algoritmo

Esta seção distingue **solução consolidada**, **decisão do DriveSync**, **teste verificável** e **hipótese ainda aberta**. Consulte [método de desenvolvimento](../../../docs/development-methodology.md) e [catálogo de fontes](../../../docs/reference-catalog.md). Os links são referências de projeto, não provas de que a implementação esteja pronta.

### ACCT-001

- **Técnica conhecida:** Vinculação por identidade estável de conta.
- **Referências primárias:** [Google OpenID Connect](https://developers.google.com/identity/openid-connect/openid-connect), [OAuth Desktop](https://developers.google.com/identity/protocols/oauth2/native-app).
- **Política do DriveSync:** Identificar uma conta com identificador estável disponibilizado pelo provedor; validar raízes e conflitos de associação; não usar somente nome exibido.
- **Testes/oráculos a detalhar:** Mesma conta duas vezes, mudança de e-mail, duas contas distintas, raízes sobrepostas.
- **Limitações / questões abertas:** Confirmar configuração de identidade/OIDC e scopes mínimos.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.

### ACCT-002

- **Técnica conhecida:** Desvinculação controlada de configuração.
- **Referências primárias:** [Google OAuth Desktop](https://developers.google.com/identity/protocols/oauth2/native-app).
- **Política do DriveSync:** Remover vínculo sem apagar local/remoto; manter processo seguro em caso de operações pendentes.
- **Testes/oráculos a detalhar:** Desvincular durante upload, durante conflito e offline.
- **Limitações / questões abertas:** Política de arquivos remanescentes após desvincular é decisão explícita do DriveSync.
- **Situação:** DRAFT; implementar somente após completar os oráculos e revisão humana.

## ACCT-003 — Persistir e restaurar perfis na abertura

Implementado para perfis de login; pastas e sincronização ficam fora deste algoritmo.

1. Ler JSON versão 2 (aceitando versão 1) em `$XDG_DATA_HOME/drivesync/accounts.json`, com fallback para
   `$HOME/.local/share/drivesync/accounts.json`; ignorar XDG relativo. No Flatpak,
   usar o diretório XDG privado da aplicação. Selecionar o namespace do cliente OAuth.
2. Validar versão, identidade estável, duplicatas e tamanho. Corrupção ou versão
   desconhecida gera erro e preserva o arquivo, sem reinicialização silenciosa.
3. Após auth salvar credenciais no cofre, atualizar o perfil sob mutex e persistir
   em temporário privado no mesmo diretório: fsync, rename e fsync do diretório.
   Confirmar sucesso só após gravação. Serializar escritas no motor único;
   concorrência entre processos externos não é suportada.
4. Na abertura, UI-001 chama em worker uma vez por processo. Verificar cada ID por
   AUTH-002 (cofre, renovação e UserInfo); atualizar sucessos e manter falhas por
   conta sem bloquear as demais. Perfil salvo nunca comprova autenticação.
5. Uma conta verificada abre Account Information; várias ficam na lista. Oferecer
   retry/reconexão nas falhas, sem abrir navegador automaticamente. UI-003 apenas
   reapresenta a janela, sem repetir a restauração.

O índice contém ID interno, subject e e-mail opcional, nunca tokens ou estado de
login; e-mail não é chave. Clientes OAuth ficam isolados. Credenciais anteriores
sem índice, ou salvas antes de uma falha na gravação do perfil, exigem reconexão
para recuperar o cadastro; não enumerar o cofre. Versão 1 recebe backup antes da primeira escrita em versão 2; versões futuras
exigem migração explícita.

Testes automatizados: [tests.rs](tests.rs). Aceitação com conta/cofre reais:
[registro de validação](../../../docs/auth-validation.md).
