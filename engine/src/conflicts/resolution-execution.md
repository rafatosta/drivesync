# Executor compartilhado de resolução — SYNC-015/SYNC-016

## Escopo e condição de habilitação

Este contrato especifica o executor necessário para concluir #15 e #16. O
aplicativo ainda não executa essas resoluções. O experimento de precondições
abaixo é executável, mas não constitui um executor nem habilita mutações no
sincronizador.

Uma consulta de revisão seguida de escrita incondicional não protege a janela
entre as duas operações. Não usar `version` como se fosse um token CAS aceito
pelo servidor. Enquanto a proteção da atualização do mesmo ID não for
demonstrada para o endpoint e modo de upload utilizados, manter o item pendente.
Não substituir essa garantia por criação de outro ID ou por escolha automática
da versão mais recente.

## Resultado das operações

| Operação | Resultado requerido |
|---|---|
| C09: rename local + edição remota | Mesmo ID remoto; nome escolhido pela alteração local; bytes da edição remota; local converge para esse nome/conteúdo. |
| C09: edição local + rename remoto | Mesmo ID remoto; nome da alteração remota; bytes da edição local; local converge para esse nome/conteúdo. |
| C07: nomes iguais | Confirmar convergência somente após verificar identidade, localização e conteúdo; não copiar o arquivo. |
| C07: nomes diferentes | Preservar e solicitar decisão; nenhuma escolha implícita por data ou ordem de observação. |
| P03: manter local | Conteúdo local escolhido é o resultado ativo; preservar a versão remota necessária antes da substituição. |
| P03: manter remoto | Conteúdo remoto escolhido é o resultado ativo; preservar a versão local necessária antes da substituição. |
| P03: manter ambas | Preservar ambas as versões com destinos distintos e verificados; não sobrescrever um nome ocupado. A confirmação deve informar qual versão permanece no caminho original e quais cópias serão sincronizadas. |
| P03: decidir depois | Nenhum efeito; a pendência e as versões permanecem disponíveis. |

As escolhas de conteúdo não autorizam exclusões, mudanças de permissões,
tratamento de tipos especiais ou movimentações arbitrárias de árvores. Cada
ação precisa de pré-condições próprias; indisponibilidade de acesso não é
evidência de exclusão nem uma versão selecionável.

## Entrada e autoridade

O plano deve vincular conta, cliente OAuth, associação de raízes e geração da
configuração, ID remoto, revisão/precondição do servidor, identidade da raiz
local, identidade/fingerprint do arquivo, origem, destino e ação autorizada.
Decisões da interface referem-se às evidências exibidas, não apenas ao ID da
pendência. Um plano antigo não pode adotar automaticamente observações novas.

Reutilizar os locks de associação e raiz do coordinator, o CAS SQLite e as
primitivas locais por descritor. Locks advisory não protegem contra escritores
externos. O novo executor deve coletar evidências novamente sob os locks e
validar permissões, pais, colisões, tipos e configuração antes dos efeitos.

## Transições e recuperação requeridas

1. **Preparada:** recolher B/L/R e construir um plano sem efeitos. Rejeitar
   identidade ambígua, conteúdo transitório, raiz indisponível ou destino incerto.
2. **Intenção durável:** persistir plano e identidade da operação antes de
   preservar ou modificar arquivos. Falha de persistência impede efeitos.
3. **Preservada:** registrar cópias verificadas das versões que podem ser
   substituídas; cópia incompleta não atende à pré-condição. Revalidar originais
   após a preservação e antes da escrita.
4. **Efeito remoto pendente:** persistir a fronteira antes de enviar a operação
   condicional ao mesmo ID. Timeout, cancelamento e perda de resposta exigem
   reconciliação por ID/revisão/conteúdo; não repetir a mutação às cegas.
5. **Efeito remoto verificado:** observar localização, revisão e bytes esperados.
   Evidências insuficientes mantêm a intenção em recuperação e B inalterada.
6. **Efeito local pendente:** persistir a fronteira; instalar/renomear por
   descritor sem substituição de destino concorrente. Revalidar raiz, pais e
   identidade antes/depois; falha de fsync não equivale a ausência do rename.
7. **Efeitos verificados:** conferir novamente ambos os lados. Mudança externa
   preserva a operação recuperável e exige nova comparação.
8. **Confirmada:** somente então persistir B e a conclusão da decisão. Como os
   journals existentes podem estar em bancos distintos, explicitar a ordem e a
   recuperação de cada confirmação; não presumir transação entre arquivos SQLite.

Na retomada, carregar intenções antes de novo planejamento. Reconhecer efeitos
já aplicados pela identidade e pelo resultado esperado; não apenas pelo nome.
Uma falha após qualquer efeito não permite apagar intenção ou cópias de proteção.
Configuração alterada impede iniciar novos efeitos sob o plano anterior; efeitos
já enviados continuam exigindo reconciliação.

## Dependências e sequência de desenvolvimento

| Entrega | Dependência e aceite |
|---|---|
| Prova da escrita remota | Experimento por endpoint/modo, incluindo controle positivo e rejeição de revisão obsoleta sem efeito. Resultado desconhecido bloqueia produção. |
| Executor durável | Reutilizar #5/#11/#12/#13; completar garantias pertinentes de #20/#21/#23, sem refazer mecanismos existentes. |
| Aplicação C09/C07 | Concluir #16 com testes nos dois sentidos e confirmação de B. |
| Resolução P03 | Concluir o motor e as ações básicas da #15, preservação e acessibilidade. |
| Movimentações de árvores | #17 acrescenta pais, ciclos e subárvores depois da base da #16. |

#18 contribui com capacidades necessárias a cada operação. #24 é política
opt-in posterior; #25 acrescenta comparação visual por tipo, não substitui a
resolução básica P03 da #15; #26 permanece pesquisa de merge especializado.
Os aceites originais de #15/#16 continuam nessas issues até serem atendidos.

## Experimento de precondições diretas

Referências oficiais consultadas em 2026-10-30:

- [files.update](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/update)
  define atualização do mesmo ID e modalidades de upload. A existência do
  endpoint não comprova proteção de concorrência.
- [Recurso File](https://developers.google.com/workspace/drive/api/reference/rest/v3/files)
  documenta `version`; não estabelece por si só uma precondição de escrita.
- [Performance](https://developers.google.com/workspace/drive/api/guides/performance)
  usa exemplos genéricos e linguagem condicional sobre ETags; não é evidência de
  CAS para cada variante de upload.
- [HTTP If-Match, RFC 9110 §13.1.1](https://www.rfc-editor.org/rfc/rfc9110.html#section-13.1.1)
  define a semântica esperada quando a precondição é implementada. Não demonstra
  que um serviço específico a implementa.

`scripts/check-drive-preconditions.py` cria dois arquivos binários novos de teste,
um para atualização de nome e outro para upload direto de conteúdo. Nunca aceita
ID de arquivo existente, não remove fixtures nem repete uma mutação incerta.
O teste exige ETag forte, confirma escrita com tag atual, tenta duas vezes a tag
obsoleta e exige HTTP 412 com nome, bytes, versão e ETag inalterados. Repete a
sequência com a tag seguinte. Cada snapshot delimita a leitura dos bytes por duas
consultas de metadados iguais. Credenciais e ETags não entram nas evidências.

Execução assistida por cofre, dentro do SDK do manifesto:

1. Abrir o aplicativo com `XDG_DATA_HOME` absoluto e separado, sem sincronização
   configurada, e conectar apenas a conta de teste. Usar a mesma configuração
   OAuth e o mesmo diretório XDG no teste.
2. Definir `DRIVESYNC_PRECONDITION_PROBE=1` e `DRIVESYNC_PROBE_EVIDENCE` como um
   diretório absoluto **novo**, privado e fora de qualquer raiz sincronizada.
3. Executar `cargo test -p drivesync-engine
   facade::live_google_tests::dedicated_account_can_probe_direct_write_preconditions
   -- --ignored --exact`.
   O teste exige exatamente uma conta no perfil e envia o token de curta duração
   ao processo Python por pipe privado, nunca por argumentos ou ambiente.
4. Inspecionar `evidence.jsonl`; cada ID criado fica registrado assim que
   confirmado. Se a criação teve resultado incerto, procurar as fixtures pelo
   prefixo `DriveSync precondition experiment` na conta de teste, sem recriar
   automaticamente. Remoção das fixtures é uma ação manual separada.

Também é possível executar o script com `--run-real --evidence-dir /caminho/novo`
e fornecer um token de curta duração pelo prompt sem eco. Não fornecer tokens
pelo chat, argumentos de shell ou arquivos versionados.

Os testes de `scripts/tests/test_drive_preconditions.py` são simulados e verificam
os oráculos e recusas do experimento; não validam a semântica Google. Um resultado
positivo do experimento real cobre apenas escritas **diretas**, na conta/build
testados. Upload resumível, concorrência local, durabilidade SQLite/filesystem,
resultado incerto, permissões e integração do executor continuam exigindo testes
próprios antes de habilitar a funcionalidade. Não há habilitação automática.

## Oráculos do executor ainda necessários

Usar remoto simulado com filesystem e SQLite reais temporários: C09 nos dois
sentidos, C07 convergente/divergente, todas as escolhas P03, destino ocupado,
troca de inode, links, alteração de configuração, instâncias concorrentes,
revisão alterada imediatamente antes da mutação, acesso perdido, falha de
preservação, rollback, cancelamento e interrupção em cada fronteira acima.
Reabertura/repetição não pode criar cópia falsa, duplicar efeito ou avançar B
indevidamente. Adicionar ensaios específicos de teclado/leitor de tela e do
Flatpak instalado; testes de propriedades GTK não os substituem.
