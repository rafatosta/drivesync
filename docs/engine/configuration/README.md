# Configuration — preferências e recuperação de bancos

## Objetivo

`configuration/` disponibiliza **substituições opcionais dos parâmetros operacionais** e o **mecanismo comum de backup/restauração de bancos SQLite registrados**. Não executa detecção, análise, transferências ou sincronização e não toma decisões de conflitos.

O módulo permanece auxiliar para preferências: todos os módulos funcionam com seus próprios valores padrão quando não houver preferência salva. A infraestrutura comum de backup/recuperação pode ser utilizada por qualquer Storage SQLite registrado, mas a capacidade de registrar e restaurar um banco **não transforma Configuration em proprietário do banco**.

## Primeira versão: parâmetros aceitos

| Chave conceitual | Consumidor | Escopo inicial | Valor quando ausente |
| --- | --- | --- | --- |
| `detector.remote.poll_interval` | Detector Remoto | Por sincronização | Padrão do detector |
| `detector.remote.page_budget` | Detector Remoto | Por sincronização | Padrão do detector |
| `detector.local.debounce` | Detector Local | Por sincronização | Padrão do detector |
| `detector.event_batch_size` | Detector, ao preparar entrega | Por sincronização | Padrão do Detector/consumidor |
| `network.request_timeout` | Transporte HTTP, quando suportado | Global | Padrão do transporte |
| `network.retry_delay` | Consumidor responsável por uma tentativa segura | Global | Padrão do consumidor, respeitando `Retry-After` |

As chaves são **nomes documentais propostos**, não um esquema GSettings já criado. Os limites exatos e unidades devem ser fechados nos tipos e testes antes da implementação. Valores inválidos são recusados, não aplicados silenciosamente. `retry_delay` **não autoriza retry de escrita com resultado incerto** e não implementa uma fila; o responsável pela operação decide se pode tentar novamente. O timeout HTTP não elimina limites específicos de endpoints ou transferências longas.

**Não incluir agora:** uploads/downloads/análises simultâneos, quantidade de threads ou workers, prioridades globais, limitação de velocidade, tamanho de chunks, limite de download por tamanho, políticas futuras de exclusão e outros recursos ainda sem decisão. Esses ajustes não condicionam a independência das contas ou o desenvolvimento dos contratos dos módulos.

## Resolução e aplicação

1. Uma configuração específica de `sync_id` prevalece sobre a global, **quando a chave admitir esse escopo**.
2. Na ausência de preferência, usa-se o valor padrão mantido pelo módulo consumidor.
3. A validação é tipada (durations, inteiros positivos e faixas permitidas), antes de entregar o valor.
4. Configurações de observadores podem ser reaplicadas na próxima rodada segura; parâmetros por chamada são capturados no início da operação. A efetiva atualização dinâmica depende do suporte daquele módulo.
5. O módulo não reinicia observadores, não cancela operações nem modifica módulos para aplicar uma preferência.

**Por conta:** não criar ainda um terceiro nível de escopo. O isolamento e eventual compartilhamento de instâncias entre contas serão decididos na próxima etapa da arquitetura. Escopo de sincronização é identificado por `SyncId`, nunca por caminho como chave principal.

## Recuperação SQLite centralizada

### Divisão de responsabilidades

- **Configuration / recuperação SQLite:** registra **referências autorizadas** dos bancos; cria snapshots consistentes com os mecanismos oficiais do SQLite; seleciona, valida tecnicamente e restaura cópias; registra resultado `Normal/Restored/RecoveryFailed`, sem interpretar tabelas de domínio.
- **Storage proprietário (Inventory, Detector ou outro):** decide onde cria seu banco e registra um descritor restrito em Configuration; mantém conexões, transações, identidade, schema e integridade de domínio; diagnostica corrupção; coordena seu fechamento/suspensão segura quando a restauração é necessária; valida identidade e formato após a restauração.
- **Coordenação de inicialização:** toma decisões sobre reobservação, recuperação do estado lógico e sincronização depois que o Storage confirmar a reabertura. O mecanismo de restauração não supõe que backup de banco restaure mudanças já realizadas no filesystem ou no Drive.

Configuration **não descobre bancos procurando arquivos no disco**, não recebe conexões de consulta pública nem altera tabelas do Inventário ou da fila. O caminho físico registrado é metadado restrito ao subsistema de recuperação, não uma preferência exportada para a GUI. Um banco que não foi registrado não é restaurado por adivinhação.

### Criar um backup seguro

Usar preferencialmente a **Online Backup API do SQLite** para copiar um snapshot consistente de uma base que possa estar em uso. `VACUUM INTO` é alternativa possível quando apropriada, após validação técnica. **Não copiar simplesmente o arquivo `.sqlite3` ativo**, pois o WAL/journal pode conter dados relevantes.

1. Receber registro do banco, com identificador/identidade opacos, caminho autorizado e política de acesso.
2. Criar snapshot SQLite em destino temporário privado, sem compartilhar uma conexão de destino durante a cópia.
3. Validar integridade técnica e identidade registrada; concluir e publicar o backup de forma segura, com permissões restritas.
4. Conservar um backup anterior válido se a nova cópia ou a validação falhar.
5. Não incluir credenciais do keyring em backups de bancos.

### Restaurar sem perda silenciosa

1. Diante de corrupção **comprovada**, o Storage sinaliza indisponibilidade e solicita recuperação por identidade registrada; erros de permissão, `SQLITE_BUSY`, disco cheio, esquema incompatível ou identidade errada **não** autorizam restauração automática.
2. Garantir exclusividade e ausência de conexões/transações ativas antes de trocar um banco, inclusive WAL/journal. Se não houver forma segura de interromper acessos, retornar erro e **não** substituir o banco.
3. Preservar o conjunto danificado (incluindo WAL/journal relevantes) para diagnóstico e possível recuperação manual.
4. Selecionar somente snapshot previamente validado para **a mesma identidade** e restaurar em local temporário antes da troca segura; verificar integridade técnica novamente.
5. O Storage proprietário reabre, confere identidade, versão de esquema e invariantes próprios. Apenas depois disso o resultado é `Restored`.
6. Caso não haja backup adequado ou a reabertura falhe, informar `RecoveryFailed`, sem descartar automaticamente o banco anterior. Uma **reconstrução vazia**, se desejada, é fluxo separado e explicitamente autorizado pela coordenação, utilizando a rotina de inicialização de schema do Storage — não é restauração.

A implementação concreta da exclusividade, publicação atômica, permissões Flatpak, WAL, durabilidade, local de backups e notificação ao usuário ainda precisa ser definida e testada. A política de snapshots e retenção inicialmente utiliza defaults do próprio mecanismo comum, **sem criar chaves de preferência adicionais**. O usuário não precisa configurar o mecanismo para a recuperação existir.

## Organização e API

A proposta de interface, os cenários de teste e as regras de implementação estão em [INTERFACES.md](INTERFACES.md), [TESTS.md](TESTS.md) e [AGENTS.md](AGENTS.md). A persistência das preferências poderá usar GSettings ou mecanismo equivalente após validação; a recuperação SQLite não exige que todos os bancos compartilhem o mesmo armazenamento.

**Estado:** planejamento documental; não há comprovação de implementação Rust nem de testes realizados.
