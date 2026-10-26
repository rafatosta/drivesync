# Validação de autenticação e configuração

Este documento mantém **critérios de verificação**, não um histórico de implementações.
Resultados de execuções específicas, mudanças feitas e limitações observadas em
cada etapa devem ficar nas mensagens de commit ou PR correspondentes.

## Verificação automatizada

No ambiente de desenvolvimento com dependências GTK/Libadwaita e Rust disponíveis:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

Os testes automatizados não substituem a verificação de integração real.

## Aceitação manual em GNOME / Flatpak

- Confirmar configuração do OAuth Desktop conforme [orientações](google-oauth.md).
- Autenticar uma conta pelo navegador do sistema; confirmar retorno ao aplicativo e armazenamento no cofre, sem exposição de segredos.
- Configurar pastas remota e local; encerrar pelo comando **Quit** e reabrir. Confirmar restauração das configurações.
- Repetir com **duas contas independentes**, verificando alternância, identificação correta e isolamento das configurações.
- Validar navegação remota e seleção local dentro do Flatpak, incluindo cancelamento do seletor e proteção contra pastas sobrepostas.
- Testar falta de rede, cofre bloqueado, credenciais revogadas e permissão negada; preservar perfis sem declarar conexão válida.
- Confirmar tratamento de cancelamento de login e tentativas posteriores.
- Verificar layout adaptativo, teclado, estados de carregamento e mensagens de erro.
- Confirmar que nenhuma transferência de arquivos é anunciada como funcional antes de existir implementação validada.

A conclusão de cada verificação deve ser evidenciada no commit ou PR da
implementação pertinente; não manter uma cronologia de resultados neste arquivo.
