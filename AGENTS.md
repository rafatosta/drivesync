# Instruções para agentes — DriveSync

Aplicam-se a todos os agentes de IA que modificam este repositório.

## 1. Contexto e eficiência

- Usar o código atual como fonte primária da implementação e `docs/README.md` como índice documental.
- Ler apenas arquivos, contratos e documentação relevantes à tarefa. Evitar varreduras integrais, buscas repetidas e releituras de conteúdo inalterado.
- Consultar histórico do Git e referências externas somente quando necessário.
- Reutilizar informações já obtidas durante a tarefa.
- Não criar relatórios, planejamentos ou documentos intermediários desnecessários.
- Atualizar documentos existentes em vez de duplicar conteúdo.

## 2. Escopo e segurança

- Realizar alterações pequenas, justificadas, verificáveis e restritas ao escopo solicitado.
- Preservar isolamento entre contas, credenciais, dados e segurança das operações.
- Proibir exclusão permanente de arquivos e pastas. Usar exclusivamente a lixeira; se indisponível, recusar sem fallback destrutivo.
- APIs de exclusão permanente devem manter suas assinaturas, emitir aviso e retornar erro sem executar a operação.
- Não introduzir funcionalidades, dependências ou refatorações não justificadas.
- Não sobrescrever alterações preexistentes ou de terceiros.

## 3. Retrocompatibilidade

- Não implementar retrocompatibilidade, migrações, adaptadores, aliases ou fallbacks legados sem autorização.
- Quando necessário, explicar o impacto e aguardar decisão.
- Não reconfirmar decisões já estabelecidas.
- Remover implementações substituídas dentro do escopo, preservando dados do usuário e contratos não relacionados.

## 4. Arquitetura

- Priorizar modularidade, baixo acoplamento, coesão e responsabilidade única.
- Preservar contratos entre módulos, salvo alterações autorizadas.
- Preferir `mod.rs` para organização e reexportação, `models.rs` para modelos relacionados e arquivos separados por responsabilidade.
- Evitar fragmentação excessiva, abstrações artificiais e duplicação de responsabilidades.
- Encapsular detalhes internos dos módulos e expor somente os contratos necessários aos consumidores.

## 5. Documentação e idioma

- Escrever commits, comentários, rustdoc e comentários em exemplos de código sempre em inglês.
- Durante o desenvolvimento, documentar apenas invariantes, riscos e comportamentos não óbvios.
- Após consolidar um módulo, documentar seus contratos e APIs relevantes com rustdoc.
- Manter a documentação coerente com a implementação vigente.
- Evitar comentários triviais, documentação provisória e reprodução desnecessária do código.
- Consultar referências de rustdoc apenas diante de dúvidas concretas.

## 6. Implementação

- Planejar tarefas complexas considerando módulos, dependências, riscos e critérios de aceitação.
- Implementar, validar e criar um commit por unidade lógica completa.
- Ordenar etapas para preservar compilação e funcionamento intermediário, quando possível.
- Executar testes direcionados durante a implementação e validação integrada ao final.
- Evitar repetir verificações sem alterações que justifiquem nova execução.
- Revisar código morto, duplicações, imports, dependências e artefatos temporários.
- Registrar correções independentes em commits `fix` e limpezas somente quando houver alterações reais.

## 7. Git

- Verificar `git status` antes de editar e revisar o diff antes de cada commit.
- Usar `git add` seletivo e criar commits locais automaticamente após validação.
- Usar títulos curtos no formato `type(scope): description`.
- Registrar no corpo motivação, alterações, validações executadas e limitações relevantes.
- Não reescrever histórico publicado.
- Nunca executar push, criar PR, publicar release ou modificar referências remotas sem autorização explícita.
- Se commits locais forem impossíveis, fornecer patches e mensagens sugeridas.
- Respeitar as permissões do ambiente e dos conectores.

## 8. Ambiente Rust

- Executar todas as ferramentas Rust exclusivamente no ambiente Flatpak definido em `flatpak/com.rtosta.DriveSync.Devel.yml`.
- Respeitar SDK, extensões e configurações do manifesto. Nunca utilizar ferramentas Rust do host como fallback.
- Usar Flathub para SDK e extensões, sem instalar ou atualizar componentes automaticamente.
- Utilizar remotes adicionais apenas para distribuição ou testes de instalação.
- Informar indisponibilidade do ambiente e verificações não executadas.
- Testes no SDK não comprovam integração real com provedores ou funcionamento do pacote instalado.

## 9. Validação

Para alterações Rust, executar no ambiente Flatpak:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Após consolidar e documentar os módulos envolvidos:

```sh
cargo doc --workspace --no-deps --document-private-items
cargo test --workspace --doc
```

- Adequar as verificações à natureza e ao impacto das alterações.
- Corrigir falhas relacionadas à tarefa antes de concluir.
- Testes com contas ou dados reais exigem ambiente isolado e consentimento apropriado.
- Nunca declarar sucesso de verificações não executadas.

## 10. Entrega

Informar objetivamente:

- Alterações realizadas.
- Branch e commits criados.
- Validações executadas e respectivos resultados.
- Limitações, bloqueios e pendências.
- Situação da publicação remota.

Não repetir informações já registradas nem produzir relatórios extensos sem solicitação.