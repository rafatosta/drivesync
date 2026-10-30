# Viabilidade técnica — resolução de conflitos e falhas

**Estado:** análise de arquitetura, não homologação. **Data:** 2026-10-27.  
**Base:** [checklist de validação](validation-checklist.md), [contratos do módulo](README.md) e [algoritmos](algorithms.md).  
**Tecnologias:** Rust/Linux, Google Drive API v3 e SQLite já adotados pelo DriveSync.

## Escopo e termos

- **Viável:** há primitivas suficientes para detectar/tratar conservadoramente; não significa implementado, automatizado ou testado.
- **Condicional:** precisa de histórico persistido, política aprovada, permissões ou interface; quando faltarem evidências, manter `UNKNOWN` e preservar.
- **Restrito:** nenhuma API/linguagem decide genericamente a intenção do usuário ou mescla qualquer formato.
- **Não marcar casos como concluídos** com base apenas nesta avaliação. Evidências e oráculos ficam no checklist.
- Todas as decisões dependem da comparação **B** (último estado *confirmado*), **L** (local) e **R** (remoto), incluindo identidade estável, revisão e fingerprint. O feed de mudanças do Drive não é um log completo de cada operação intermediária.

## Capacidades técnicas disponíveis

| Origem | Recursos úteis | Limites |
|---|---|---|
| Google Drive API v3 | `files.list/get`: `id`, `name`, `parents`, `mimeType`, `version`, `modifiedTime`, `md5Checksum` (quando disponível), `trashed`, `capabilities`; `changes.list` com cursor; transferências e upload resumível | `version` é revisão de metadados/arquivo remoto, não comparação direta com versão local; `md5Checksum` não vale para Docs nativos; `removed` também pode refletir perda de acesso; permissões/escopos limitam operações |
| Rust/Linux | `std::fs`, `std::io`, `rustix`, leitura estável, renomeação/arquivos temporários, hashes, observação local | inotify/notify não é diário durável offline; timestamps e nomes não identificam arquivos; operações em diferentes sistemas não são transações atômicas |
| Stack DriveSync | `reqwest`, `oauth2`, `rusqlite`, `serde`, `md-5`, `sha2`; inventário remoto, change feed, planejador conservador, checkpoints de download/upload | `conflicts` implementa a classificação pura SYNC-006, sem resolução completa; `planner` atual é orientado a download e não emite exclusão/sobrescrita remota; observações não equivalem sempre a baseline B confirmado |

**Garantia crucial:** reconsultar versões antes de efeitos destrutivos é necessário, porém reconsulta + operação posterior pode sofrer corrida. Verificar suporte de precondições HTTP/revisionais *por endpoint* e, quando inexistente, impedir sobrescrita sem proteção de ambas as versões e posterior reconciliação. Não supor compare-and-swap genérico da API.

## Mapeamento completo dos 44 casos

### Conflitos de conteúdo

| Caso | Evidência / mecanismo | Módulos responsáveis | Conclusão |
|---|---|---|---|
| C01 | B/L/R, Drive version/hash, fingerprint | changes, planner, state, conflicts | Viável; conteúdo divergente requer preservar/escolha |
| C02 | IDs remotos, inventário local e colisão de caminhos | remote, planner, conflicts | Viável; não inferir identidade pelo nome |
| C03 | Mudança local + remoto removido/indisponível | changes, state, conflicts | Viável; indisponível não autoriza apagar |
| C04 | Ausência local comprovada + versão remota nova | state, planner, conflicts | Viável; confirmar exclusão local real |
| C05 | ID remoto, identidade local, base | state, planner, conflicts | Viável com histórico de identidade |
| C06 | Checkpoint persistido e recaptura após offline | changes, state, planner | Viável com baseline confirmado |

### Conflitos de operações

| Caso | Evidência / mecanismo | Módulos responsáveis | Conclusão |
|---|---|---|---|
| C07 | IDs e histórico local de nomes | state, local, planner, conflicts | Condicional: sem histórico local, inferência ambígua |
| C08 | Histórico de rename e remoção | local, changes, conflicts | Viável; preservar antes de agir |
| C09 | Comparar alteração de nome e conteúdo separadamente | planner, state, conflicts | Viável com revalidação |
| C10 | Identidade persistente e parent IDs | state, remote, local, conflicts | Condicional: movimentos locais offline ambíguos |
| C11 | Parent IDs / ausência / base | changes, state, conflicts | Viável; sem exclusão automática |
| C12 | Parent IDs e fingerprints | planner, state, conflicts | Viável com revalidação |

### Estrutura e caminhos

| Caso | Evidência / mecanismo | Módulos responsáveis | Conclusão |
|---|---|---|---|
| C13 | Tipos local e MIME remotos | planner, remote | Viável; bloquear colisão |
| C14 | Mapa de destinos e identidade | planner, conflicts | Viável; reservar nome sem sobrescrever |
| C15 | Sensibilidade a caixa do filesystem | local, planner | Viável; testar no destino real |
| C16 | Inventário de descendentes e tombstones | changes, state, conflicts | Viável com baseline completo |
| C17 | Rastreamento de diretórios, IDs, pais | state, local, remote | Condicional: histórico local pode faltar |
| C18 | Validação de nomes, path limits e mapeamento | planner, local | Viável; mapping persistente |
| C19 | Consulta e revalidação do destino | remote, planner | Viável; fail closed |
| C20 | IDs diferentes e mesmo parent/name | remote, planner, conflicts | Viável; Drive permite nomes iguais |

### Metadados e tipos

| Caso | Evidência / mecanismo | Módulos responsáveis | Conclusão |
|---|---|---|---|
| C21 | Hashes disponíveis e fingerprints locais | remote, state, local | Viável; hash remoto não é universal |
| C22 | canDownload/canEdit e permissões locais | remote, local, planner | Condicional: modelos de permissão distintos |
| C23 | Modelo de metadados e compatibilidade | remote, local, state | Condicional: mapear só atributos suportados |
| C24 | mimeType e links/export quando permitido | remote, download, planner | Condicional: sem edição bidirecional universal de Workspace |
| C25 | changes.removed, trashed, consultas possíveis | changes, remote, state | Condicional: perda de acesso pode ser indistinguível |

### Falhas operacionais

| Caso | Evidência / mecanismo | Módulos responsáveis | Conclusão |
|---|---|---|---|
| E01 | Erros IO, leitura estável, atomicidade | local, download, upload | Viável; bloqueio advisory não é universal |
| E02 | ENOSPC, quota/storageQuota | local, remote, upload, download | Viável detectar e suspender; não aumentar espaço |
| E03 | std::io::Error e HTTP 403/capabilities | local, remote | Viável classificar; pode exigir usuário |
| E04 | reqwest, timeouts, retries/backoff | remote, upload, download | Viável; reconciliar efeitos ambíguos |
| E05 | staging/checkpoints/upload resumível | download, upload, state | Viável com recuperação idempotente |
| E06 | fingerprint e versão antes/depois | local, upload, download | Viável; revalidar antes de confirmar |
| E07 | oauth2/refresh token e erros Google | auth, remote | Viável; revogação exige nova autenticação |
| E08 | HTTP 429/403 reason e Retry-After | remote, coordinator | Viável; não contornar quotas |
| E09 | changes e files.get quando autorizado | changes, remote, state | Condicional; ambiguidade => UNKNOWN |
| E10 | md5/sha2, tamanho, checksum disponível | download, upload, state | Viável com hash local; nem todo remoto expõe MD5 |

### Políticas

| Caso | Evidência / mecanismo | Módulos responsáveis | Conclusão |
|---|---|---|---|
| P01 | Comparação sem divergência | planner, coordinator | Viável após base confirmada |
| P02 | Cópias/versionamento sem sobrescrita | conflicts, download, state | Viável; necessita armazenamento reservado |
| P03 | Decisão humana + revalidação | conflicts, state, UI | Viável; depende da interface e política |
| P04 | version não é relógio compartilhado; timestamps falíveis | planner, conflicts | Restrito; não adotar padrão last-write-wins |
| P05 | Algoritmo específico por formato | conflicts | Restrito; auto-merge genérico não disponível |
| P06 | Trashing/deletion + autorização/retention | remote, state, conflicts, UI | Condicional; exigir revisão de segurança |
| P07 | Persistência e independência por item | state, coordinator, conflicts | Viável com journal de pendências |
| P08 | Reserva de nomes + vínculos por ID | planner, conflicts, state | Viável |
| P09 | Auditoria e estado durável | state, coordinator | Viável |

## Cenários adicionais — N01–N12

| ID | Capacidade técnica e limite | Módulos envolvidos | Avaliação |
|---|---|---|---|
| N01 | Inventários iniciais + estado B ausente; não inferir equivalência por caminho | planner, state, changes | Viável; bloqueio conservador inicial |
| N02 | SQLite integrity check, arquivo de estado e fail-closed | state, coordinator | Viável; nunca tratar base perdida como exclusão |
| N03 | stat/mount e observação de raiz Linux | local, coordinator | Viável; diferenciar indisponível de vazio |
| N04 | files.get/changes e permissões, sem prova universal da causa | remote, changes, coordinator | Condicional; raiz UNKNOWN e sincronização suspensa |
| N05 | symlink_metadata, rustix e verificações por descritor | local, planner | Viável com política de links/hard links |
| N06 | Revalidação de fingerprint/revisão antes de efeito | conflicts, planner, coordinator | Viável; decisão antiga deve expirar |
| N07 | Escopo e geração de configuração persistidos | configuration, coordinator, state | Viável com cancelamento/replanejamento |
| N08 | Contagem/percentual por lote e política anti-exclusão | planner, conflicts, coordinator | Viável; limiares/UX a decidir |
| N09 | Lock por raiz e controle concorrente | coordinator, state | Viável; também considerar processo externo |
| N10 | Arquivo temporário, estabilidade de leitura e validação final | local, upload | Condicional; formatos temporários variam |
| N11 | Reconciliação após resultado HTTP ambíguo | remote, upload, state | Viável com identidade/idempotência; sem repetição cega |
| N12 | Baseline, revisões e histórico de restauração | state, planner, conflicts | Condicional; intenção nem sempre inferível |

**Interpretação:** 56 casos e políticas possuem estratégias investigáveis com a stack atual; nem todos admitem resolução automática. Viabilidade não comprova implementação, segurança operacional ou validação.

## Lacunas da implementação atual

1. Persistir baseline **B** validado por item, vinculado ao ID remoto, fingerprint local e revisão observada; distinguir observação de transferência concluída.
2. Inventariar e identificar alterações locais offline, criações, exclusões, renomeações e movimentos sem inferir identidade apenas por caminho/nome.
3. Definir registro durável de conflitos, reservas de nomes e política de retenção de cópias; idempotência após crash.
4. Implementar decisão bidirecional com operações explicitamente autorizadas; atualizar o executor em fase posterior, não por esta documentação.
5. Definir semântica e UX de **manter local**, **manter remoto**, **manter ambos** e pendências, com revalidação imediata e recuperação.
6. Revisar acesso OAuth, permissões e garantias de cada operação remota antes de habilitar atualização de existentes, lixo e exclusão.
7. Especificar oráculos por caso, com testes de falha entre efeito remoto e commit local, colisão, offline e perda de acesso.

## Ajustes transversais de arquitetura

- Distinguir catálogo de cenários (C/N), erros (E) e políticas (P); números de validação não representam funcionalidades independentes.
- Incluir proteção de exclusão/modificação em massa, disponibilidade da raiz e corrida de múltiplas instâncias.
- Separar cópia de conflito, histórico recuperável e journal de execução com retenção documentada.
- Atualizar status dos módulos a partir de código e testes: README genérico de rascunho não deve sobrepor implementações verificadas.

## Sequência recomendada

1. **Detecção:** baseline e matriz B/L/R; identidade, caminhos, nomes e hashes; oráculos puros.
2. **Preservação:** journal, cópias de conflito, exclusões suspensas e recuperação idempotente.
3. **Resolução assistida:** interface, escolhas explícitas, revalidação e persistência.
4. **Operações avançadas:** atualização remota de existentes, exclusões com proteção e políticas especiais; mesclagem por tipo apenas se houver algoritmo validado.

## Fontes primárias

- [Drive API — Files resource](https://developers.google.com/workspace/drive/api/reference/rest/v3/files)
- [Drive API — Changes resource](https://developers.google.com/workspace/drive/api/reference/rest/v3/changes)
- [Drive API — Track changes](https://developers.google.com/workspace/drive/api/guides/manage-changes)
- [Drive API — Resumable upload](https://developers.google.com/workspace/drive/api/guides/manage-uploads)
- [Drive API — Handle errors](https://developers.google.com/workspace/drive/api/guides/handle-errors)
- [Rust std::fs](https://doc.rust-lang.org/std/fs/index.html)

**Resultado:** 56 itens catalogados possuem caminhos de investigação e tratamento conservador na stack atual, **não** 56 resoluções automáticas. Alguns resultados corretos são bloquear, preservar, solicitar decisão ou informar falha persistente. **Nenhum item do checklist foi marcado como validado.**
