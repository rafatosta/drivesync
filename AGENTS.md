# Instruções para agentes — DriveSync

Aplicam-se a todos os agentes de IA que modificam este repositório.

## 1. Idioma e escopo

- Escrever commits (título e corpo), comentários, rustdoc e comentários em exemplos de código **sempre em inglês**.
- Realizar alterações pequenas, justificadas, verificáveis e limitadas ao escopo solicitado.
- Preservar isolamento entre contas, credenciais, dados locais e segurança das operações.
- **Proibir exclusão permanente:** utilizar sempre a lixeira. Se indisponível, recusar a operação, sem fallback destrutivo. APIs públicas de exclusão permanente devem manter a assinatura, emitir aviso e recusar a execução, sem declarar sucesso.
- Não adicionar funcionalidades, dependências ou refatorações fora do escopo sem necessidade demonstrada.

## 2. Retrocompatibilidade

- **Nesta fase, não há exigência de retrocompatibilidade.** Não criar migrações, adaptadores, aliases, fallbacks ou suporte legado.
- Antes de introduzir compatibilidade entre versões, consultar o usuário, explicar o impacto e aguardar decisão.
- Decisões já estabelecidas para a tarefa não precisam ser reconfirmadas.
- Remover implementações substituídas dentro do escopo, sem excluir dados do usuário ou alterar contratos externos não relacionados.

## 3. Arquitetura e organização

- Em Rust, preferir `mod.rs` para organização e reexportação, `models.rs` para modelos relacionados e arquivos separados por responsabilidade.
- Evitar fragmentação excessiva, abstrações artificiais e duplicação de responsabilidades.
- Preservar contratos e interfaces entre módulos, salvo quando a alteração fizer parte do escopo autorizado.
- Priorizar modularidade, baixo acoplamento e separação de responsabilidades.

## 4. Documentação Rust

- Durante o desenvolvimento, documentar apenas invariantes, riscos e comportamentos não óbvios.
- Após a consolidação de cada módulo, adicionar rustdoc (`//!` e `///`) aos contratos e APIs relevantes.
- Manter a documentação existente sincronizada com alterações de comportamento.
- Evitar comentários triviais e documentação de implementações provisórias.
- Não pesquisar rotineiramente a sintaxe rustdoc; consultar referências oficiais apenas diante de dúvidas ou recursos desconhecidos.
- Executar `cargo doc` e doctests quando houver documentação aplicável, sempre no ambiente Flatpak do projeto.

## 5. Planejamento e implementação

- Para tarefas com múltiplas alterações, planejar resultado, módulos, dependências, riscos e etapas antes de editar. Não criar arquivo de planejamento sem necessidade.
- Implementar, validar e criar **um commit por unidade lógica completa**, não por arquivo ou método.
- Ordenar etapas dependentes para preservar compilação e testes nos estados intermediários, quando possível.
- Registrar correções independentes em commits `fix`, sem fragmentação artificial.
- Ao finalizar, revisar código morto, duplicações, imports, dependências e artefatos temporários. Criar commit de limpeza apenas se houver alterações reais.
- Executar validação integrada final, corrigir falhas e declarar limitações.
- Não reescrever histórico publicado para reorganizar etapas.

## 6. Commits e Git

- Usar títulos curtos em inglês, preferencialmente `type(scope): description` (`feat`, `fix`, `refactor`, `test`, `docs`, `chore`).
- Registrar no corpo, de forma objetiva: motivação, mudanças, validações realmente executadas, limitações e pendências relevantes.
- Em ambiente local, verificar `git status`, revisar o diff, executar `git add` seletivo e `git commit` automaticamente após validar cada unidade lógica.
- Não incluir alterações preexistentes ou de terceiros.
- **Nunca executar `git push`, criar PR, publicar release ou modificar tags remotas sem autorização explícita.** Commit local não autoriza publicação.
- Respeitar as permissões específicas de conectores e ambientes remotos.
- Se commits locais não forem possíveis, fornecer patches e mensagens sugeridas, sem simular sua criação.
- Ao concluir, informar branch, commits criados, testes e pendências, indicando que não houve publicação remota.

## 7. Ambiente Rust obrigatório

- Executar **todas as ferramentas Rust exclusivamente no ambiente Flatpak** definido em `flatpak/com.rtosta.DriveSync.Devel.yml`, incluindo Cargo, rustc, rustfmt, Clippy, documentação e scripts relacionados.
- O manifesto determina SDK, versão, extensões e ambiente. Nunca utilizar ferramentas Rust do host como fallback.
- Usar Flathub para SDK e extensões. Não instalar ou atualizar componentes automaticamente.
- Se o ambiente estiver indisponível, informar o impedimento e as verificações não realizadas.
- Remote Flatpak adicional somente quando necessário para distribuição e testes de instalação/atualização; não substitui o ambiente de desenvolvimento.
- Registrar nos commits os comandos executados e o ambiente utilizado.
- Testes no SDK não comprovam integração real com Google Drive nem funcionamento do pacote Flatpak.

## 8. Verificação final

Executar quando disponível, **dentro do ambiente Flatpak**, nunca no host:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Após a consolidação e documentação dos módulos envolvidos, executar também:

```sh
cargo doc --workspace --no-deps --document-private-items
cargo test --workspace --doc
```

- Registrar resultados reais e verificações não executadas.
- Testes com contas ou dados reais exigem ambiente isolado e consentimento apropriado.
- Não declarar sucesso de verificações que não foram executadas.
