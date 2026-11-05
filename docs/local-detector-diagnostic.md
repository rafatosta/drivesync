# Diagnóstico de atividade local

O `drivesync-local-diagnostic` observa atividade sem ler conteúdo nem executar
sincronização. `--client` e `--account` são rótulos de teste: não acessam contas,
OAuth, credenciais ou o banco do aplicativo. Use uma pasta exclusiva com arquivos
dispensáveis, fora de pastas pessoais. Os caminhos aparecem na saída.

## Execução pelo GNOME Builder

Selecione o manifesto `flatpak/com.rtosta.DriveSync.Devel.yml`. Ele concede
acesso direto de leitura e escrita a `DriveSync-Test` dentro de Documentos,
usando `--filesystem=xdg-documents/DriveSync-Test`. Crie essa pasta pelo
sistema antes de executar; a concessão de leitura não a cria automaticamente.
Escolha essa pasta ou uma de suas subpastas na configuração da conta.

Após alterar o manifesto, pare a execução anterior, compile novamente e execute
pelo Builder. As permissões são aplicadas ao criar um novo ambiente, não a uma
janela que já estava aberta. Overrides da instalação separada não substituem
as permissões da execução pelo Builder.

Crie, edite, mova e remova arquivos pelo gerenciador de arquivos nessa área.
O detector apenas observa e pode usar a concessão direta mesmo quando a seleção
foi feita pelo portal. A lista temporária apresenta nomes; o estado de cobertura
permanece separado. Acesso somente de leitura é suficiente para esta etapa;
download e outras escritas locais exigirão uma revisão explícita da permissão.

O manifesto de produção não recebe essa autorização. Pastas fora dessa árvore
continuam sujeitas às limitações do portal ou a uma concessão própria. Use apenas
dados descartáveis na área de testes.

## Execução

Compile exclusivamente no SDK definido pelo manifesto de desenvolvimento:

```sh
flatpak run --devel --command=sh \
  --filesystem="$PWD" --filesystem="$HOME/.cargo" org.gnome.Sdk//50 \
  -c 'export PATH=/usr/lib/sdk/rust-stable/bin:$PATH; export CARGO_HOME="$2/.cargo"; cd "$1"; cargo build --offline --locked --bin drivesync-local-diagnostic' \
  sh "$PWD" "$HOME"
```

Em um pacote que inclua o diagnóstico, conceda somente a pasta de testes nesta
execução (o observador solicita acesso de leitura):

```sh
mkdir -p /tmp/drivesync-local-test
flatpak run --filesystem=/tmp/drivesync-local-test:ro \
  --command=drivesync-local-diagnostic com.rtosta.DriveSync.Devel \
  --client diagnostic --account disposable --root /tmp/drivesync-local-test \
  --seconds 60
```

Se o pacote instalado ainda não incluir o novo executável, é possível testar
o binário recém-compilado no sandbox desse aplicativo sem atualizar a instalação:

```sh
flatpak run --filesystem="$PWD/target/debug:ro" \
  --filesystem=/tmp/drivesync-local-test:ro \
  --command="$PWD/target/debug/drivesync-local-diagnostic" \
  com.rtosta.DriveSync.Devel --client diagnostic --account disposable \
  --root /tmp/drivesync-local-test --seconds 60
```

Não é necessário conceder acesso geral ao filesystem nem alterar permissões
persistentes. Este ensaio verifica uma concessão direta de pasta; não comprova
acesso por portal, execução em segundo plano ou integração automática.

## Roteiro

Em outro terminal, manipule somente a pasta de testes:

1. Criar um arquivo e escrever nele: esperar `created`, `written` e
   `closed_after_write` conforme as operações que o sistema emitir.
2. Renomear o arquivo: esperar `moved` com origem e destino relativos.
3. Criar pastas A e B; após o aviso de criação, mover B para dentro de A:
   esperar `moved` da pasta. Criar um arquivo em A/B: esperar aviso nesse caminho.
4. Mover uma pasta com descendentes para dentro da raiz: esperar `moved_in`
   e `recheck/SubtreeAttached`; alterações posteriores devem ser observadas.
5. Mover essa pasta para fora: esperar `moved_out` após a janela de correlação;
   alterações posteriores fora da raiz não devem aparecer como atividade interna.
6. Remover arquivo e pasta: esperar `removed`. Não significa exclusão no Drive.
7. Encerrar e reiniciar: esperar `recheck/Startup`. Mudanças enquanto parado
   não são recuperadas por este componente.
8. Usar `--max-watches 1` com subpastas: esperar `ResourceLimit` e estado
   `Incomplete`. Remover ou renomear a raiz: esperar `Stopped` e saída de erro.

## Saída e interpretação

Cada linha é JSON. `path.hex` e `from.hex` preservam exatamente os bytes Unix;
`utf8` é opcional. Caminho vazio representa a raiz. `object_type` distingue `Directory`, `File`, `Symlink`, `Special` e `Unknown`.
`Unknown` significa que o tipo não pôde ser consultado; não presumir arquivo regular. Os avisos são
pistas históricas, não comandos de sincronização ou prova de conteúdo alterado.

`Observing` indica watches ativos, não inventário conferido ou sincronização.
Filesystems sem cobertura estabelecida, incluindo FUSE do portal de documentos,
produzem `FilesystemUnverified` e iniciam incompletos. A API não resolve links
simbólicos na raiz: um caminho de portal que atravesse o alias simbólico de
`/run/user/.../doc` será rejeitado; mesmo um caminho canônico aceito não prova
entrega de notificações de alterações feitas pelo host.

`Incomplete` persiste até o fim da sessão; avisos posteriores continuam úteis,
mas uma conferência separada é necessária. `Stopped` encerra a coleta e retorna
código 2. Argumentos inválidos e falhas de início/saída também retornam 2.
O encerramento por `--seconds` retorna 0 mesmo em `Incomplete`: isso significa
execução concluída, não cobertura completa. Consulte sempre o estado no JSON.

A fila é volátil e limitada (4096 avisos por padrão). Um aviso reservado informa
`ApplicationOverflow` quando há descarte. O kernel também pode agrupar avisos
ou sinalizar `KernelOverflow`. Não usar esse fluxo para contar salvamentos ou
recuperar todas as versões. `--max-queued`, `--max-watches` e `--poll-ms`
ajustam o diagnóstico; os demais limites estão na API da engine.

## Pendências persistentes

Por padrão, o diagnóstico continua volátil. Para exercitar o registro durável,
use `--state-dir /caminho/privado/estado`, fora da raiz observada. O diretório é
criado com permissão privada; um diretório existente deve ser privado também.
Cada lote é gravado antes de aparecer na saída. A aplicação desktop usa o
registro privado em `XDG_DATA_HOME/drivesync/local-state` automaticamente.

Depois de encerrar, execute o mesmo diagnóstico com os mesmos rótulos `--client`,
`--account`, `--root`, `--state-dir` e acrescente `--pending`. Essa opção consulta
estado, IDs e pendências sem iniciar o observador nem confirmar o consumo.
Os caminhos das pendências são arrays de bytes Unix; `generation` permite
recusar decisões baseadas em estado antigo. Conexão desconhecida marca revisão
necessária, não constitui autorização para aplicar mudanças ao Drive.

O registro persiste necessidades de investigação, não todas as ações ou versões.
Avisos repetidos são agrupados por caminho; nomes iguais não criam identidade.
Vínculos por ID somente são registrados por chamadas explícitas da API depois
de verificação pelo chamador. Startup e perda de cobertura requerem conferência;
o registro não reconstrói mudanças intermediárias ocorridas enquanto parado.
