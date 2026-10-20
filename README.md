# DriveSync

Cliente não oficial de sincronização do Google Drive para Linux/GNOME.

**Fase atual:** projeto inicial executável e especificação modular. Ainda não há autenticação nem sincronização implementadas.

## Princípios
- Motor independente da interface GNOME.
- Especificações e contratos de módulos independentes da linguagem.
- Algoritmos, limites, erros e testes documentados antes da implementação.
- Nenhuma operação destrutiva sem verificação explícita do estado anterior.

## Execução inicial
Instale Rust, GTK4 e Libadwaita com seus pacotes de desenvolvimento e execute `cargo run`.

O protótipo apenas abre uma janela com o texto **DriveSync**.

A integração Flatpak com GNOME Builder ainda precisa ser validada.
