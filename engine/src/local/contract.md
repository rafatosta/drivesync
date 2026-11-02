# Contrato — local

O contrato vigente de LOCAL-001/002 está em
[inventário e estabilização](reconciliation.md), com limites, entradas, saídas,
erros, concorrência, cancelamento e oráculos. Consumidores: preparação de
downloads, reconciliação e criação de uploads.

O módulo observa a árvore elegível por descritores, não chama Google e não
instala, publica ou exclui conteúdo. Evidência desconhecida não é ausência;
assinaturas são detectores de mudança, não prova de precedência temporal.
Monitor contínuo permanece proposta e não é pré-condição da varredura por ciclo.
O [contrato proposto de monitoramento](monitoring.md) exige revisão humana
antes de implementação e não altera as garantias atuais do inventário.

Uma leitura negada pelo kernel produz evidência `AccessDenied`, conservada como
`Unknown` para o planner e apresentada como limitação de acesso. A travessia
negada de um diretório invalida o scan completo, preservando o checkpoint anterior.
Não se infere capacidade por bits POSIX e não se tenta elevar permissões.
