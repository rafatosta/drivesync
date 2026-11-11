# Connection — testes de contrato

- CON-01: abrir sem OAuth permite listar/editar cadastro local; conectar falha com erro sanitizado e não acessa keyring.
- CON-02: begin prepara URL via GoogleDriveClient, escopos mínimos, state e desafio S256, listener somente loopback; não troca código nem grava conta.
- CON-03: callback válido executa exchange_authorization_code e Accounts real com vault isolado; conta é retornada somente após persistência.
- CON-04: estado incorreto, caminho/método inválido e parâmetros duplicados não executam exchange; callback válido posterior ainda pode concluir.
- CON-05: cancelamento, prazo e recusa no navegador limpam tentativas; repetição/finish concorrente não repete troca de código.
- CON-06: falhas de Provider/keyring não expõem tokens, não anunciam conta fictícia e preservam recuperação de Accounts.
- CON-07: reautorização preserva AccountId e rejeita outra identidade; list/update não fazem rede.
- CON-08: métodos podem ser aguardados fora de contexto Tokio; executor privado mantém GTK responsivo.
- UI-01: zero contas mostra boas-vindas, contas salvas mostram dashboard; conta nova abre editor com Salvar.
- UI-02: campos de pastas, sincronização, transferências, atividades e conflitos não existem na composição.
- UI-03: preferências alteram aparência e background, sem acessar engine; About permanece disponível.
- UI-04: erro ao salvar mantém nome digitado; cancelamento/fechamento não deixa retorno tardio alterar janela destruída.

Executar Rust somente no Flatpak do manifesto. Google real e keyring pessoal não são fixtures. Usar cliente Google real contra servidor HTTP local, Accounts real com vault sintético e armazenamento temporário. Exercitar layout em display/perfil isolados quando disponíveis. Registrar resultados reais ao concluir.

## Execução — 2026-11-10

Ambiente: `org.gnome.Sdk//50`, extensão Rust do manifesto Flatpak e dependências travadas em Cargo.lock. Não houve acesso a Google real, keyring pessoal ou configuração de autostart real.

| Verificação | Resultado |
| --- | --- |
| `cargo build --workspace --locked` | Aprovado, incluindo o aplicativo normal sem depender da unificação de features de teste |
| `cargo fmt --all --check` | Aprovado |
| `cargo test --workspace --locked` | 496 aprovados, 5 testes opt-in ignorados na execução padrão |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Aprovado |
| `cargo doc --workspace --no-deps --document-private-items --locked` | Aprovado |
| `cargo test --workspace --doc --locked` | Aprovado; nenhum doctest definido nesta versão |
| Testes gettext (`python3 -m unittest scripts.tests.test_i18n`) | 2 aprovados |
| `account_only_layout_and_initial_name_editor` em display, D-Bus e perfil XDG isolados | Aprovado separadamente; capturas de boas-vindas, conta nova e preferências inspecionadas |
| `isolated_accounts_vault_roundtrip` pelo runner de Secret Service privado | Aprovado separadamente |

Os outros três testes opt-in são os cenários preexistentes de desktop/background/autostart e não foram executados, pois exigem permissões e interação específicas com portais. Os dois casos isolados acima correspondem aos outros dois testes ignorados pela execução padrão.

### Rastreabilidade

- CON-01/08: `open_without_oauth_preserves_local_use` e `boundary_can_be_awaited_without_a_tokio_context`.
- CON-02/03: `valid_callback_persists_via_accounts_and_pkce_stays_internal` verifica escopos, desafio S256, formulário enviado pelo cliente real, retorno de conta persistida e edição local sem nova chamada remota.
- CON-04/05: `invalid_callback_does_not_exchange_and_concurrent_finish_is_rejected` e `cancellation_expiration_and_provider_denial_preserve_empty_accounts` verificam estado incorreto, parâmetro duplicado, consumo concorrente, cancelamento, a transição de expiração usada em produção e recusa. Outros limites de método/caminho são impostos pelo parser, sem alegar teste individual de cada combinação.
- CON-06/07: `reauthorization_checks_identity_and_failed_exchange_is_not_retried` verifica identidade, preservação de AccountId, mensagem sanitizada e única tentativa de troca. As falhas de keyring/persistência continuam cobertas pela suíte real de Accounts; a composição Connection usa vault sintético nos testes HTTP e não simula a própria Accounts.
- UI-01/02/03: teste visual verifica boas-vindas, editor de conta nova, editor fechado em conta existente, abertura por Editar, ausência dos componentes removidos, preferências de aparência/background e registro de About. O controle de background é alterado somente no arquivo de preferências isolado.
- UI-04: uso de referências fracas, preservação do rascunho em erro e cancelamento explícito estão implementados. A edição bem-sucedida é testada pela API e o layout pelo teste visual; não houve teste E2E completo de falha de salvamento, navegador real ou fechamento em todas as fases.

### Revisão e limites

Providers permaneceu sem alterações. Nenhum código em `src/` importa `drivesync_engine::accounts`, `providers` ou `models`: o aplicativo usa os tipos e métodos públicos da raiz. Não há nova crate de facade nem autenticação encaminhada ao Orquestrador.

A suíte global voltou a passar após remover o teste obsoleto do diagnóstico local junto do recorte de observação/sincronização. A implementação interna e os testes de Accounts/Providers foram preservados. Não foi medida porcentagem de cobertura, nem validado um aplicativo Flatpak instalado ou um consentimento Google real. As capturas têm dados fictícios e demonstram apresentação, não uma conta autenticada real.
