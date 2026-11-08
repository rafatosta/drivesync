# Padrão mínimo de documentação dos módulos — DriveSync

## 1. Finalidade e aplicação

Esta especificação orienta **novos módulos da engine** a partir dos padrões consolidados em [Google API](providers/google/api/README.md) e [Inventário](inventory/README.md). O objetivo é definir uma base **mínima, consistente e reutilizável**, sem impedir que cada módulo acrescente seções pertinentes à sua natureza. Deve ser aplicada a módulos como `accounts/`, `configuration/`, detectores, analisador, orquestrador e operadores.

**Quatro documentos constituem a base documental recomendada:**

| Documento | Responde | É fonte de verdade para |
| --- | --- | --- |
| `README.md` | O que é o módulo? O que possui, conhece, faz e não faz? | Domínio, responsabilidades, dados, fronteiras e regras arquiteturais |
| `INTERFACES.md` | Como os demais módulos interagem com ele? | Assinaturas, tipos, campos, obrigatoriedade, retornos, erros e comportamento público |
| `TESTS.md` | Como demonstrar que os contratos são respeitados? | Cenários positivos/negativos, fixtures, verificações, critérios de aceite |
| `AGENTS.md` | Como um agente deve trabalhar sem violar os contratos? | Escopo de alteração, leitura obrigatória, TDD, limitações, segurança e entrega |

**Não criar um arquivo por tabela, struct ou método:** cada documento pode conter quantas seções forem necessárias. Não reproduzir no `README.md` todas as assinaturas; não transferir para `AGENTS.md` toda a matriz de testes. Utilizar links relativos.

Um módulo pode ter **nenhum banco, nenhuma dependência HTTP ou nenhuma operação assíncrona**. Essas características são específicas, não requisitos do padrão. **Também não são exigidos números mínimos de métodos, campos, tabelas, testes ou páginas.**

### Submódulos com implementação independente

Quando um componente possui submódulos que são **unidades reais de implementação independentes**, cada submódulo também deve possuir seus próprios `README.md`, `INTERFACES.md`, `TESTS.md` e `AGENTS.md`. O componente pai pode expor uma API externa única enquanto **consome somente contratos internos tipados** dos filhos, sem exigir a mesma implementação ou dependências concretas.

**Isolamento de testes:** testar o pai com fakes das portas dos filhos; testar cada filho real isoladamente, usando fakes das próprias dependências e recursos reais temporários quando aplicável. “Implementar o pai” **não significa** implementar automaticamente todos os filhos, nem “implementar um filho” exige que seus irmãos estejam prontos. Integração conjunta é uma etapa separada.

Exemplo: [Detector](detectors/README.md) com [Local](detectors/local/README.md), [Remoto](detectors/remote/README.md) e [Storage](detectors/storage/README.md). O Detector coordena uma **fila privada de eventos** pelo Storage; Local/Remoto produzem eventos individuais; o Orquestrador consome **lotes mistos** exclusivamente pela API pública do Detector. **Lote misto não resolve conflitos de sincronização.**

### Organização por responsabilidade e por provedor

A documentação acompanha o **proprietário da responsabilidade**: APIs externas em [`providers/`](providers/README.md), com [Google API](providers/google/api/README.md); a estratégia Google do Detector Remoto em [`detectors/remote/google/`](detectors/remote/google/README.md); contratos comuns em `models/`; execução funcional de Upload/Download nos seus próprios módulos. **A implementação de um especialista é independente da implementação dos seus irmãos**, e seus testes ficam junto à sua documentação. Não duplicar contratos HTTP nos especialistas.

Esta é uma estrutura **documental proposta**, que não implica migração física de código Rust, novas funcionalidades ou modelos já implementados.

## 2. README.md — responsabilidades e modelo do módulo

### Seções mínimas

1. **Objetivo e escopo:** descrever em termos claros a finalidade, os consumidores e a responsabilidade principal.
2. **Responsabilidades e exclusões:** enumerar o que o módulo faz e, igualmente importante, o que não faz; declarar o proprietário de cada informação.
3. **Dados e conceitos de domínio:** definir entidades e estados essenciais; explicar relações e regras importantes; diagramas só quando agregam clareza.
4. **Integração com outros módulos:** indicar o que recebe, o que fornece e quais contratos externos utiliza, sem duplicá-los.
5. **Persistência e efeitos externos, quando aplicáveis:** descrever se existem banco, arquivos, rede, credenciais, fila, cache ou eventos, bem como proprietário, escopo e segurança.
6. **Invariantes e decisões de projeto:** garantias que nunca podem ser violadas; exemplos concretos quando necessário.
7. **Referências:** apontar para `INTERFACES.md`, `TESTS.md`, arquitetura geral e fonte externa normativa, se houver.

### Estrutura sugerida

```markdown
# Nome do módulo

## Objetivo
Descrição do problema resolvido e de seus consumidores.

## Responsabilidades e limites
O que faz e o que não faz; separação dos módulos vizinhos.

## Modelo conceitual
Entidades, estados, relacionamentos e regras.

## Dependências e comunicação
Dados recebidos e disponibilizados, com links para os contratos.

## Persistência e efeitos externos (se houver)
Propriedade dos dados, armazenamento privado e segurança.

## Invariantes
Garantias de comportamento e situações que não podem ocorrer.

## Documentação associada
INTERFACES.md, TESTS.md e AGENTS.md.
```

**Decisão central:** o módulo é proprietário dos **seus dados e regras**, e pode adotar GSettings, SQLite, arquivo ou nenhum mecanismo de persistência. Os consumidores não acessam diretamente sua implementação nem seu armazenamento; usam **API**.

## 3. INTERFACES.md — contrato público e documentação funcional

### Seções mínimas

1. **Objetivo e fronteira pública:** o que é exposto e o que permanece interno.
2. **Tipos públicos:** IDs, entidades, estruturas de entrada/saída, enums, filtros, resultados e erros, apenas os necessários.
3. **Assinaturas:** contratos Rust ou assinatura equivalente conforme o componente, com visibilidade explícita.
4. **Semântica dos atributos:** o que significa cada campo, qual é obrigatório ou opcional e **por que** pode estar ausente. Não tratar `None` como exclusão ou limpeza sem regra expressa.
5. **Contrato funcional individual de cada método:** finalidade, parâmetros, retorno, limites e erros.
6. **Regras transversais e evolução:** concorrência, paginação, idempotência, efeitos colaterais e garantias de compatibilidade, somente se aplicáveis.

### Formato mínimo obrigatório de apresentação de cada método

```markdown
### metodo_publico

**Assinatura:** `fn metodo_publico(...)` — preservar a declaração real.

**Finalidade:** Qual problema resolve e quando deve ser chamado.

**Parâmetros:** Explicar cada argumento e cada atributo relevante
do objeto de entrada. Para atributos opcionais, explicar o
significado de ausência, valor presente e eventuais restrições.

**Retorno:** Qual objeto devolve, significado de campos condicionais,
resultado vazio, sucesso parcial ou ausência quando aplicável.

**Limites e erros:** O que este método não executa, seus efeitos
colaterais autorizados, pré-condições e falhas tipadas.
```

**Concisão útil:** não basta repetir o nome de um campo (`item_id`: “ID do item”). Descrever **a qual item pertence, como é validado e o que acontece se estiver ausente/incorreto**. Para operações simples, cada parte pode ser curta.

### Objetos de entrada e saída

- **Módulos próprios do DriveSync:** preferir um objeto de requisição coeso e um de resposta nomeado por operação, sobretudo quando os parâmetros pertencem à mesma ação. Ex.: `fn get_item(&self, request: GetItemRequest) -> InventoryResult<GetItemResult>`.
- **Adaptação de contratos externos:** respeitar a assinatura já validada ou imposta pelo serviço/SDK. O `providers/google/api/` é exemplo: IDs, sessão, `options` e `request` podem ser parâmetros distintos. **Não converter contratos existentes para objeto único apenas por padronização visual.**
- O objeto de entrada não deve virar um “saco” genérico de opções desconexas. Separar componentes coesos e usar enums para escolhas mutuamente exclusivas.
- Diferenciar `None` (**não fornecido/desconhecido**, conforme contrato) de `Clear` (**limpeza explícita**) e de estados como ausência comprovada.
- A adição de um campo à `pub struct` pode quebrar construções por literal em Rust, mesmo sem alterar a assinatura do método. Utilizar builders/construtores/`Default` quando fizer sentido, **sem prometer compatibilidade automática**.
- Tipos de outro módulo permanecem de propriedade dele; reutilizar sua API e não manter duas cópias divergentes.
- O contrato define o comportamento atual; **não escrever “comparado com a versão anterior” como justificativa**. Registrar decisões de projeto de modo atemporal.

### Separação entre interface e implementação

Não expor SQL, conexão SQLite, transação, implementação HTTP, caminhos internos, tokens ou formato privado de persistência como API de negócio. Exceções técnicas devem ser deliberadas e documentadas, nunca surgir apenas por conveniência da implementação.

## 4. TESTS.md — matriz e casos descritivos por método

### Seções mínimas

1. **Objetivo e fontes de verdade:** vincular `INTERFACES.md` e `README.md`.
2. **Abordagem TDD e isolamento:** RED → GREEN → REFACTOR, fixtures independentes e meios de simulação.
3. **Índice de cobertura:** relacionar **todos** os métodos públicos a IDs de casos, como `MOD-01`, `MOD-02` (prefixo pertinente ao módulo).
4. **Caso individual de cada método:** sucesso, negativos/opcionais, retornos e invariantes a verificar.
5. **Casos transversais:** erros, consistência, segurança e, quando houver, integração, persistência, concorrência, paginação ou recuperação.
6. **Metas, critérios de aceite e evidências:** o que deve ser executado para afirmar que funciona e como registrar limitações e pendências.

### Formato mínimo obrigatório de teste por método

```markdown
### MOD-01 — `metodo_publico`

**Contrato:** Assinatura/tipos e referência ao INTERFACES.md.

**Sucesso:** Ambiente inicial, dados de entrada, ação exercitada
e resultado que deve ser observado/confirmado.

**Negativos e opcionais:** Entradas inválidas, atributos omitidos,
pré-condições violadas, falhas de infraestrutura e efeitos que
não podem acontecer.

**Retorno e invariantes:** Campos esperados no objeto de saída,
erros tipados, efeitos permitidos e garantias preservadas.

**Verificação:** Evidência objetiva por teste — comparação de objetos,
requisição HTTP capturada, estado durável após reabertura,
rollback, eventos emitidos etc., conforme o componente.
```

### Seleção da técnica de teste — conforme a responsabilidade

| Natureza do módulo | Técnica principal | Não basta |
| --- | --- | --- |
| Serviço HTTP/provedor, como Google API | **Cliente real do módulo** apontado a servidor HTTP local simulado | Mock do trait que apenas retorna valores esperados |
| Módulo com banco, como Inventário | **Armazenamento real isolado**, com FKs/transações e reabertura | Mock do banco que não exercita persistência |
| Módulo de filesystem | Diretórios e arquivos temporários reais, com limites seguros | Simulação que ignora nomes, permissões e movimentações |
| Módulo que decide políticas | Funções/regras testadas com estados sintéticos e dependências falsas | Testar apenas que um método foi chamado |
| Orquestração e filas | Provedores falsos + estados/dados reais quando apropriado, recuperação e concorrência | Verificar apenas o caminho feliz |
| Configurações e contas | API pública + backend isolado apropriado e credenciais fictícias | Acesso a dados do perfil do desenvolvedor |

**Asserções devem ser independentes do código testado.** Não gerar o valor esperado com o mesmo serializador, query builder ou decisor sob teste. Testar campos opcionais e casos de erro; não inventar números-limite não definidos no contrato.

### Critérios mínimos de aceite

- Cada método público possui pelo menos cenários de **sucesso**, **falha**, **opcionalidade quando aplicável** e **ausência de efeitos proibidos**.
- Objetos públicos possuem cobertura da presença/ausência de campos condicionais; resultados com `None` não equivalem automaticamente a falha.
- Efeitos duráveis devem sobreviver à reabertura; operações transacionais precisam demonstrar rollback; concorrência precisa de teste quando houver risco.
- O módulo é testável de forma independente, com fakes das **dependências**, e não da própria unidade sob teste.
- A porcentagem de cobertura não substitui a comprovação dos invariantes importantes. **Não registrar como aprovado um cenário que não foi executado.**
- Caso o contrato tenha lacuna, marcar caso como **pendente/bloqueado**, documentar a decisão e atualizar `INTERFACES.md` antes de implementar o comportamento que depende dela.

## 5. AGENTS.md — regras operacionais específicas

### Seções mínimas

1. **Escopo e referências:** diretório que o agente pode modificar e sequência de leitura `INTERFACES.md` → `README.md` → `TESTS.md` → `AGENTS.md` global.
2. **Responsabilidades e fronteiras:** o que não pode implementar, acessar ou alterar.
3. **Invariantes e segurança:** riscos específicos do módulo (secrets, perdas de dados, concorrência, estado parcial).
4. **Fluxo obrigatório de trabalho:** TDD, resolução prévia de ambiguidades, implementação incremental e regressão.
5. **Ambiente de testes:** ferramentas e isolamento (SDK/Flatpak, fake remoto, SQLite/FS reais temporários conforme necessário).
6. **Entrega e evidências:** resultados de testes realmente executados, pendências, mudanças e limites.

### Estrutura sugerida

```markdown
# Instruções para agentes — <Módulo>

## Escopo e referências
O que implementar; ler INTERFACES, README, TESTS e AGENTS global.

## Contratos e limites
Não mudar API pública; não implementar responsabilidades alheias.

## Invariantes e segurança
Garantias próprias do módulo; proteção contra perdas de dados.

## Método de trabalho
Contract-First + TDD; RED, GREEN, REFACTOR, regressão, commits coesos.

## Ambiente e testes
Flatpak definido pelo projeto; dependências simuladas;
armazenamento real isolado quando aplicável.

## Entrega
Métodos, cenários, comandos, resultados e limitações verificáveis.
```

**Não duplicar** o `AGENTS.md` global: referenciá-lo. As regras da raiz continuam válidas (commits e comentários em inglês; `cargo`/Rust **exclusivamente no Flatpak** do projeto; commits atômicos, sem `git push`/PR sem autorização explícita). O `AGENTS.md` local serve para especializar, não substituir, essas regras.

### Regra de execução integral ou parcial

**Por padrão, a solicitação “implemente o módulo `X`” significa implementar integralmente o escopo documentado de `X`**, e não apenas preparar estrutura, criar testes ou implementar o primeiro grupo. O agente deve usar `README.md`, `INTERFACES.md`, `TESTS.md` e `AGENTS.md` como conjunto de referência e **percorrer todos os contratos e respectivos casos de teste**, dividindo o trabalho em unidades funcionais pequenas, com RED → GREEN → REFACTOR, regressão e commits locais coesos. Avançar autonomamente entre unidades **não** significa implementar tudo de uma vez ou adiar validações.

**Implementação parcial:** quando a solicitação nomear uma capacidade, método, grupo ou etapa, executar **apenas esse escopo**, preservando contratos e registrando o restante como pendente, sem anunciar o módulo completo.

**Critério de conclusão da implementação integral:** todos os contratos públicos do módulo e seus testes aplicáveis estão implementados, os testes foram efetivamente executados no ambiente exigido, a regressão está verificada e não restam pendências bloqueadoras dentro do escopo. Relatar lista de contratos concluídos, testes e comandos executados, falhas, cobertura apurada (quando disponível), commits locais e limitações. Não afirmar “concluído”, “testado” ou “integrado” quando houver método não implementado, suíte não executada ou verificação impossível. Em caso de impedimento técnico real, documentar bloqueio específico, evidência, impacto e trabalho restante; não preencher lacunas contratuais com comportamentos imaginados.

**Autonomia e limites:** não pedir confirmação a cada método ou commit local; os limites do módulo, as decisões contratuais já validadas, o isolamento, a proteção contra perdas e a proibição de `git push`/PR sem autorização explícita **continuam válidos**. A solicitação de implementação completa **não autoriza** alteração unilateral de contrato público, testes relaxados para passar ou integração real com serviços sem permissão.

**Modelo de solicitação:** consultar [IMPLEMENTATION_REQUEST.md](IMPLEMENTATION_REQUEST.md), com exemplos completos e parciais reutilizáveis.

## 6. Fluxo recomendado para criar um novo módulo

1. Consultar [SYSTEM_ARCHITECTURE.md](../SYSTEM_ARCHITECTURE.md) para compreender responsabilidade e relações.
2. Criar `README.md` com objetivo, fronteiras, dados e invariantes; **não começar por tabelas nem classes da implementação**.
3. Definir tipos, erros e **todas as interfaces públicas previstas** em `INTERFACES.md`. Documentar individualmente cada método, inclusive a semântica dos opcionais.
4. Criar `TESTS.md`: índice completo de métodos, cenários textuais individuais e matriz dos riscos específicos.
5. Criar `AGENTS.md` do módulo com escopo e método TDD, referenciando os outros documentos e a raiz.
6. Revisar consistência cruzada: toda interface deve ter teste; toda persistência, decisão e regra importante no README deve ter evidência; AGENTS não deve mandar fazer o contrário.
7. **Só então implementar de forma incremental**, começando pelos testes dos contratos. Não construir tudo de uma vez nem declarar testes simulados como prova de integração real.

## 7. Checklist mínimo de revisão documental

- [ ] `README.md` explica finalidade, limites, responsabilidades, modelos e invariantes.
- [ ] `INTERFACES.md` contém todos os métodos e tipos públicos previstos.
- [ ] Cada método tem **finalidade, parâmetros, retorno, limites e erros**, com campos opcionais explicados.
- [ ] `TESTS.md` enumera todos os métodos sem omissões ou duplicações.
- [ ] Cada método possui **sucesso, negativos/opcionais, retorno/invariantes e verificação objetiva**.
- [ ] Existem testes transversais para efeitos externos, persistência, concorrência e segurança **quando aplicáveis**.
- [ ] `AGENTS.md` explicita restrições, ordem de leitura, TDD, ambiente e evidências de entrega.
- [ ] Links entre arquivos estão corretos; sem duplicação de especificações ou afirmações de testes não executados.
- [ ] Detalhes internos de SQLite/HTTP/credenciais não escapam sem necessidade por APIs públicas.
- [ ] O padrão **não força** mudanças em assinaturas já validadas, em contratos de terceiros ou em módulos consolidados.

## 8. Modelos de referência e decisão de manutenção

| Referência | O que exemplifica |
| --- | --- |
| [providers/google/api/README.md](providers/google/api/README.md), [INTERFACES](providers/google/api/INTERFACES.md), [TESTS](providers/google/api/TESTS.md), [AGENTS](providers/google/api/AGENTS.md) | Contratos extensos de integração externa; método a método sem alterar protocolo; servidor HTTP simulado |
| [inventory/README.md](inventory/README.md), [INTERFACES](inventory/INTERFACES.md), [TESTS](inventory/TESTS.md), [AGENTS](inventory/AGENTS.md) | Contratos próprios com requisição/resposta; persistência SQLite privada; baseline, fila, histórico e TDD transacional |

**Este documento padroniza o mínimo, não reabre as decisões técnicas dos módulos de referência.** Novos módulos podem adicionar seções e artefatos quando necessário; mudanças no padrão devem manter os critérios de legibilidade, documentação contratual, verificabilidade e isolamento.
