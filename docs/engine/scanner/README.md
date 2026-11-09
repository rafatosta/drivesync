# Scanner Local

## Objetivo

Reconstruir as **observações do estado local** dos arquivos e diretórios de uma sincronização e registrá-las no [Inventário](../inventory/INTERFACES.md). O Scanner percorre o filesystem, identifica o que conseguiu observar, preserva incertezas e **termina o levantamento mesmo diante de erros individuais**.

É um módulo independente do [Detector Local](../detectors/local/README.md), do [Detector Remoto](../detectors/remote/README.md) e do [Analisador](../analyzer/README.md). Não recebe, consome ou publica eventos dos Detectores, não acessa a API de provedores, não decide conflitos e não executa operações de sincronização.

## Uso na inicialização

1. O Orquestrador inicia os Detectores, que acumulam observações em suas **próprias filas persistentes**, mantendo suspenso o consumo dessas filas.
2. O Orquestrador inicia o Scanner para a raiz local configurada.
3. O Scanner percorre diretórios e arquivos locais e atualiza os estados locais do Inventário, **sem alterar baseline ou estados remotos**.
4. Mesmo se encontrar entradas inacessíveis, continua nos demais ramos e registra as limitações observadas sem declarar ausências indevidas.
5. Ao terminar, devolve um `ScanReport` informando conclusão do percurso, cobertura e problemas. Erros de um item **não invalidam automaticamente a varredura**.
6. O Orquestrador decide se pode retomar o processamento dos eventos e itens. O Analisador, posteriormente, trata cada `SyncItem` e suas inconsistências pelo Reconciliador e pelo módulo de Conflitos.

O Scanner **não entrega itens diretamente ao Analisador**. Seu produto é o Inventário local atualizado e um relatório resumido para o Orquestrador. O Detector não grava diretamente no Inventário por causa desta etapa.

## Semântica do levantamento

- Um recurso **presente** e acessível é observado com seus metadados reais disponíveis.
- Um recurso **presente, mas não legível integralmente**, pode ter presença conhecida e atributos indisponíveis. Isso não comprova corrupção.
- Uma entrada que desapareceu durante a leitura pode ser registrada como observação inconclusiva, sem tentar reconciliá-la.
- Uma entrada que não apareceu em uma **varredura comprovadamente completa de seu escopo**, e que constava anteriormente, pode receber `AbsentConfirmed` com prova adequada.
- Se uma subárvore não pôde ser percorrida, seus descendentes ficam desconhecidos/sem nova confirmação: não declará-los excluídos.
- Alterações simultâneas feitas pelo usuário são normais. O Scanner registra o que observou e não precisa conciliar sua ordem com eventos detectados.
- Metadados ou conteúdo não coletados não são substituídos por estimativas ou hashes inventados.

**Conclusão do percurso é diferente de integridade dos arquivos.** Uma varredura pode terminar com problemas por item registrados. A resolução das diferenças e a escolha entre download, upload ou intervenção do usuário pertencem ao Analisador.

## Componentes envolvidos

| Componente | Contrato com o Scanner |
| --- | --- |
| Orquestrador | Inicia `scan` e recebe o relatório; suspende e retoma o consumo das filas dos Detectores |
| Inventário | Recebe as observações locais por métodos públicos; mantém vínculos, revisão, baseline e dados remotos |
| Filesystem local | Fonte única dos arquivos, diretórios e metadados observados |
| Detector Local/Remoto | **Nenhuma integração direta**; mantêm filas próprias |
| Analisador/Reconciliador | **Nenhuma integração direta**; interpretam posteriormente os estados registrados |

## Limites e segurança

O Scanner deve respeitar a raiz autorizada, tratar nomes Linux não UTF-8, links simbólicos, caminhos especiais, erros de permissão e alterações concorrentes. A varredura **não modifica conteúdo nem remove entradas locais**. A pasta interna `.drivesync/` do Inventário não é um arquivo de usuário a sincronizar.

O Scanner não executa reparo de arquivo, download, upload, renomeação ou exclusão remota. Problemas estruturais que impedem prosseguir ou persistir com segurança são reportados distintamente de problemas isolados de arquivo.

## Documentação

[Interfaces](INTERFACES.md) · [Testes](TESTS.md) · [Orientações para agentes](AGENTS.md)

**Estado:** especificação documental inicial, não implementação Rust. A persistência de diagnósticos individuais no Inventário exige integração ainda não prevista integralmente em seu contrato atual.

## Independência da recuperação de banco

Se o Inventário tiver sido restaurado ou reconstruído após falha de armazenamento, o Scanner executa **a mesma varredura local** e registra as observações pela API habitual. Não solicita backup, não detecta corrupção SQLite nem tenta recuperar histórico. A inicialização só deve invocá-lo quando o Inventário estiver operacional; sem baseline anterior, ele não inventa associações ou exclusões. A política e as notificações de recuperação pertencem a [Configurations](../configuration/README.md) e [Inventory Storage](../inventory/storage/INTERFACES.md).
