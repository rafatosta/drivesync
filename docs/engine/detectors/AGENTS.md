# Agentes — Detector (fachada pública)

## Escopo e fontes

Implementar **somente o coordenador público `detectors/`**, sem implementar automaticamente Local, Remoto ou Storage. Ler [INTERFACES](INTERFACES.md), [README](README.md), [TESTS](TESTS.md), [AGENTS raiz](../../../AGENTS.md) e [padrão](../MODULE_STANDARD.md). Consumir exclusivamente APIs privadas dos filhos, descritas em seus próprios `INTERFACES.md`.

## Regras obrigatórias

1. **Uma única API externa:** apenas Detector, eventos, lotes, requests/results e status; nenhum tipo dos filhos reexportado.
2. **Três dependências isoladas:** Local e Remoto produzem eventos individuais; Storage possui API de fila durável. O pai conhece **somente as interfaces**, não as implementações, tecnologias ou layouts.
3. **Lotes mistos:** `take_events` pode retornar Local e Remoto juntos, inclusive eventos diferentes do mesmo `SyncItem`, preservando IDs individuais e ordenação de enfileiramento.
4. **Não resolver conflitos:** juntar eventos no lote não autoriza deduplicação destrutiva, sincronização, comparação de baseline ou decisão de prevalência.
5. **Confirmação explícita:** leitura reserva; somente `acknowledge` com decisão do Orquestrador confirma ou descarta. `stop` não limpa fila; `release_events` permite reentrega.
6. **Durabilidade:** sink interno só confirma ao produtor após `Storage.enqueue` bem-sucedido. Falha implica erro/backpressure e recuperação.
7. **Estratégia livre dos filhos:** não impor polling ao Local nem watcher ao Remoto; o especialista Google é independente e vive em `detectors/remote/google/`; cada um organiza suas estruturas e testes, podendo funcionar sem irmãos concretos.
8. **Sem acesso a bancos:** Detector não usa SQL, conhece caminho de DB ou altera `inventory.sqlite3`.

## Contract-First + TDD

Testar coordenador real com **Local fake, Remoto fake e Storage fake**: os fakes são portas, não substitutos dos testes reais dos filhos. Testar batch com eventos intercalados, mesmo item em duas origens, confirmação, descarte, lease, falha de enqueue e armazenamento lento. RED → GREEN → REFACTOR e regressão por unidade, commits locais coesos.

**“Implemente o Detector”** significa concluir **oito métodos públicos** do coordenador, sem exigir implementação dos filhos reais. Para integração dos quatro módulos, solicitar explicitamente etapa posterior. Não declarar integrado se apenas fakes passaram.

Executar ferramentas Rust/Cargo **somente no Flatpak** do projeto; comentários e commits em inglês; não fazer push/PR sem autorização. Relatar contratos, testes e comandos realmente executados, commits e limites.
