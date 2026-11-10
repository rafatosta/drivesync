# Connection — API pública entre aplicativo e engine

## Necessidade

O aplicativo precisa conectar uma conta sem conhecer tokens, protocolos, armazenamento ou os módulos internos da engine. A crate `drivesync-engine` já é o package responsável por essa fronteira: não é necessário criar outra crate nem colocar essa responsabilidade no Orquestrador.

`engine/src/lib.rs` reexporta os contratos mínimos. Um módulo Rust privado `connection/` implementa a composição necessária, com um tipo `Engine` que mantém os serviços e as tentativas de conexão. Esse é código funcional de conexão, não uma fachada universal que apenas replica todo o catálogo interno.

## Responsabilidades

- Engine mantém contexto, PKCE, estado de retorno, listener loopback, prazo e cancelamento das tentativas.
- Providers fornece a URL e executa toda comunicação OAuth/identidade com Google, sem alterações neste trabalho.
- Accounts verifica a identidade e persiste cadastro/credenciais. A conexão também permite reautorizar uma conta existente sem mudar sua identidade.
- GTK abre o navegador, permite copiar o link e apresenta espera, erros e dados da conta.
- Orquestrador coordena sincronizações e não participa do cadastro/autorização de contas.

## Recorte da interface

Manter inicialização com recuperação de falha, boas-vindas, conexão Google, dashboard de contas, edição explícita do nome e preferências de aparência/background/About. Uma conta recém-criada abre o editor do nome com Salvar. Nenhum campo de pasta ou operação de sincronização é oferecido.

Remover seleção de pastas, observadores, download inicial, timeline, transferências, conflitos e intervalo de verificação remota. Preferências de aplicação continuam pertencendo ao aplicativo. A remoção de contas não é exposta nesta API inicial: depende da guarda do proprietário das conexões e não será substituída por uma autorização permissiva. O cadastro e o método seguro de remoção internos de Accounts permanecem preservados.

## Execução e isolamento

A engine possui executor Tokio privado. Seus métodos assíncronos podem ser aguardados pelo contexto GLib sem exigir que GTK configure Tokio. Arquivos e Secret Service são acessados fora da thread gráfica. Fechar a janela de conexão cancela a tentativa até o início da persistência; não promete desfazer escrita já iniciada.

A configuração OAuth é carregada exclusivamente pelo método existente de Providers a partir de um arquivo explicitamente selecionado pelo bootstrap. Sua ausência não impede listar contas nem abrir a interface; tentar conectar apresenta erro de configuração. Não há migração de formatos legados ou autenticação real automática.

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Orientações](AGENTS.md)
