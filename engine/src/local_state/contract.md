# Identidade, estado e pendências locais

## Entrega

Registro próprio da nova engine, isolado por cliente OAuth, conta e raiz local.
Não lê conteúdo, não investiga automaticamente versões, não transfere e não
modifica arquivos observados ou objetos remotos. Não reaproveita bancos antigos.
O observador continua emitindo pistas; elas passam a ser persistidas antes da
entrega à interface. Falhas de armazenamento interrompem essa sessão e ficam
visíveis, sem apresentar a entrega como confirmada.

## Identidade e árvore

Um registro verificado recebe ID interno aleatório e pode ter ID do Drive.
Nome e pasta pai são propriedades mutáveis; não identificam o documento.
Vínculos são registrados somente por uma API explícita cujo chamador deve ter
verificado a associação (futuro download/investigador). Nenhum aviso, inode,
conteúdo igual ou coincidência de caminho cria automaticamente identidade.
A raiz tem nome vazio e nenhum pai; os demais itens têm nome Unix sem `/`, NUL,
`.` ou `..` e pai diretório existente. Rejeitar ciclos, caminhos ambíguos,
IDs remotos duplicados, profundidade maior que 128 e caminhos maiores que 4096
bytes. Mover um diretório verificado muda apenas seu pai/nome; os descendentes
mantêm IDs e caminhos derivados. Remoção é um estado, não exclusão do registro.

Cada item preserva estado confirmado anterior e estado atual observado,
incluindo uma referência opcional de versão de conteúdo e de versão remota.
Essas referências são fornecidas pelo chamador: o registro não calcula hashes
nem declara que uma versão ausente corresponde a conteúdo inalterado.
Registrar/atualizar/confirmar estado requer a geração lida pelo chamador;
concorrência rejeita decisões obsoletas. Confirmar não executa transferência:
é uma fronteira explícita para um futuro executor com resultado verificado.

## Pendências

Agrupar avisos pelo caminho em uma necessidade de investigação, conservando
motivos e ID/revisão da pendência. Movimentos afetam origem e destino; movimentos
de diretórios e perdas de cobertura exigem conferência da árvore. Um registro
no mesmo caminho é apenas candidato, nunca identidade confirmada do aviso.
Nenhum aviso altera automaticamente a árvore confirmada/observada.

Pendências locais têm prioridade de agendamento sobre conferências rotineiras;
isso não dispensa reconciliação, identidade, conflitos ou confirmação. Conexão é
contexto explícito do produtor, não deduzida do OAuth: offline ou desconhecida
marca necessidade de confirmação, preservada ao agrupar. O worker atual informa
desconhecida; a futura detecção de conectividade e a interface de confirmação
não fazem parte desta etapa. Nada é executável automaticamente.

Concluir investigação requer geração atual e revisão exata, para não apagar
um aviso recebido enquanto o consumidor examinava a pendência. Isso somente
remove a necessidade de investigação, não autoriza exclusão ou transferência.
Startup sempre agenda conferência: o banco não recupera ações intermediárias
ocorridas enquanto o observador estava parado. O histórico visual permanece
volátil; a fila de trabalho é independente e sobrevive ao fechamento da janela.

## Persistência e limites

SQLite privado, formato atual único, sem migração. Inicialização atômica sem
substituir arquivos existentes; rejeitar symlinks, permissões abertas, versões
incompatíveis e conteúdo inválido sem apagá-los. Transação IMMEDIATE confirma
estado e pendências conjuntamente; FULL synchronous. Erro de commit pode ser
ambíguo: reler antes de repetir. Isolamento por chave composta de escopo.

Limites por escopo: 10.000 itens, 4.096 pendências e 16 MiB de payload. Estado
inteiro é validado e gravado por transação; otimização para árvores maiores é
uma etapa posterior. Atingir o limite de pendências substitui a lista por uma
conferência abrangente da raiz, preservando necessidade de confirmação. Não
confundir isso com conclusão das pendências anteriores. Atingir outros limites
retorna erro preservando o estado confirmado no banco. Não há coleta automática
de escopos antigos; não prometer limite global de disco entre infinitos escopos.

## Algoritmo e verificação

1. Abrir banco privado e validar formato.
2. Iniciar transação, carregar documento do escopo e validar estrutura/limites.
3. Aplicar mutação ou agrupar avisos; validar geração para decisões do consumidor.
4. Validar árvore, versões e pendências; incrementar geração e confirmar juntos.
5. Publicar lote ao consumidor apenas após o commit; falha deixa estado anterior
   ou exige releitura se o commit foi ambíguo.

Testes isolados cobrem IDs após reinício, árvore e nomes não UTF-8, mudanças de
pai, tombstones, referências de versões, duplicações/ciclos, escopos, agrupar
avisos, corridas de conclusão, confirmação offline, limites, incompatibilidade,
rollback, escritores concorrentes e interrupção de processo. Integração do
worker deve demonstrar persistência antes da entrega, sem contas reais.

Fundamentação: [transações SQLite](https://www.sqlite.org/lang_transaction.html)
e [atomic commit](https://www.sqlite.org/atomiccommit.html), consultados em
2026-11-04. Atomicidade depende das premissas de filesystem/hardware do SQLite;
testes não comprovam sobrevivência a qualquer falha física.
