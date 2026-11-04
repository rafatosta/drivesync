# Diagnóstico de atividade local

O `drivesync-local-diagnostic` observa atividade sem ler conteúdo nem executar
sincronização. `--client` e `--account` são rótulos de teste: não acessam contas,
OAuth, credenciais ou o banco do aplicativo. Use uma pasta exclusiva com arquivos
dispensáveis, fora de pastas pessoais. Os caminhos aparecem na saída.

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
`utf8` é opcional. Caminho vazio representa a raiz. `is_directory: null` significa
que o tipo não foi determinado; não presumir arquivo regular. Os avisos são
pistas históricas, não comandos de sincronização ou prova de conteúdo alterado.

`Observing` indica watches ativos, não inventário conferido ou sincronização.
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
