# Models — contratos comuns da engine

Este diretório destina-se a **modelos de domínio reutilizáveis**, como `RemoteFile`, `LocalFile` e referências de identidade de sincronização, quando seus contratos forem formalizados. Modelos compartilhados não devem importar `GoogleFile` ou clientes HTTP como dependências obrigatórias.

Cada serviço mantém sua representação específica em `providers/<provedor>/`; conversões e adaptações são responsabilidade de componentes que conhecem os dois lados do contrato. **Ainda não há especificação final de `RemoteFile` aqui:** esta etapa reorganiza documentos por responsabilidade, sem presumir structs existentes ou alterar contratos atuais do Inventário.
