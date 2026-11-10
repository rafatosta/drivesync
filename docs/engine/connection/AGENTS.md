# Orientações — conexão pública

Ler README, INTERFACES, TESTS e AGENTS.md da raiz antes de editar. Este componente é privado e seus contratos são reexportados por lib.rs. Não criar nova crate, facade universal ou dependência de GTK dentro da engine.

Não modificar Providers; comunicar qualquer limitação impeditiva e aguardar orientação. Usar suas operações existentes para comunicação Google. Manter credenciais sob Accounts. A coordenação de navegador/callback pertence a Connection; não atribuir esse fluxo de aplicação ao mapeamento HTTP de Providers nem ao Orquestrador.

Trabalhar com testes primeiro, regressão por unidade e commits locais em inglês. Rust somente no SDK Flatpak. Não executar consentimento real nem usar keyring pessoal para validar. Não criar uma implementação permissiva da guarda de remoção de Accounts.

Remover componentes antigos de sincronização da interface junto de seus testes específicos; preservar o código dos módulos da engine e as preferências independentes do aplicativo. Não enfraquecer testes existentes de Providers/Accounts para reativar GTK.
