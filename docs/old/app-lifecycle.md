# Ciclo de vida do aplicativo

Uma aplicação GTK/Libadwaita apresenta a janela de contas. Reativar uma
instância existente apresenta novamente a janela. A restauração de contas
ocorre em um worker, sem bloquear a interface.

Fechar pode ocultar a janela e manter o processo em segundo plano conforme as
preferências e a autorização do ambiente; **Quit** encerra o aplicativo.
Inicialização automática depende de suporte e consentimento do portal.
Segundo plano não inicia transferências de arquivos.

Verificar reativação, fechamento, saída e indisponibilidade do portal em uma
sessão de desktop apropriada. A compilação não comprova integração real.
