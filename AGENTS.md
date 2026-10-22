# Instruções para agentes — DriveSync

Estas regras se aplicam a alterações realizadas por agentes de IA neste repositório.

## Escopo e segurança

- Trabalhar em tarefas pequenas, com mudanças justificadas e verificáveis.
- Ler `docs/invariants.md`, `docs/development-methodology.md` e os contratos relevantes antes de alterar comportamento.
- Preservar isolamento por conta, credenciais, dados locais e garantias contra operações destrutivas.
- Não desenvolver funcionalidades adicionais, criar dependências ou refatorar módulos alheios sem necessidade demonstrável.
- Não interpretar compilar/testar como prova de validação real com Google Drive ou Flatpak.

## Fonte de verdade e documentação

- **Git é o histórico de alterações.** Não criar nem ampliar documentos com diários cronológicos, listas de arquivos modificados, relatos de cada commit, totais de testes de etapas anteriores ou narrativas do processo de implementação.
- Manter documentação voltada a contratos, requisitos, uso, invariantes, decisões duradouras e critérios de aceite; evidências de execução pontuais pertencem ao commit ou PR correspondente.
- **Durante o MVP:** manter `docs/MVP.md` como plano temporário. Não reescrever preventivamente arquitetura, contratos ou documentação dos módulos a cada implementação. Ao concluir o MVP, confrontar o código efetivamente entregue com o escopo planejado, consolidar a documentação e avaliar a remoção do plano temporário.
- Uma mudança crítica que torne uma orientação operacional ou de segurança perigosa/inválida deve ser corrigida imediatamente; não manter instruções enganosas em nome do congelamento documental.
- Não apagar contratos e decisões de projeto apenas porque a implementação ainda não chegou àquela etapa.
- Evitar informações duplicadas; a reorganização documental ampla será tratada separadamente.

## Planejamento, implementação incremental e commits atômicos

Para uma demanda com várias mudanças, **planejar antes de editar**: identificar
resultado esperado, módulos afetados, dependências, riscos e uma sequência de
entregas pequenas e verificáveis. O plano orienta o trabalho; não precisa virar
um arquivo de histórico no repositório.

- **Um commit por unidade lógica completa**, não por método, arquivo ou quantidade
  fixa de linhas. A unidade pode abranger vários arquivos quando isso for
  indispensável para entregar uma alteração coesa.
- Implementar e validar cada unidade antes de avançar à próxima; criar o
  respectivo commit assim que estiver consistente. Não acumular toda a demanda
  em um único commit final quando houver etapas independentes.
- Quando houver dependência entre módulos, ordenar os commits para que cada
  estado intermediário do repositório permaneça compilável e com testes
  relevantes passando, sempre que tecnicamente possível.
- Separar correções descobertas durante o desenvolvimento em commits `fix`
  próprios quando forem mudanças logicamente distintas. Não gerar commits
  artificiais apenas para aumentar a quantidade de commits.
- **Ao concluir a demanda**, revisar o conjunto das alterações e procurar código
  morto, funções substituídas, duplicações, artefatos temporários, imports e
  dependências desnecessárias. Fazer um commit `refactor` ou `chore` de limpeza
  somente se houver resíduos reais; não criar commit vazio.
- Rodar a validação final integrada e informar limites de teste. Se encontrar
  defeitos, corrigi-los em commits apropriados.
- Não reescrever commits já publicados para simular etapas posteriores. Se o
  ambiente não permitir commits, entregar patches e mensagens de commit
  separados por unidade lógica.

Exemplo de sequência para uma funcionalidade envolvendo dois módulos:

1. `feat(engine): implementar contrato de listagem` — lógica e testes do módulo A.
2. `feat(ui): integrar listagem na interface` — consumo e testes do módulo B.
3. `fix(ui): tratar erro de paginação` — correção específica identificada nos testes.
4. `refactor: remover auxiliar substituído` — **somente se** a revisão final
   detectar código obsoleto.

## Commits como registro de implementação

Ao finalizar uma alteração, preparar um commit com:

1. **Título curto e objetivo**, preferencialmente `tipo: descrição` (`feat`, `fix`, `refactor`, `test`, `docs`, `chore`).
2. **Motivação e escopo:** qual problema foi tratado e por quê.
3. **Mudanças:** comportamento e arquivos/componentes relevantes, sem repetir o diff linha a linha.
4. **Validação:** comandos e testes realmente executados, resultados e ambiente quando relevante. Não declarar testes não executados.
5. **Limitações, riscos e pendências:** verificações não feitas e próximos passos necessários, quando houver.

Criar cada commit ao concluir sua unidade lógica, quando o ambiente e as permissões permitirem; se não for possível, fornecer a mensagem de commit pronta e informar a limitação. Não alterar histórico publicado nem misturar tarefas independentes sem autorização.

## Commits locais e publicação remota

- **No ambiente local (Codex, terminal ou outros agentes com acesso ao checkout):**
  depois de validar cada unidade lógica, executar `git add` seletivamente e
  `git commit` automaticamente, sem solicitar autorização adicional para cada
  commit, desde que o ambiente permita essas operações.
- Não incluir alterações pré-existentes ou de terceiros no commit; verificar
  `git status` e revisar o diff preparado antes de confirmar.
- **Nunca executar `git push`, criar PR, publicar release, alterar tags remotas
  ou sincronizar alterações com o GitHub sem autorização explícita do usuário
  para essa operação.** O envio é uma etapa separada da criação de commits.
- Em ambientes de integração remota, como conectores GitHub, não interpretar
  esta regra como permissão para enviar alterações diretamente. Respeitar
  autorizações e restrições específicas do ambiente.
- Ao finalizar, informar branch, commits locais criados e resultado dos testes;
  deixar claro que os commits ainda não foram enviados ao remoto. Quando o
  ambiente não permitir commits locais, informar isso e fornecer as mensagens
  sugeridas em vez de simular a criação dos commits.

## Formatos internos antes da primeira versão estável

- Manter somente o formato atual de persistência, sem migradores ou compatibilidade retroativa entre versões internas de desenvolvimento.
- Rejeitar dados incompatíveis sem apagá-los automaticamente.
- Preservar escrita atômica, integridade, permissões e isolamento de credenciais e pastas.

## Verificação

Antes de concluir, executar quando disponível:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Registrar o que foi efetivamente executado no commit. Testes de integração real exigem ambiente isolado e consentimento apropriado.
