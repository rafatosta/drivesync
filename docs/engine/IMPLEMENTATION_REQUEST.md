# Modelos de solicitação de implementação de módulos — DriveSync

## Objetivo

Este arquivo apresenta **exemplos de solicitações para agentes de programação** aplicando o padrão documentado em [MODULE_STANDARD.md](MODULE_STANDARD.md). Os exemplos podem ser copiados para Codex ou outro agente com acesso ao repositório e ao ambiente Flatpak.

**Interpretação:** quando se pede “implemente o módulo”, sem restringir o escopo, a tarefa é **integral**: percorrer todos os contratos públicos e testes aplicáveis. Quando se pede uma capacidade específica, a tarefa é **parcial**. Em ambos os casos, implementação incremental, TDD, segurança e limites do repositório permanecem obrigatórios.

## 1. Exemplo completo — Google API

Copiar e utilizar como instrução para o agente:

> Implemente completamente o módulo `providers/google/api/` do DriveSync, seguindo os documentos:
>
> - `docs/engine/providers/google/api/README.md`
> - `docs/engine/providers/google/api/INTERFACES.md`
> - `docs/engine/providers/google/api/TESTS.md`
> - `docs/engine/providers/google/api/AGENTS.md`
> - `AGENTS.md` da raiz.
>
> Execute o processo **Contract-First + TDD**, dividindo a implementação em unidades funcionais pequenas e coesas. Inclua **todos os 59 métodos REST, os 9 de OAuth/OIDC e os 10 de transferências/conveniência**, com tipos, erros e casos de teste previstos.
>
> Para cada unidade: (1) escreva os testes antes da implementação; (2) implemente o mínimo necessário para cumprir os contratos; (3) execute os testes e corrija falhas; (4) execute regressão dos grupos anteriores; (5) crie um commit **local**, em inglês, para a unidade validada.
>
> Continue autonomamente pelas unidades seguintes, **sem solicitar autorização para cada método ou commit local**. Não implemente todos os métodos em um único lote sem validações intermediárias.
>
> Use servidor HTTP local simulado, fixtures independentes e dados sintéticos. **Não utilize contas, tokens, serviços ou arquivos pessoais reais do Google.** Execute ferramentas Rust somente no ambiente Flatpak previsto no repositório.
>
> **Não altere contratos públicos já validados, não reduza testes para obter sucesso e não implemente responsabilidades de outros módulos.** Se alguma definição estiver tecnicamente bloqueada, documente o método/caso, o motivo e o que resta fazer. Não apresente o módulo como concluído com métodos pendentes ou testes não executados.
>
> Ao final, execute a suíte de testes aplicável e apresente um relatório com: contratos implementados e pendentes; casos executados, aprovados e reprovados; comandos e ambiente; cobertura medida se disponível; regressões; commits locais; riscos e limitações.
>
> **Não execute `git push`, não abra PR e não faça integração com conta Google real sem minha autorização explícita.**

## 2. Exemplo completo — Inventário

> Implemente completamente o módulo `inventory/` do DriveSync de acordo com `docs/engine/inventory/README.md`, `INTERFACES.md`, `TESTS.md`, `AGENTS.md` e com `AGENTS.md` da raiz.
>
> Implemente os **20 métodos públicos** e todos os tipos, erros e comportamentos aplicáveis previstos no contrato. Trabalhe em etapas coesas: abertura e identidade; leitura; observações; associação e baseline; fila; eventos remotos; histórico.
>
> Para cada etapa, siga **RED → GREEN → REFACTOR**, escrevendo primeiro testes, implementando apenas o necessário, executando regressão e registrando um commit local após validação.
>
> Execute testes com **SQLite real, em diretórios temporários isolados**. Comprove integridade relacional, atomicidade, revisões concorrentes, rollback, reabertura, durabilidade de fila/eventos/histórico, ausência comprovada versus desconhecida e preservação de tombstones.
>
> O Inventário **não faz upload, download ou análise de conflitos**, não cria banco global, não acessa Google real e não altera arquivos pessoais. Não recrie um banco inválido nem interprete indisponibilidade de pasta como exclusão.
>
> Continue autonomamente até concluir o escopo, sem pedir autorização a cada método ou commit local. Se um contrato dependente de outro módulo estiver indefinido, registre o bloqueio de forma explícita, sem improvisar sua semântica.
>
> Ao final, reporte os 20 métodos com status individual, resultados dos testes, comandos executados no Flatpak, métricas medidas quando disponíveis, commits, limitações e pendências. **Não declare conclusão integral enquanto houver contratos sem implementação ou testes obrigatórios não executados.**
>
> Não execute `git push` nem abra PR sem minha autorização explícita.

## 3. Exemplo genérico — implementação completa de qualquer módulo

> Implemente completamente o módulo `<nome_do_modulo>/` da engine DriveSync.
>
> Leia primeiro `README.md`, `INTERFACES.md`, `TESTS.md` e `AGENTS.md` específicos desse módulo, depois as regras do `AGENTS.md` global e `docs/engine/MODULE_STANDARD.md`.
>
> Trate todos os contratos públicos documentados como escopo obrigatório. Divida a execução em capacidades pequenas e testáveis; para cada capacidade, escreva os testes antes do código, implemente, rode os testes, valide regressão e faça commit local. Continue para as próximas capacidades de forma autônoma.
>
> Use fakes de serviços externos e armazenamento real isolado quando necessário; siga o ambiente Flatpak do repositório. Não faça alterações destrutivas, não mude contratos públicos nem implemente funcionalidades externas ao escopo para simplificar o trabalho.
>
> Ao finalizar, forneça um relatório de cobertura dos contratos e testes, comandos executados, commits locais, limitações e impedimentos. Se algum caso obrigatório não puder ser implementado ou testado, declare conclusão **parcial** e identifique o bloqueio.
>
> Não execute `git push` nem abra PR sem autorização explícita.

## 4. Exemplo genérico — implementação parcial

> Implemente **somente** a capacidade `<nome_da_capacidade>` do módulo `<nome_do_modulo>/` conforme seus `README.md`, `INTERFACES.md`, `TESTS.md` e `AGENTS.md`.
>
> Aplique TDD e testes isolados para todos os métodos e comportamentos **dessa capacidade**, com regressão das capacidades já implementadas. Crie commit local após validação, mas não execute `git push`/PR.
>
> Não implemente os demais grupos do módulo e não declare o módulo inteiro concluído. Informe precisamente o que foi implementado, o que foi testado e o restante do escopo pendente.

## 5. Como avaliar a entrega

| Situação | Como o agente deve apresentar o resultado |
| --- | --- |
| Todos os contratos implementados e testes aplicáveis executados com sucesso | **Implementação integral concluída**, indicando o escopo efetivamente validado |
| Uma capacidade específica implementada e validada | **Capacidade concluída; módulo parcialmente implementado** |
| Métodos não implementados, ou testes obrigatórios ausentes/não executados | **Implementação parcial**, com lista de pendências e justificativas |
| Ambiente Flatpak, dependência ou contrato impede progresso | **Bloqueado**, com descrição da causa e evidências; não simular sucesso |
| Testes com fake/servidor local passam, mas serviço real não foi acessado | **Contrato isolado validado**; integração real **não validada** |

**Importante:** a autorização para criar commits **locais** não autoriza publicar alterações no repositório remoto. O agente deve respeitar a separação entre implementação, validação e publicação, conforme [AGENTS.md da raiz](../../AGENTS.md).
