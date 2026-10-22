# Método de desenvolvimento — decomposição e fundamentação (v0.3)

## Objetivo

Construir o DriveSync como composição de **problemas pequenos, conhecidos e verificáveis**, evitando delegar decisões arquiteturais implícitas à IA. O fato de a solução ser escrita por IA ou por uma pessoa não modifica o contrato e os critérios de correção.

A implementação deve ser auditável por alguém que esteja aprendendo Rust. Cada alteração deve caber num algoritmo ou numa integração explicitamente justificada.

## Três classes de decisões — não confundir

1. **Técnica conhecida:** possui RFC, documentação oficial, literatura ou implementação de referência (por exemplo PKCE, transações SQLite, download por staging). Citar fontes específicas, limitações e versões.
2. **Política DriveSync:** decisão do produto que não decorre automaticamente da literatura (por exemplo conflito pendente preserva as duas versões e permite sincronização de itens independentes).
3. **Hipótese em estudo:** sem comprovação de viabilidade ou ainda dependente de peculiaridade de Google Drive/Flatpak (por exemplo controle atômico de concorrência na API remota; autorização ampla; arquivos nativos Workspace). Deixar aberta e criar experimento.

Não copiar código de projetos terceiros sem verificar a licença; estudar técnicas e contratos é diferente de copiar implementação.

## Fluxo obrigatório por algoritmo

1. **Delimitar:** identificar módulo, ID, entrada, saída, invariantes, consumidores e pré-condições. Se o algoritmo exigir outro módulo, declarar a interface e um substituto de teste.
2. **Pesquisar:** consultar primeiro normas, documentação oficial, bibliografia e exemplos consolidados. Registrar link estável, trecho/conceito aplicável, limitações e data de verificação. Se não existir solução consolidada, registrar hipótese.
3. **Especificar antes de implementar:** escrever pseudocódigo determinístico, transições de estado, erros tipados, efeito permitido, recuperação de interrupções e política de segurança.
4. **Definir oráculos de teste:** criar casos com estados iniciais, entradas, falhas injetadas, saída/efeito esperado e critérios de aceite; preparar doubles/fakes para ports externos.
5. **Revisar contrato e algoritmo:** mudança de comportamento requer aprovação humana da especificação; questionamentos abertos bloqueiam alterações destrutivas.
6. **Implementar um único escopo:** solicitar à IA apenas o algoritmo identificado. Evitar alterações em módulos alheios ou bibliotecas novas sem justificativa. Fazer diffs pequenos.
7. **Validar:** revisão do código Rust (ownership, erros, concorrência, I/O), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, testes de contrato/integração e simulação de falhas conforme o caso.
8. **Integrar:** só conectar ao Coordinator depois de demonstrar contrato isolado. Verificar comportamento de ponta a ponta com provedores simulados antes de conta Drive de teste.
9. **Registrar evidência:** PR/commit vincula algoritmo, documentação, referência, riscos, testes executados e lacunas. Nunca anunciar confiabilidade sem evidência.

Para demandas com vários algoritmos ou módulos, dividir o trabalho em unidades lógicas e commits atômicos conforme `AGENTS.md`. Validar as etapas intermediárias; depois revisar o conjunto em busca de resíduos de implementação e executar testes integrados. Registrar ajustes e correções em commits próprios quando fizer sentido, sem divisão artificial por método.

## Organização modular e encapsulamento em Rust

A organização deve facilitar leitura, manutenção e localização humana do código,
sem replicar mecanicamente classes ou convenções de Java.

- **Separar por responsabilidade e coesão**, não por contagem de linhas, quantidade
  de métodos ou regra de um tipo por arquivo. Estruturas, enums e tipos relacionados
  de um módulo podem ficar reunidos em `models.rs` quando isso ajudar a localizar
  o modelo de domínio; dividir apenas se surgirem responsabilidades distintas.
- **`mod.rs` é o módulo Rust, não exclusivamente uma interface pública.** No
  DriveSync, preferir usá-lo como ponto de organização: declarar submódulos,
  controlar visibilidade e reexportar apenas a API necessária. Pequena lógica
  de integração pode permanecer ali; mover implementações substanciais para
  arquivos nomeados conforme sua responsabilidade.
- Diferenciar **dados/modelos**, **comportamento e serviços**, **adaptadores de
  I/O**, **validação** e **apresentação** quando houver motivo real para separá-los.
  Evitar arquivos genéricos ou divisões que apenas deslocam poucas linhas.
- Manter campos e detalhes internos privados por padrão; empregar `pub`,
  `pub(crate)` e `pub(super)` somente conforme o acesso necessário. Não abrir
  visibilidade apenas para facilitar uma divisão artificial.
- Em Rust, preferir composição (`struct` + `impl`) e `trait` quando existir
  contrato compartilhado ou substituição útil; não criar traits, wrappers ou
  abstrações sem consumidores ou necessidade demonstrada.
- Preservar a separação entre a GUI GTK/Libadwaita e a fachada pública do
  `drivesync-engine`. Módulos internos não devem ser expostos diretamente à UI.
- Uma refatoração de organização deve preservar comportamento, contratos,
  isolamento de contas e segurança. Planejar commits pequenos, testar cada
  unidade e revisar resíduos ao final, conforme `AGENTS.md`.

Exemplo ilustrativo (não é estrutura obrigatória):

```text
accounts/
├── mod.rs         # submódulos e interface controlada
├── models.rs      # modelos relacionados
├── repository.rs  # persistência e consultas
├── validation.rs  # regras de validação, se justificadas
└── tests.rs
```

## Documentação do código com rustdoc

**Padrão obrigatório para pessoas e agentes de IA**, aplicado
incrementalmente nas implementações e refatorações do MVP. A documentação
de arquitetura e requisitos do produto será consolidada em marcos posteriores;
o rustdoc deve acompanhar o código desde já.

Escrever todos os comentários e a documentação rustdoc em **inglês**, inclusive
nos exemplos abaixo. A regra de idioma para mensagens de commit e comentários
está em `AGENTS.md`.

### Onde e quando documentar

- Colocar `//!` no início de `lib.rs`, `mod.rs` ou do arquivo que define
  um módulo para explicar responsabilidade, limites e relacionamento com outros
  componentes, sem repetir o catálogo de arquivos da documentação externa.
- Usar `///` imediatamente antes de `struct`, `enum`, `trait`, tipos,
  funções e métodos cujos contratos precisem ser compreendidos por consumidores
  ou mantenedores. Campos com semântica ou invariantes relevantes também podem
  receber `///`.
- Cobrir APIs expostas e **itens privados relevantes** para segurança,
  persistência, concorrência, I/O e regras de domínio. Não documentar cada
  getter, auxiliar trivial ou teste só para atingir um número de comentários.
- Usar texto curto, preciso e no **presente**, explicando comportamento
  observado no código, não funcionalidades futuras nem histórico de alterações.
- Organizar a documentação perto de sua implementação, inclusive em arquivos
  como `models.rs`, `repository.rs` e `validation.rs`. Não manter cópias
  manuais da referência de métodos em documentos separados.

### Conteúdo dos contratos

- **Resumo:** o que o item faz e quais limites ou garantias importam.
- **Parâmetros e retorno:** explicar os que não forem evidentes pela assinatura,
  incluindo significado de valores opcionais, propriedade ou condições especiais.
- **`# Errors`:** para operações falíveis relevantes, indicar condições de erro
  e distinções importantes; usar links para tipos reais quando possível.
- **`# Panics`:** quando houver condições reais de pânico que componham o
  contrato; não adicionar seção vazia.
- **`# Safety`:** descrever pré-condições de cada `unsafe fn`; justificar
  blocos `unsafe` no código quando existirem. Não introduzir `unsafe`
  apenas por conveniência.
- **Invariantes e efeitos:** documentar atomicidade, isolamento por conta,
  comportamento em falhas, concorrência e alterações no filesystem ou rede
  quando fizerem diferença para uso seguro.
- **`# Examples`:** exemplos pequenos, reproduzíveis e sem dependência de
  autenticação real, rede, segredos ou dados pessoais; usar doctests quando
  a API puder ser exercitada de modo seguro e útil. Não forçar exemplos
  artificiais para cada método privado.
- Usar links intra-doc como `[`Tipo`]` e `[`Self::metodo`]` quando
  apontarem a itens resolvíveis pelo rustdoc; evitar links quebrados.

Exemplo mínimo ilustrativo de comentário sobre um método falível:

```rust
/// Records account metadata in the local index.
///
/// Replaces a saved profile with the same identity without storing tokens.
///
/// # Errors
///
/// Returns `AuthError::AccountStorage` if the saved index is invalid or
/// persistence fails.
```

A documentação deve refletir a implementação real. Não prometer sucesso
atômico ou ausência de efeitos em todas as falhas quando o código não garante
isso. Não expor tokens, IDs sensíveis ou caminhos reais de usuários.

### Verificação

A partir da raiz do workspace, quando o ambiente oferecer Rust/Cargo:

```sh
cargo doc --workspace --no-deps --document-private-items
cargo test --workspace --doc
```

O HTML gerado fica em `target/doc/`. Opcionalmente, para detectar links
intra-doc inválidos:

```sh
RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --workspace --no-deps --document-private-items
```

Continuar executando `cargo fmt`, `cargo clippy` e `cargo test` conforme
as regras de validação; **rustdoc e doctests não substituem testes de
comportamento**. Registrar resultados efetivos ou impossibilidade de execução
no commit, nunca afirmar que um comando passou sem executá-lo.

A regra acima é referência suficiente para a documentação rustdoc rotineira;
consultar fontes oficiais somente para funcionalidades pouco familiares,
incertezas de sintaxe ou regras sensíveis à versão da ferramenta. Isso não
elimina a pesquisa técnica exigida ao projetar novos algoritmos.

## Política de documentação e histórico no Git

Os documentos guardam requisitos, contratos, decisões duradouras, critérios de
aceite e instruções úteis; não são diários de implementação. Motivações,
alterações, comandos/testes realmente executados, resultados, riscos e pendências
de uma tarefa devem ser registrados no corpo do commit ou PR, conforme `AGENTS.md`.

Durante o primeiro MVP, `docs/MVP.md` é um plano temporário. Não sincronizar
preventivamente os documentos de arquitetura e de módulos a cada pequena mudança.
Depois da conclusão do MVP, confrontar a implementação real com o planejamento,
revisar e consolidar a documentação técnica e decidir a retirada do plano.

Exceção: corrigir imediatamente instruções operacionais ou informações de
segurança que se tornem incorretas e possam causar erro ou perda de dados.
A organização de conteúdos duplicados fica para uma revisão posterior.

## Política de evolução de formatos durante o desenvolvimento

Enquanto o DriveSync não tiver uma versão estável publicada, usar **um único
formato vigente** para persistência interna (incluindo perfis e pastas).
Alterações de schema atualizam diretamente leitura, escrita, validações e
testes; **não criar migradores, backups de migração ou caminhos de compatibilidade
retroativa** apenas para versões internas anteriores de desenvolvimento.

Manter um identificador de versão do formato para rejeitar dados incompatíveis
explicitamente, sem apagá-los nem sobrescrevê-los automaticamente. A necessidade
de reinicializar dados locais de desenvolvimento deve ser comunicada e executada
de forma deliberada; credenciais no cofre do sistema e arquivos de usuário não
podem ser apagados por detecção de formato antigo.

Esta política **não** dispensa gravação atômica, validação de integridade,
restrições de permissões, isolamento por conta e proteção contra perda de dados.
Antes de uma versão estável, reavaliar e documentar a necessidade de migração
para preservar configurações de usuários em futuras atualizações.

## Estados de maturidade

- **DRAFT:** comportamento esboçado, sem testes completos.
- **RESEARCHED:** referência aplicável e restrições verificadas; política própria identificada.
- **SPECIFIED:** contrato/pseudocódigo e oráculos de teste definidos.
- **REVIEWED:** revisão humana registrada.
- **IMPLEMENTED:** código corresponde ao contrato; testes executados.
- **VALIDATED:** integração e falhas adversas demonstradas.
- **BLOCKED:** hipótese técnica indispensável ainda sem evidência.

Não marcar um módulo como validado por ter código compilando. É possível que componentes individuais sejam validados antes do fluxo de sincronização completo.

## Política de testes

- **Teste unitário:** injeta dados válidos/simulados sem Google real. Ex.: autenticação real não é pré-requisito para testar Planner.
- **Teste de contrato:** substitui implementações mantendo entradas/saídas/erros e semântica.
- **Teste de integração:** usa filesystem e SQLite reais isolados; rede simulada com respostas controladas.
- **Teste real E2E:** conta Google de teste, pasta exclusiva e arquivos dispensáveis; nunca dados pessoais.
- **Falhas adversas:** interrupção entre efeito externo e confirmação, timeout, evento perdido, alteração concorrente e corrupção.

## Regras de segurança

Sem operações destrutivas quando identidade, versão ou estado estiverem `UNKNOWN`. Um HTTP 404 pode indicar falta de acesso, não apenas exclusão. Falha de rede não pode propagar exclusão. O banco SQLite não é reinicializado silenciosamente. Conflitos preservam versões até decisão segura.

## Responsabilidades da IA

Pode: procurar alternativas, sintetizar referências com links, propor pseudocódigo, implementar escopo aprovado, criar fixtures e testes, identificar falhas e explicar Rust.
Não pode: inventar capacidades da Drive API, decidir política destrutiva sem aprovação, enfraquecer invariantes para passar testes, alterar contratos de outros módulos silenciosamente, escolher dependências sem justificativa ou substituir testes por alegações.

Consultar `docs/algorithm-spec-template.md`, `docs/reference-catalog.md`, `docs/ai-development-guidelines.md` e `docs/feasibility-validation.md`.
