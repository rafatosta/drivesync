# Orientações para agentes — Scanner Local

## Objetivo

Implementar um Scanner **somente local**, com os métodos `new` e `scan`, que percorre uma raiz de sincronização e atualiza as observações locais do Inventário, continuando diante de falhas individuais de arquivo.

## Leitura obrigatória

- [README.md](README.md): objetivo e limites.
- [INTERFACES.md](INTERFACES.md): tipos, métodos, erros, relatório e integração.
- [TESTS.md](TESTS.md): testes TDD e critérios de aceite.
- [Inventário](../inventory/INTERFACES.md): `SyncItem`, `record_local`, revisões, presenças, `ObservationProof` e baseline.
- [Models](../models/INTERFACES.md): `File`, `FileData`, `LocalFile` e metadados indisponíveis.
- [AGENTS.md da raiz](../../../AGENTS.md): convenções, segurança e ambiente Flatpak.

## Regras de implementação

- Percorrer apenas o filesystem local; **não** adicionar `Remote` ao Scanner.
- Criar observações com `FileData::Local` e `ObservationSource::LocalScan` utilizando os modelos compartilhados.
- Persistir observações exclusivamente pela API pública do Inventário. Não abrir SQLite diretamente nem alterar baseline, dados remotos, fila ou histórico de operações confirmadas.
- Não utilizar Detector Local ou Remoto, suas filas, seus checkpoints ou armazenamento próprio.
- Não chamar Analisador, Reconciliador, Conflitos ou Operador; não produzir comandos de sincronização.
- Diante de erro individual, registrar diagnóstico e continuar em outros arquivos e ramos. `Completed` pode conter problemas individuais; `Partial` caracteriza perda de cobertura/persistência relevante.
- Reservar `ScannerResult::Err` a falhas estruturais, escopo inválido, cancelamento e impossibilidade geral de trabalhar com raiz/Inventário.
- Não presumir ausência de arquivo devido a falha de leitura, diretório inacessível ou observação incompleta.
- Não presumir corrupção de conteúdo somente porque leitura ou hash falhou. O Analisador decidirá providências com evidências suficientes.
- Reconhecer que arquivos podem mudar durante a varredura; não exigir bloqueio do diretório e não implementar reconciliação de eventos.
- Reutilizar identidade lógica existente quando comprovada; não identificar recursos apenas por nome ou caminho se isso gerar ambiguidade.
- Respeitar escopo, symlinks, nomes não UTF-8 e exclusão do próprio diretório interno `.drivesync/`.
- Diagnósticos não persistidos por falta de API adequada no Inventário devem ser declarados **lacuna de integração**, nunca simulados como estado durável disponível ao Analisador.
- Não alterar interfaces de outros módulos como efeito colateral; registrar dependências de integração para revisão posterior.

## Testes

Exercitar os cenários `SC-01` a `SC-20` e `SC-S01` a `SC-S12` de [TESTS.md](TESTS.md), com filesystem e Inventário reais isolados, erros injetados, diretórios inacessíveis, arquivo desaparecendo e varredura repetida.

Testes de compilação e de integração Rust devem ser executados no Flatpak configurado pelo projeto. Reportar somente os resultados efetivamente obtidos, incluindo casos `BLOCKED` por lacunas de interface.

## Escopo documental

Esta etapa envolve **somente** `docs/engine/scanner/README.md`, `INTERFACES.md`, `TESTS.md` e `AGENTS.md`. Não modificar Scanner Rust, Inventário, Detectores, Orquestrador ou Analisador sem solicitação posterior.

**Estado:** especificação documental, sem código Rust implementado.
