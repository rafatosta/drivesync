# Checklist de validação — conflitos e falhas de sincronização

**Estado:** catálogo revisado; cenários não validados. **Total:** 56 itens rastreáveis (C01–C25, E01–E10, P01–P09, N01–N12).  
**Escopo:** detecção, classificação, preservação e resolução de divergências na sincronização bidirecional DriveSync.  
**Responsáveis funcionais:** `conflicts` (classificação/política), `planner` (decisão de operações), `state` (baseline, identidades e persistência), `coordinator` (execução/recuperação), adaptadores de arquivos/API e interface (resolução assistida).  
**Referências locais:** [README](README.md), [algoritmos](algorithms.md), [análise de viabilidade técnica](technical-feasibility.md) e [pesquisa conceitual de UX](ux-research.md). Esta lista é um **plano de validação**, não evidência de suporte atual.

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

## Classificação para implementação e interface

- **Conflito real:** alterações concorrentes incompatíveis com versões identificáveis (ex.: C01, C03, C04).
- **Diferença conciliável/equivalência:** operações independentes ou conteúdo equivalente não devem produzir conflito visível (ex.: C09, C12, C21).
- **Incompatibilidade:** nomes, tipos, propriedades ou mapeamentos não representáveis devem ser bloqueados ou explicados sem escolher versão arbitrariamente (ex.: C13, C18, C23, C24).
- **Indisponibilidade/erro:** permissões, autenticação, transporte ou estado remoto desconhecido não provam exclusão (C25, E01–E10).
- Os códigos históricos C permanecem para rastreabilidade, mesmo quando não representam conflito de conteúdo.

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

## Cenários complementares de segurança e ciclo de vida

- [ ] **N01** — Primeira sincronização com arquivos preexistentes em ambos os lados: não inventar baseline confirmado nem sobrescrever.
- [ ] **N02** — Banco local perdido, ilegível ou corrompido: impedir inferência de exclusão e preservar dados.
- [ ] **N03** — Raiz local desmontada/inacessível: suspender operações, sem propagar exclusões.
- [ ] **N04** — Raiz remota movida, excluída ou sem acesso: distinguir indisponibilidade e não executar ações destrutivas.
- [ ] **N05** — Symlinks, hard links e arquivos especiais: definir suporte seguro, isolamento da raiz e prevenção de travessia.
- [ ] **N06** — Arquivo muda enquanto o usuário compara ou confirma: invalidar decisão obsoleta e revalidar.
- [ ] **N07** — Preferências/escopo alterados durante transferência: cancelar, reconciliar ou continuar somente sob regras explícitas.
- [ ] **N08** — Exclusões/modificações em massa: detectar impacto, suspender ações potencialmente destrutivas e exigir política/consentimento.
- [ ] **N09** — Duas instâncias/atores atuando sobre a mesma raiz local: bloqueio por escopo e proteção contra corrida.
- [ ] **N10** — Arquivo parcial, temporário ou ainda gravado: não publicá-lo como versão definitiva.
- [ ] **N11** — Resultado remoto desconhecido após timeout/crash: reconciliar por identidade antes de repetir operação.
- [ ] **N12** — Restauração de backup/versão histórica: distinguir alteração legítima de inconsistência e preservar histórico.

**Prioridade para avaliação inicial:** N01, N02, N03, N06, N08, N09 e N11. Todo N permanece pendente até teste com evidência.

## Salvaguardas transversais

- [ ] Separar cópia de conflito, histórico de versões e journal/auditoria; especificar local, retenção, limpeza e quota sem perda silenciosa.
- [ ] Bloquear operações destrutivas em lote e apresentar quantidade, percentual, escopo e consequência; limiares a definir em requisito.
- [ ] Não permitir política automática sobre exclusões, raízes indisponíveis, fontes ambíguas ou revisões desatualizadas.
- [ ] Verificar bloqueios por raiz, symlinks e corridas entre verificação e escrita, inclusive no Linux/Flatpak.
- [ ] UI deve distinguir conflito, incompatibilidade e erro operacional, indicando versões realmente preservadas e explicando ações irreversíveis.
- [ ] Casos de teste devem cobrir acessibilidade, decisões adiadas, reabertura, mudanças durante resolução e lote.

## Procedimento de teste por caso

Para cada `Cxx`, `Exx` ou `Pxx`:
1. Registrar identidade do item, caminho local/remoto, baseline B, estados L/R, revisões e hashes quando aplicáveis.
2. Executar o cenário controlado (incluindo concorrência e offline).
3. Comparar com o oráculo: operação permitida, conflito pendente, cópias íntegras e nenhum dado perdido.
4. Repetir com falha antes/depois do efeito externo e reiniciar a aplicação.
5. Verificar idempotência, status da interface e persistência de estado.
6. Anexar referência do teste/issue/commit e só então marcar o item concluído.

**Observação:** as ações de atualização de arquivos remotos existentes e de propagação de exclusões não devem ser consideradas implementadas ou aprovadas por constarem neste documento. Esta lista não modifica a política atual do MVP.
