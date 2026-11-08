# Providers — integrações de serviços externos

## Responsabilidade

`providers/` reúne as implementações de **comunicação com APIs externas**. Cada serviço possui código, contratos e testes próprios. O objetivo **não é** desenvolver um cliente universal nem concentrar aqui a lógica de sincronização.

O provedor responde a requisições e retorna modelos específicos de seu serviço. Os módulos responsáveis por uma função do DriveSync, como detecção remota, upload ou download, decidem **quando e por que** consumir essas operações.

## Organização documental

```text
providers/
├── README.md
├── google/
│   ├── README.md
│   └── api/
        ├── README.md
        ├── INTERFACES.md
        ├── TESTS.md
        └── AGENTS.md
└── example/             # Provedor fictício, somente documentação
    ├── README.md
    └── api/
        └── README.md
```

A documentação da Google API existente foi movida **sem redesenhar o contrato** para [Google API](google/api/README.md). OneDrive e outros serviços serão adicionados apenas se implementados; não criar camadas fictícias de provedores.

## Limites

- `providers/google/api/`: protocolos Google Drive/OAuth/OIDC, recursos REST, modelos Google, erros de serviço e mecanismos de transferência de baixo nível.
- `detectors/remote/google/`: **estratégia de detectar alterações** usando operações da API Google.
- `upload/google/` e `download/google/`, quando formalizados: estratégias funcionais de transferência; seus detalhes são diferentes do protocolo HTTP da API.
- `accounts/auth/`: ciclo de vida da conta, credenciais e sessão; os clientes do provedor recebem contexto autenticado por contratos adequados.
- `models/`: modelos comuns da engine, sem importar tipos específicos da API Google.

**Não** mover código Rust nesta etapa. Esta é a estrutura-alvo **da documentação**, não uma declaração sobre os diretórios de código existentes.

## Exemplo de expansão futura

O provedor fictício [`example`](example/README.md) ilustra a separação das implementações: [API](example/api/README.md), [Detector Remoto](../detectors/remote/example/README.md), [Upload](../upload/example/README.md) e [Download](../download/example/README.md). Esses diretórios não representam funcionalidades existentes.
