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

## ACCT-003 — Persistir e restaurar perfis na abertura (implementação autorizada)

Escopo: descoberta de perfis de login, sem pastas/sincronização. UI-001 chama uma
vez por processo, fora da thread GTK; UI-003 reapresenta a janela sem novo motor.

1. Ler índice JSON versão 1 em `$XDG_DATA_HOME/drivesync/accounts.json`, ou
   `$HOME/.local/share/drivesync/accounts.json`. XDG relativo é ignorado. No Flatpak,
   XDG_DATA_HOME já aponta ao diretório privado da aplicação.
2. Selecionar namespace do client ID Desktop em uso. Perfil salvo não comprova
   autenticação. Validar versão, identidade, duplicatas e limites; corrupção ou
   versão desconhecida gera erro e preserva o arquivo, nunca reinicializa.
3. Após auth salvar no cofre, atualizar perfil sob mutex e gravar arquivo temporário
   privado no mesmo diretório, fsync, rename e fsync do diretório. Confirmar o
   perfil só após persistência. Falha pode deixar credencial no cofre sem índice:
   reconectar a mesma conta repara o cadastro; não enumerar segredos do cofre.
4. Na abertura, verificar cada ID por AUTH-002 (cofre, renovação se necessário,
   UserInfo). Sucesso atualiza o perfil; falha mantém o perfil e retorna erro por
   conta. Uma conta indisponível não impede verificar as demais. Nunca abrir
   navegador automaticamente. Retry verifica novamente; reconectar exige clique.
5. UI mostra progresso, contas verificadas ou falhas explícitas. Uma única conta
   verificada abre Account Information; várias ficam disponíveis na lista.

Índice não armazena tokens nem estado autenticado; credenciais continuam no Secret
Service. Cliente alterado tem namespace separado. E-mail não é chave. Escritas
serializadas no motor único; uso concorrente por processos externos não suportado.
Nenhuma migração de cofre antigo é inferida: perfis anteriores sem índice exigem
uma reconexão. Versões futuras precisam de migração explícita.

Oráculos: reinício com duas contas; atualização sem duplicar; namespaces isolados;
falha de gravação sem sucesso; arquivo inválido/futuro preservado; offline, cofre
bloqueado, credencial ausente/revogada; uma conta falha e outra verifica; retry;
nenhum token em JSON; nenhuma chamada ao navegador ao restaurar.
