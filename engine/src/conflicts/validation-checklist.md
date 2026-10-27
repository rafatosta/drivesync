# Checklist de validação — conflitos e falhas de sincronização

**Estado:** proposta para revisão; cenários não validados.  
**Escopo:** detecção, classificação, preservação e resolução de divergências na sincronização bidirecional DriveSync.  
**Responsáveis funcionais:** `conflicts` (classificação/política), `planner` (decisão de operações), `state` (baseline, identidades e persistência), `coordinator` (execução/recuperação), adaptadores de arquivos/API e interface (resolução assistida).  
**Referências locais:** [README](README.md), [algoritmos](algorithms.md) e [análise de viabilidade técnica](technical-feasibility.md). Esta lista é um **plano de validação**, não evidência de suporte atual.

## Critério de conclusão

- [ ] Para cada caso, preparar fixture com estado base **B** (última sincronização confirmada), estado local **L** e remoto **R**, identidades, revisões e operações concorrentes.
- [ ] Documentar pré-condições, evento, resultado esperado, artefatos preservados, erro tipado e comportamento após reinício.
- [ ] Implementar teste automatizado determinístico para cada cenário pertinente e teste de integração para efeitos externos.
- [ ] Demonstrar preservação de dados e idempotência (repetir não deve duplicar, perder ou sobrescrever versões).
- [ ] Simular interrupção entre escrita externa, persistência de journal/checkpoint e confirmação.
- [ ] Confirmar que `UNKNOWN`, timeout e perda de permissão **não** implicam exclusão.
- [ ] Validar novamente as versões antes de qualquer resolução destrutiva; não sobrescrever conteúdo divergente sem preservação.
- [ ] Registrar, para cada caso, referência ao teste, resultado e decisão aprovada. **Só marcar [x] com evidência.**

## Matriz de diferenças básicas (B / L / R)

Considerar 'inalterado', 'modificado' e 'excluído' sempre **em relação a B**, não apenas comparando timestamps.

| Local | Remoto | Resultado esperado a validar |
|---|---|---|
| Inalterado | Inalterado | Sem ação |
| Modificado | Inalterado | Envio local, se permitido pela política e versão revalidada |
| Inalterado | Modificado | Recebimento remoto seguro |
| Excluído | Inalterado | **Não propagar exclusão automaticamente** enquanto política destrutiva não for aprovada |
| Inalterado | Excluído | Preservar conteúdo local, distinguir exclusão de perda de acesso e seguir política |
| Modificado | Modificado | Se conteúdo divergir, conflito; se equivalente, reconciliar sem duplicação |
| Modificado | Excluído | Conflito: preservar alteração local |
| Excluído | Modificado | Conflito: preservar alteração remota |
| Excluído | Excluído | Convergência confirmada; nenhuma exclusão adicional |

Criação sem B, renomeação, movimentação e substituição precisam de análise por **identidade estável** e eventos; nome/caminho não identifica sozinho um arquivo. Um arquivo remoto pode ter nome duplicado e representações locais podem ser incompatíveis.

## Casos de conflito de conteúdo

- [ ] **C01** — Mesmo arquivo modificado local e remotamente, com conteúdos divergentes.
- [ ] **C02** — Dois arquivos distintos criados simultaneamente com o mesmo caminho/nome.
- [ ] **C03** — Arquivo modificado localmente e excluído remotamente.
- [ ] **C04** — Arquivo excluído localmente e modificado remotamente.
- [ ] **C05** — Arquivo substituído por outro de mesmo nome, identidade diferente.
- [ ] **C06** — Versões divergentes após período offline; detectar usando baseline e versões, não apenas data.

## Casos de operações concorrentes

- [ ] **C07** — Mesmo arquivo renomeado para nomes distintos nos dois lados.
- [ ] **C08** — Renomeação de um lado versus exclusão do outro.
- [ ] **C09** — Renomeação de um lado versus alteração de conteúdo do outro; combinar quando seguro.
- [ ] **C10** — Mesmo arquivo movido para destinos distintos nos dois lados.
- [ ] **C11** — Movimentação versus exclusão.
- [ ] **C12** — Movimentação versus modificação; combinar quando identidade e condições permitirem.

## Casos de estrutura e caminhos

- [ ] **C13** — Arquivo local versus diretório remoto com o mesmo caminho (ou inverso).
- [ ] **C14** — Colisão de nomes/caminhos de identidades diferentes.
- [ ] **C15** — Nomes que diferem apenas por caixa em filesystem sem distinção de caixa.
- [ ] **C16** — Diretório excluído enquanto descendentes foram alterados no outro lado.
- [ ] **C17** — Mesmo diretório movido simultaneamente para destinos distintos.
- [ ] **C18** — Caracteres, nomes reservados ou comprimento de caminho incompatíveis no destino.
- [ ] **C19** — Destino da movimentação deixou de existir ou mudou de identidade.
- [ ] **C20** — Dois itens distintos com o mesmo nome na mesma pasta remota do Google Drive.

## Casos de metadados e tipos

- [ ] **C21** — Timestamps divergentes com conteúdo idêntico; não criar conflito falso.
- [ ] **C22** — Divergência de permissões de leitura/escrita entre lados.
- [ ] **C23** — Metadados/atributos não representáveis de forma simétrica.
- [ ] **C24** — Arquivo local comum versus documento nativo Google Workspace; respeitar suporte/limitações explícitos.
- [ ] **C25** — Mudança de compartilhamento/perda de acesso remoto; não interpretar como exclusão.

## Erros operacionais (não são necessariamente conflitos)

- [ ] **E01** — Arquivo bloqueado/em uso.
- [ ] **E02** — Espaço insuficiente no disco ou na cota remota.
- [ ] **E03** — Permissão negada em arquivo, diretório ou API.
- [ ] **E04** — Queda de rede durante transferência.
- [ ] **E05** — Transferência interrompida/parcial; retomar ou descartar staging com segurança.
- [ ] **E06** — Arquivo alterado durante upload/download; revalidar versão.
- [ ] **E07** — Token OAuth inválido, expirado ou revogado; distinguir renovação possível.
- [ ] **E08** — Limite de taxa/cota operacional da API; backoff/retry controlado.
- [ ] **E09** — Arquivo remoto indisponível, removido ou inacessível; distinguir causas.
- [ ] **E10** — Integridade divergente após transferência; não confirmar estado incorreto.

## Políticas a especificar e validar

- [ ] **P01** — Sem divergência: sincronização automática.
- [ ] **P02** — Divergência com ambas as versões válidas: preservar as duas e registrar conflito sem sobrescrita silenciosa (`CONFLICT-001`).
- [ ] **P03** — Resolução assistida: manter local / remoto / ambas, com revalidação antes de aplicar (`CONFLICT-002`).
- [ ] **P04** — Prioridade por versão mais recente: **não assumir que timestamp implica precedência confiável**; decidir se política será suportada e em quais condições.
- [ ] **P05** — Mesclagem automática: somente se formato e algoritmo confiável forem definidos; **fora do escopo inicial**.
- [ ] **P06** — Exclusões locais/remotas, inclusive em lote: definir autorização, retenção, recuperação e prevenção de perda antes de habilitar propagação destrutiva.
- [ ] **P07** — Conflito pendente não bloqueia itens independentes; pendência persiste após fechamento/reinício.
- [ ] **P08** — Nomes reservados para cópias preservadas, sem colisão; vínculo entre cópias, identidade original e origem.
- [ ] **P09** — Auditoria das decisões: ação, versões observadas, efeitos parciais, estado final e recuperação.

## Procedimento de teste por caso

Para cada `Cxx`, `Exx` ou `Pxx`:
1. Registrar identidade do item, caminho local/remoto, baseline B, estados L/R, revisões e hashes quando aplicáveis.
2. Executar o cenário controlado (incluindo concorrência e offline).
3. Comparar com o oráculo: operação permitida, conflito pendente, cópias íntegras e nenhum dado perdido.
4. Repetir com falha antes/depois do efeito externo e reiniciar a aplicação.
5. Verificar idempotência, status da interface e persistência de estado.
6. Anexar referência do teste/issue/commit e só então marcar o item concluído.

**Observação:** as ações de atualização de arquivos remotos existentes e de propagação de exclusões não devem ser consideradas implementadas ou aprovadas por constarem neste documento. Esta lista não modifica a política atual do MVP.
