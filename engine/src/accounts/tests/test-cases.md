# Casos de teste — contas

Os testes em [`tests.rs`](../tests.rs) exercitam persistência e restauração,
isolamento por cliente e conta, rejeição de dados inválidos, concorrência,
falhas de armazenamento e configuração de pastas. Verificam também que
metadados preexistentes não são criados ou apagados pela configuração.

Testes com dados sintéticos não substituem a
[aceitação com contas e cofre reais](../../../../docs/old/auth-validation.md).
