# Arquitetura do DriveSync

## Separação de responsabilidades

- `modules/`: especificações dos módulos do motor, independentes de linguagem.
- `src/`: aplicativo inicial Rust/Libadwaita. A interface não contém regras de sincronização.
- `docs/`: contratos globais, estados e invariantes entre módulos.

## Fluxo previsto

GUI GNOME → API do motor → coordenador → planejador → operações (upload/download/conflitos) → provedores locais/remotos e repositório de estado.

As interfaces entre componentes devem ser explícitas, com entradas, saídas, erros e invariantes documentados. Comunicação interprocessos via D-Bus é prevista para etapa posterior, não obrigatória agora.
