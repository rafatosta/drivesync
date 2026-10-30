# Matriz numerada de cenários de sincronização — DriveSync

> **Estado:** especificação proposta, não evidência de implementação ou aceite. Esta numeração `SYNC-001` a `SYNC-054` identifica **cenários documentais**, não as issues GitHub homônimas (por exemplo, issue SYNC-001). O catálogo legado de 56 casos `C/E/P/N` em [validation-checklist.md](../../engine/src/conflicts/validation-checklist.md) permanece vigente; a correspondência entre catálogos será auditada separadamente.

## Convenções

- **B:** último estado-base *confirmado*, ou ausência explícita de B. **L/R:** observações local/remota.
- `ausente` é ausência confirmada; `UNKNOWN`, erro, falta de acesso ou desconexão **não** comprovam exclusão.
- Classificação não executa escrita. Todo efeito exige autorização, revalidação de identidade/revisão e persistência durável apropriada.
- Uma ação **proposta** não implica que exista executor ou teste aprovado. Operações destrutivas e sobrescritas divergentes permanecem bloqueadas até política e salvaguardas aprovadas.
- Cada caso possui fluxograma em [flowcharts.md](flowcharts.md). Evidências e lacunas ficam em [verification.md](verification.md).

## Estados B/L/R

| ID | Cenário | Pré-condição / evidência | Ação esperada (proposta) | Requisito |
|---|---|---|---|---|
| [SYNC-001](flowcharts.md#sync-001) | Sem alterações | B confirmado; L e R iguais ao estado-base | Não transferir; conservar B | RF-CON-01 |
| [SYNC-002](flowcharts.md#sync-002) | Somente local modificado | B confirmado; L mudou, R não | Planejar upload apenas se autorizado e com precondição remota | RF-SYNC-03 |
| [SYNC-003](flowcharts.md#sync-003) | Somente remoto modificado | B confirmado; R mudou, L não | Planejar download protegido e revalidar R | RF-SYNC-04 |
| [SYNC-004](flowcharts.md#sync-004) | Ambos modificados, divergentes | B confirmado; conteúdos diferentes | Bloquear sobrescrita, preservar versões e pedir decisão | RF-CON-03 |
| [SYNC-005](flowcharts.md#sync-005) | Ambos modificados, equivalentes | B confirmado; conteúdos equivalentes com evidência forte | Reconciliar sem duplicar e confirmar B | RF-CON-01 |
| [SYNC-006](flowcharts.md#sync-006) | Local excluído, remoto intacto | B confirmado; ausência local comprovada | Suspender exclusão remota até autorização específica | RF-CON-04 |
| [SYNC-007](flowcharts.md#sync-007) | Local intacto, remoto excluído | B confirmado; exclusão remota comprovada | Preservar local e aplicar política de exclusão autorizada | RF-CON-04 |
| [SYNC-008](flowcharts.md#sync-008) | Local excluído, remoto modificado | B confirmado; R alterado | Preservar R e solicitar resolução | RF-CON-03 |
| [SYNC-009](flowcharts.md#sync-009) | Local modificado, remoto excluído | B confirmado; L alterado | Preservar L e solicitar resolução | RF-CON-03 |
| [SYNC-010](flowcharts.md#sync-010) | Ambos excluídos | B confirmado; duas ausências comprovadas | Registrar convergência sem nova exclusão | RF-CON-01 |
| [SYNC-011](flowcharts.md#sync-011) | Edição e renomeação locais | Identidade mantida; R sem mudanças | Validar destino e planejar operações autorizadas | RF-CON-02 |
| [SYNC-012](flowcharts.md#sync-012) | Edição e renomeação remotas | Identidade mantida; L sem mudanças | Validar nome/destino e receber alterações seguras | RF-CON-02 |
| [SYNC-013](flowcharts.md#sync-013) | Renomeações divergentes | Mesmo ID, nomes diferentes | Bloquear escolha arbitrária e solicitar resolução | RF-CON-02 |
| [SYNC-014](flowcharts.md#sync-014) | Edição local e renomeação remota | Operações independentes e identidades comprovadas | Combinar somente se seguro; senão pendência | RF-CON-02 |
| [SYNC-015](flowcharts.md#sync-015) | Renomeação local e edição remota | Operações independentes e identidades comprovadas | Combinar somente se seguro; senão pendência | RF-CON-02 |

## Criação, identidade e estrutura

| ID | Cenário | Pré-condição / evidência | Ação esperada (proposta) | Requisito |
|---|---|---|---|---|
| [SYNC-016](flowcharts.md#sync-016) | Novo local, remoto ausente | Sem B; ausência remota comprovada | Criar remotamente apenas sob política de upload | RF-CON-01 |
| [SYNC-017](flowcharts.md#sync-017) | Novo remoto, local ausente | Sem B; ausência local comprovada | Baixar sem sobrescrever arquivo preexistente | RF-SYNC-02 |
| [SYNC-018](flowcharts.md#sync-018) | Criações simultâneas equivalentes | Sem B; dois objetos e mesmo nome/conteúdo | Não presumir mesma identidade; reconciliar apenas com evidência suficiente | RF-CON-02 |
| [SYNC-019](flowcharts.md#sync-019) | Criações simultâneas divergentes | Sem B; mesmos nomes, bytes diferentes | Preservar dois objetos e bloquear colisão | RF-CON-02 |
| [SYNC-020](flowcharts.md#sync-020) | Nomes remotos duplicados | Dois IDs remotos sob mesmo pai e nome | Mapear identidades sem sobrescrever no filesystem | RF-CON-02 |
| [SYNC-021](flowcharts.md#sync-021) | Arquivo versus pasta | Tipos diferentes ocupam mesmo caminho | Bloquear e informar incompatibilidade | RF-CON-02 |
| [SYNC-022](flowcharts.md#sync-022) | Recriação após exclusão | Nova identidade surge após tombstone | Tratar como nova criação; não herdar B pelo nome | RF-SYNC-01 |
| [SYNC-023](flowcharts.md#sync-023) | Movimentações concorrentes | Mesmo item movido para destinos diferentes | Comparar IDs e pais; bloquear destinos incompatíveis | RF-CON-02 |
| [SYNC-024](flowcharts.md#sync-024) | Renomeação de pasta e edições descendentes | Alterações de árvore e descendentes | Reconstruir dependências antes de mover ou atualizar | RF-CON-02 |

## Conectividade e offline

| ID | Cenário | Pré-condição / evidência | Ação esperada (proposta) | Requisito |
|---|---|---|---|---|
| [SYNC-025](flowcharts.md#sync-025) | Offline sem mudança local | Sem acesso remoto | Aguardar sem inferir estado remoto | RF-CON-01 |
| [SYNC-026](flowcharts.md#sync-026) | Offline com edição local | Mudança local observada; R desconhecido | Registrar pendência, sem upload até revalidar | RF-CON-01 |
| [SYNC-027](flowcharts.md#sync-027) | Offline com criação local | Novo L, R desconhecido | Registrar intenção e reavaliar ao reconectar | RF-CON-01 |
| [SYNC-028](flowcharts.md#sync-028) | Offline com exclusão local | Ausência L, R desconhecido | Registrar intenção; nenhuma exclusão remota | RF-CON-04 |
| [SYNC-029](flowcharts.md#sync-029) | Reconexão; só local mudou | R consultado e igual a B | Planejar envio conforme política, revalidando | RF-CON-01 |
| [SYNC-030](flowcharts.md#sync-030) | Reconexão; só remoto mudou | L igual a B, R diferente | Planejar download seguro | RF-CON-01 |
| [SYNC-031](flowcharts.md#sync-031) | Reconexão; ambos mudaram | L e R diferem de B | Classificar divergência ou equivalência; preservar | RF-CON-01 |
| [SYNC-032](flowcharts.md#sync-032) | Reconexão; edição local e exclusão remota | L editado; tombstone remoto confirmado | Proteger L; pedir decisão | RF-CON-03 |
| [SYNC-033](flowcharts.md#sync-033) | Queda durante upload | Transferência de envio interrompida | Persistir intenção; reconciliar efeito antes de repetir | RF-REC-01 |
| [SYNC-034](flowcharts.md#sync-034) | Queda durante download | Download incompleto | Manter original; validar temporário antes de instalar | RF-REC-01 |
| [SYNC-035](flowcharts.md#sync-035) | Rede intermitente | Erros transitórios repetidos | Backoff e tentativas limitadas/idempotentes | RF-REC-01 |
| [SYNC-036](flowcharts.md#sync-036) | Remoto indisponível/UNKNOWN | Consulta não conclui estado de R | Não converter erro em exclusão; suspender ações dependentes | RF-CON-01 |

## Falhas e condições especiais

| ID | Cenário | Pré-condição / evidência | Ação esperada (proposta) | Requisito |
|---|---|---|---|---|
| [SYNC-037](flowcharts.md#sync-037) | Disco local cheio | Sem capacidade para baixar/preservar | Abortar instalação, conservar original e registrar erro | RF-REC-01 |
| [SYNC-038](flowcharts.md#sync-038) | Quota remota excedida | Upload impedido por cota | Manter L, suspender envio e informar | RF-REC-01 |
| [SYNC-039](flowcharts.md#sync-039) | Permissão local negada | I/O local bloqueado | Não modificar caminho; registrar impedimento | RF-REC-01 |
| [SYNC-040](flowcharts.md#sync-040) | Permissão remota revogada | Acesso R negado | Não inferir exclusão; solicitar correção de acesso | RF-CON-01 |
| [SYNC-041](flowcharts.md#sync-041) | Credencial expirada | Token inválido/expirado | Renovar se possível; caso contrário reconectar | RF-REC-01 |
| [SYNC-042](flowcharts.md#sync-042) | API limita requisições | 429 ou erro transitório equivalente | Aplicar backoff e retomar com revalidação | RF-REC-01 |
| [SYNC-043](flowcharts.md#sync-043) | Encerramento durante transferência | Operação interrompida antes de confirmar | Recuperar journal e reconciliar antes de repetir | RF-REC-01 |
| [SYNC-044](flowcharts.md#sync-044) | Edição local durante upload | Snapshot local muda antes da confirmação | Não afirmar B antigo como atual; replanejar | RF-SYNC-03 |
| [SYNC-045](flowcharts.md#sync-045) | Edição remota durante download | Revisão R muda durante recebimento | Revalidar revisão, interromper instalação obsoleta | RF-SYNC-04 |
| [SYNC-046](flowcharts.md#sync-046) | Efeito remoto incerto | Timeout após possível sucesso | Consultar resultado por ID/intenção antes de retry | RF-REC-01 |
| [SYNC-047](flowcharts.md#sync-047) | Arquivo temporário em gravação | Arquivo ainda incompleto/volátil | Aguardar estabilidade e checar identidade | RF-CON-02 |
| [SYNC-048](flowcharts.md#sync-048) | Symlink fora da raiz | Link aponta fora do escopo autorizado | Não seguir; registrar bloqueio | RF-CON-02 |
| [SYNC-049](flowcharts.md#sync-049) | Nome/caminho incompatível | Destino não representa nome ou tipo | Suspender item; oferecer tratamento explícito | RF-CON-02 |
| [SYNC-050](flowcharts.md#sync-050) | Exclusão em massa | Quantidade anômala de remoções | Bloquear lote e exigir confirmação detalhada | RF-CON-04 |
| [SYNC-051](flowcharts.md#sync-051) | Instâncias concorrentes | Duas execuções sobre mesma raiz | Usar lock e proteger estado persistente | RF-REC-01 |
| [SYNC-052](flowcharts.md#sync-052) | Mudança de raiz durante execução | Configuração alterada | Invalidar plano, cancelar ou replanejar | RF-CON-02 |
| [SYNC-053](flowcharts.md#sync-053) | Documento Google Workspace | Arquivo nativo sem binário equivalente | Usar estratégia específica de link/exportação; não presumir bytes | RF-CON-02 |
| [SYNC-054](flowcharts.md#sync-054) | Metadados incompatíveis | Permissões/atributos não representáveis | Preservar conteúdo, registrar limitação de metadados | RF-CON-02 |

## Critérios de aceite transversais

1. Registrar B/L/R, conta, raiz, ID remoto, fingerprint e revisão quando disponíveis.
2. Testar classificação sem I/O e efeitos com provedor remoto simulado, filesystem e SQLite reais isolados sempre que possível.
3. Simular reabertura, interrupção, repetição e alterações concorrentes; verificar bytes preservados e efeitos proibidos.
4. Quando a simulação não cobrir requisito externo, declarar claramente o ensaio real pendente, sem tratá-lo como aprovado.
5. Antes de marcar um cenário validado, confrontar cada ramo do diagrama com código, teste, resultado e política vigente.
