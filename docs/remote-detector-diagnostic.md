# Diagnóstico de alterações remotas

O comando é experimental e executado somente sob solicitação. Consulta
metadados e alterações, salva um inventário e uma fila locais e **não**
transfere nem modifica arquivos. A saída JSON contém IDs de conta, nomes e
metadados de arquivos: trate-a como privada e não a publique. Tokens e
cursores não são exibidos. Nenhum ensaio real é comprovado por este roteiro.

## Preparação

Use um aplicativo `com.rtosta.DriveSync.Devel` construído e instalado com o
manifesto de desenvolvimento atualizado, a sessão gráfica com Secret Service
disponível e uma conta Google de testes. Na interface, conecte a conta e
configure uma pasta remota **exclusiva para testes** e uma pasta local
apropriada. O diagnóstico usa o mesmo perfil, cliente OAuth e cofre do
aplicativo instalado; ele não usa a configuração da versão de produção.

Escolha arquivos dispensáveis, sem dados pessoais, e tenha autorização para
criá-los, editá-los, movê-los e excluí-los no Drive. O roteiro solicita que a
pessoa execute essas operações no site do Drive, não o diagnóstico.

## Comandos

Liste perfis e selecione o campo `id` da conta de teste:

```sh
flatpak run --command=drivesync-remote-diagnostic com.rtosta.DriveSync.Devel accounts
```

Substitua `ACCOUNT` pelo ID exato, inclusive `google:`. Para uma pasta em um
drive compartilhado do qual a conta seja membro, acrescente
`--shared-drive DRIVE_ID` a **todos** os comandos abaixo; use o mesmo corpus
em todas as consultas. Para outras pastas, não acrescente a opção.

```sh
flatpak run --command=drivesync-remote-diagnostic com.rtosta.DriveSync.Devel check 'ACCOUNT'
```

Na primeira verificação completa, `Initialized` contém observações `Initial`
do inventário e `InitialComplete`. Anote o `root` dos eventos. A raiz também
aparece no inventário, mesmo quando não há filhos. Mudanças simultâneas à
primeira leitura podem gerar também eventos `Reevaluate`; elas não são
classificadas como criações. Se o resultado for incompleto ou erro, não o
trate como sucesso; corrija a causa e repita.

Consulte eventos sem removê-los:

```sh
flatpak run --command=drivesync-remote-diagnostic com.rtosta.DriveSync.Devel events 'ACCOUNT' 'ROOT'
```

Cada `id` é um identificador local de entrega. Depois de registrar e conferir
um evento, confirme-o explicitamente com:

```sh
flatpak run --command=drivesync-remote-diagnostic com.rtosta.DriveSync.Devel ack 'ACCOUNT' 'ROOT' EVENT_ID
```

Sem confirmação, o evento reaparece nas consultas, inclusive após reinício.
O comando retorna `removed: true` apenas quando removeu um evento pendente
daquele escopo; `false` indica que ele não estava pendente ali.
O comando `events` aceita o ID da raiz anterior para recuperar sua fila
mesmo depois de mudar a configuração da pasta.

## Roteiro funcional

1. Confirme e reconheça os eventos iniciais. Execute `check` novamente sem
   alterar nada: a saída esperada é `NoChanges`.
2. No site do Drive, crie um arquivo dispensável dentro da raiz. Execute
   `check` e verifique seu ID, metadados e `after: Inside`. Confirme o evento.
3. Edite o conteúdo e depois renomeie o arquivo, executando `check` e
   conferindo as observações entre cada ação. A comparação de metadados pode
   mostrar versão ou nome alterado; o evento é `Reevaluate`, não uma afirmação
   de que apenas uma operação específica ocorreu.
4. Crie uma subpasta com arquivo dentro e confira ambos. Mova uma subpasta
   preexistente para dentro e depois para fora da raiz, consultando após cada
   ação: confira também a reavaliação dos descendentes.
5. Mova o arquivo de teste para fora da raiz e consulte. Crie ou altere outro
   arquivo exclusivamente fora da raiz e confirme que ele não é apresentado
   como pertencente à raiz. Exclua ou coloque na lixeira um arquivo dentro da
   raiz e consulte: ausência ou tombstone pode significar perda de acesso,
   mudança de corpus ou exclusão, e não deve ser interpretado como exclusão
   física confirmada.
6. Deixe um evento sem confirmação, encerre o comando e execute `events`
   novamente. O mesmo ID deve reaparecer. Confirme-o e verifique que não
   aparece mais. Se houver outra conta de teste autorizada, repita a
   inicialização nela e verifique que suas filas e erros são independentes.

Registre resultados e limitações separadamente dos testes automatizados. Não
compartilhe JSON contendo dados privados. Ensaios de queda de conexão,
processo interrompido e token rejeitado devem preservar o banco e usar apenas
um ambiente de teste isolado. O reconhecimento de eventos nunca executa
operações em arquivos; resultados ambíguos devem permanecer para reavaliação.
