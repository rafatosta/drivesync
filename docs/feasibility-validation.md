# Estudo de viabilidade técnica — DriveSync

**Plano de implementação ativo:** [MVP — roteiro e checklist](MVP.md). A aceitação do MVP cobre a prova de download/incremental/recuperação em Flatpak; os marcos gerais F3 (upload seguro) e F4 (bidirecional/exclusões) continuam **fora do MVP** e não devem ser dados como aprovados por ele.


## Pergunta
Conseguimos manter pasta local e pasta remota do Google Drive coerentes e recuperáveis, em uso real no Fedora/Flatpak, sem perda silenciosa de dados?

## Provas graduais
| Marco | Experimento controlado | Aceite |
|---|---|---|
| F0 — OAuth e escopos | autorizar conta de teste, listar pastas/arquivos já existentes, renovar tokens | autorização realmente permite o caso de uso; custos/limitações da publicação compreendidos |
| F1 — leitura incremental | inventário inicial, cursor changes, vários ciclos sem alterações | IDs/revisões consistentes, nenhuma transferência duplicada |
| F2 — download seguro | streaming, staging, hash/tamanho quando disponível, crash e espaço insuficiente | arquivo original intacto em todas as falhas |
| F3 — upload seguro | criar/atualizar arquivo de teste, interromper sessão resumable | retomada ou reconciliação sem duplicação/perda |
| F4 — conflito e exclusões | alteração local/remota simultânea, rename, remoção e mudança de permissão | versões preservadas; nenhuma exclusão causada por estado UNKNOWN |
| F5 — execução GNOME | Flatpak aberto/oculto/autostart, repetidas ativações | motor único e progresso independente da janela; saída limpa |
| F6 — robustez | injetar falhas em cada fronteira de efeito/commit e execução prolongada | invariantes comprovados, erros claros e recuperação reproduzível |

## Experimentos de alto risco
- Escopo `drive.file` permite a experiência desejada com arquivos anteriores ao DriveSync? Se não, avaliar `drive` e requisitos de verificação (ver R-SCOPES).
- É possível executar atualização remota com garantia de revisão esperada? Não inferir atomicidade de uma verificação anterior.
- Downloads do Workspace Google Docs/Sheets/Slides não equivalem a arquivos binários comuns; adiar ou definir estratégia explícita.
- Renomeações, arquivos com nomes duplicados, mudanças de compartilhamento e múltiplas revisões: validar identidade por ID, não por caminho apenas.

## Métricas e evidência
Número de dados perdidos/sobrescritos sem decisão (=0 nos cenários testados); operações duplicadas; número de reuploads em ciclo sem alterações (=0); recuperação após crash; consumo de API, memória e CPU; tempo de convergência. Metas de performance detalhadas são estabelecidas após medições reais, não inventadas antecipadamente.

## Ambiente seguro
Conta Google separada, pasta remota dedicada, diretório temporário local, arquivos gerados com conteúdo descartável; nunca executar cenários de exclusão sobre arquivos pessoais.

## Estado do estudo
PLANEJADO. Nenhuma prova acima pode ser declarada aprovada apenas com documentação.
