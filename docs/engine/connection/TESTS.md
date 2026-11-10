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
