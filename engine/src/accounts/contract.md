# Contrato — accounts

## Propósito
Identidade de contas e vínculos com pastas.

## Consumidores
coordinator,ui.

## Entrada
Objetos conceituais validados, identificadores estáveis, contexto de cancelamento e, quando pertinente, versões esperadas.

## Saída
Resultado tipado ou erro explícito e classificável: temporário, definitivo, conflito ou resultado ambíguo.

## Responsabilidade e limites
Identidade de contas e vínculos com pastas. Não assumir responsabilidades de outros módulos nem depender da implementação gráfica.

## Invariante
Desvincular nunca apaga conteúdo local ou remoto.

## Qualidade de contrato
Toda operação deverá especificar efeitos colaterais, idempotência, concorrência permitida, pré/pós-condições e recuperação após interrupções. Nenhuma falha pode ser interpretada como sucesso.

## Multi-conta: identidade e ciclo de vida

Gerenciar coleção de contas com `AccountId` interno e identificador estável do Google, sem depender apenas de e-mail exibido. Contas podem ser adicionadas/desvinculadas durante vida do mesmo motor. Toda raiz pertence a exatamente uma conta. Desvincular suspende operações da conta afetada e **não apaga arquivos locais**; outras contas continuam.

Consumidores obtêm perfis sanitizados. Falhas de uma conta não alteram perfis de outras. Testes: vincular duas contas, e-mail alterado, desconectar A durante download de B, reiniciar com duas contas.

## Integração mínima de login

Após auth confirmar identidade e persistir credenciais, registrar perfil sanitizado
em memória por `AccountId = google:<sub>`. Reautorizar o mesmo subject atualiza
o perfil; e-mail não é chave. `list_accounts` não inclui tokens. Raízes e
persistência/descoberta dos perfis continuam em F07/F08; credenciais em auth
sobrevivem ao processo independentemente desta lista.
